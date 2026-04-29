use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info};

use crate::input::events::{InputEvent, MouseButton, Modifiers};
use super::{InputCapture, InputInjector};

pub struct MacOsCapture {
    locked:      Arc<Mutex<bool>>,
    lock_pos:    Arc<Mutex<(f64, f64)>>,
    running:     Arc<AtomicBool>,
    /// Posição virtual do cursor no ecrã remoto (0..1), convenção PADRÃO: (0,0) = topo-esq.
    /// Inicializada com as coordenadas de entrada e actualizada via posição absoluta do cursor.
    virtual_pos: Arc<Mutex<(f32, f32)>>,
}

impl MacOsCapture {
    pub fn new() -> Self {
        Self {
            locked:      Arc::new(Mutex::new(false)),
            lock_pos:    Arc::new(Mutex::new((0.0, 0.0))),
            running:     Arc::new(AtomicBool::new(false)),
            virtual_pos: Arc::new(Mutex::new((0.0, 0.5))),
        }
    }
}

impl InputCapture for MacOsCapture {
    fn start(&self, callback: Box<dyn Fn(InputEvent) + Send + Sync + 'static>) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let locked      = Arc::clone(&self.locked);
        let lock_pos    = Arc::clone(&self.lock_pos);
        let running     = Arc::clone(&self.running);
        let virtual_pos = Arc::clone(&self.virtual_pos);

        std::thread::spawn(move || {
            use core_graphics::event::{
                CGEventTap, CGEventTapLocation, CGEventTapPlacement,
                CGEventTapOptions, CGEventType,
            };

            let callback          = Arc::new(callback);
            let locked_inner      = Arc::clone(&locked);
            let lock_pos_inner    = Arc::clone(&lock_pos);
            let virtual_pos_inner = Arc::clone(&virtual_pos);

            let tap_result = CGEventTap::new(
                CGEventTapLocation::HID,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                vec![
                    CGEventType::MouseMoved,
                    CGEventType::LeftMouseDown,
                    CGEventType::LeftMouseUp,
                    CGEventType::LeftMouseDragged,   // necessário para arrastar no PC remoto
                    CGEventType::RightMouseDown,
                    CGEventType::RightMouseUp,
                    CGEventType::RightMouseDragged,  // necessário para arrastar no PC remoto
                    CGEventType::ScrollWheel,
                    CGEventType::KeyDown,
                    CGEventType::KeyUp,
                ],
                move |_proxy, event_type, event| {
                    let is_locked = locked_inner.lock()
                        .map(|g| *g)
                        .unwrap_or(false);

                    if is_locked {
                        match event_type {
                            // ── Movimento do rato (e arrastar) quando cursor está preso ──────────
                            // Usamos a posição ABSOLUTA do evento menos lock_pos para obter o delta.
                            // Isto inclui a aceleração do macOS — ao contrário de MOUSE_EVENT_DELTA
                            // que retorna valores RAW sem aceleração, o que tornava o cursor remoto
                            // ~10× mais lento que o esperado.
                            // Após ler a posição, fazemos warp de volta a lock_pos; o evento de
                            // warp terá location == lock_pos, logo delta == 0 — sem feedback loop.
                            CGEventType::MouseMoved
                            | CGEventType::LeftMouseDragged
                            | CGEventType::RightMouseDragged => {
                                let loc = event.location();
                                let (lx, ly) = lock_pos_inner.lock()
                                    .map(|g| *g)
                                    .unwrap_or((0.0, 0.0));

                                let dx = (loc.x - lx) as f32;
                                // Quartz: Y cresce para baixo. Na convenção padrão (0=topo),
                                // mover para baixo → dy > 0 → vy deve AUMENTAR.
                                let dy = (loc.y - ly) as f32;

                                let (vx, vy) = {
                                    use core_graphics::display::CGDisplay;
                                    let b = CGDisplay::main().bounds();
                                    let w = b.size.width.max(1.0) as f32;
                                    let h = b.size.height.max(1.0) as f32;
                                    let mut vp = virtual_pos_inner.lock()
                                        .unwrap_or_else(|p| p.into_inner());
                                    vp.0 = (vp.0 + dx / w).clamp(0.0, 1.0);
                                    // += dy/h: dy>0 (baixo) → vy cresce → cursor move para baixo ✓
                                    vp.1 = (vp.1 + dy / h).clamp(0.0, 1.0);
                                    *vp
                                };
                                callback(InputEvent::MouseMove { x: vx, y: vy });

                                // Manter o cursor preso visualmente em lock_pos.
                                let _ = core_graphics::display::CGDisplay::warp_mouse_cursor_position(
                                    core_graphics::geometry::CGPoint { x: lx, y: ly }
                                );
                                return None;
                            }

                            // ── Botões: suprimir localmente e reencaminhar ao PC remoto ──────────
                            CGEventType::LeftMouseDown
                            | CGEventType::LeftMouseUp
                            | CGEventType::RightMouseDown
                            | CGEventType::RightMouseUp => {
                                let btn_event = match event_type {
                                    CGEventType::LeftMouseDown  => Some(InputEvent::MouseButton { button: MouseButton::Left,  pressed: true  }),
                                    CGEventType::LeftMouseUp    => Some(InputEvent::MouseButton { button: MouseButton::Left,  pressed: false }),
                                    CGEventType::RightMouseDown => Some(InputEvent::MouseButton { button: MouseButton::Right, pressed: true  }),
                                    CGEventType::RightMouseUp   => Some(InputEvent::MouseButton { button: MouseButton::Right, pressed: false }),
                                    _ => None,
                                };
                                if let Some(ev) = btn_event {
                                    callback(ev);
                                }
                                // Manter o cursor preso visualmente.
                                let (lx, ly) = lock_pos_inner.lock()
                                    .map(|g| *g)
                                    .unwrap_or((0.0, 0.0));
                                let _ = core_graphics::display::CGDisplay::warp_mouse_cursor_position(
                                    core_graphics::geometry::CGPoint { x: lx, y: ly }
                                );
                                return None;
                            }
                            _ => {}
                        }
                    }

                    // ── Eventos locais (cursor NÃO está no PC remoto) ───────────────────────────
                    let input = match event_type {
                        CGEventType::MouseMoved
                        | CGEventType::LeftMouseDragged
                        | CGEventType::RightMouseDragged => {
                            // Convenção PADRÃO: (0,0) = topo-esq, (1,1) = base-dir.
                            // `loc.y` Quartz: 0 = topo, cresce para baixo → ny = loc_y / h
                            let loc = event.location();
                            let (nx, ny) = normalize_mouse_standard(loc.x, loc.y);
                            Some(InputEvent::MouseMove { x: nx, y: ny })
                        }
                        CGEventType::LeftMouseDown => Some(InputEvent::MouseButton {
                            button: MouseButton::Left, pressed: true,
                        }),
                        CGEventType::LeftMouseUp => Some(InputEvent::MouseButton {
                            button: MouseButton::Left, pressed: false,
                        }),
                        CGEventType::RightMouseDown => Some(InputEvent::MouseButton {
                            button: MouseButton::Right, pressed: true,
                        }),
                        CGEventType::RightMouseUp => Some(InputEvent::MouseButton {
                            button: MouseButton::Right, pressed: false,
                        }),
                        CGEventType::ScrollWheel => {
                            use core_graphics::event::EventField;
                            let dy = event.get_integer_value_field(
                                EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1) as f32;
                            let dx = event.get_integer_value_field(
                                EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2) as f32;
                            Some(InputEvent::MouseScroll { dx, dy })
                        }
                        CGEventType::KeyDown | CGEventType::KeyUp => {
                            use core_graphics::event::{EventField, CGEventFlags};
                            let keycode = event.get_integer_value_field(
                                EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                            let flags = event.get_flags();
                            let mut mods = Modifiers::NONE;
                            if flags.contains(CGEventFlags::CGEventFlagShift)    { mods = Modifiers(mods.0 | Modifiers::SHIFT.0); }
                            if flags.contains(CGEventFlags::CGEventFlagControl)  { mods = Modifiers(mods.0 | Modifiers::CTRL.0); }
                            if flags.contains(CGEventFlags::CGEventFlagAlternate){ mods = Modifiers(mods.0 | Modifiers::ALT.0); }
                            if flags.contains(CGEventFlags::CGEventFlagCommand)  { mods = Modifiers(mods.0 | Modifiers::META.0); }
                            let pressed = matches!(event_type, CGEventType::KeyDown);
                            Some(InputEvent::KeyEvent { keycode, pressed, modifiers: mods })
                        }
                        _ => None,
                    };

                    if let Some(ev) = input {
                        callback(ev);
                    }
                    Some(event.to_owned())
                },
            );

            match tap_result {
                Ok(tap) => {
                    info!("MacOsCapture: CGEventTap criado");
                    let loop_src = match tap.mach_port.create_runloop_source(0) {
                        Ok(s) => s,
                        Err(_) => {
                            error!("MacOsCapture: falha ao criar RunLoopSource");
                            running.store(false, Ordering::SeqCst);
                            return;
                        }
                    };
                    let run_loop = core_foundation::runloop::CFRunLoop::get_current();
                    run_loop.add_source(&loop_src, unsafe {
                        core_foundation::runloop::kCFRunLoopDefaultMode
                    });
                    tap.enable();
                    while running.load(Ordering::SeqCst) {
                        unsafe {
                            core_foundation::runloop::CFRunLoop::run_in_mode(
                                core_foundation::runloop::kCFRunLoopDefaultMode,
                                std::time::Duration::from_millis(100),
                                true,
                            );
                        }
                    }
                    info!("MacOsCapture: loop encerrado");
                }
                Err(e) => {
                    error!("CGEventTap falhou: {:?} — verifique permissão de Acessibilidade em Preferências do Sistema", e);
                    running.store(false, Ordering::SeqCst);
                }
            }
        });

        info!("MacOsCapture: captura iniciada");
        Ok(())
    }

    fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        info!("MacOsCapture: encerrada");
    }

    fn lock_cursor(&self, entry_x: f32, entry_y: f32) {
        if let Ok(pos) = get_cursor_position() {
            *self.lock_pos.lock().unwrap() = pos;
        }
        // Inicializar posição virtual com o ponto de entrada no ecrã remoto.
        // entry_x, entry_y usam convenção PADRÃO (0=topo-esq, 1=base-dir).
        *self.virtual_pos.lock().unwrap() = (entry_x, entry_y);
        *self.locked.lock().unwrap() = true;
        info!("MacOsCapture: cursor bloqueado; virtual_entry=({:.3}, {:.3})", entry_x, entry_y);
    }

    fn unlock_cursor(&self) {
        *self.locked.lock().unwrap() = false;
        info!("MacOsCapture: cursor liberado");
    }
}

// ── Injector ──────────────────────────────────────────────────────────────────

pub struct MacOsInjector;

impl MacOsInjector {
    pub fn new() -> Self { Self }
}

impl InputInjector for MacOsInjector {
    fn inject(&self, event: InputEvent) -> Result<(), String> {
        use core_graphics::event::{CGEvent, CGEventType, CGMouseButton, CGEventFlags};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        use core_graphics::geometry::CGPoint;

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "Falha ao criar CGEventSource".to_string())?;

        match event {
            InputEvent::MouseMove { x, y } => {
                // Convenção PADRÃO: x=0 esq, x=1 dir, y=0 topo, y=1 base.
                // Quartz: origin na top-left, Y cresce para baixo → mapeamento directo.
                use core_graphics::display::CGDisplay;
                let d = CGDisplay::main();
                let b = d.bounds();
                let gx = b.origin.x + (x as f64) * b.size.width;
                let gy = b.origin.y + (y as f64) * b.size.height;
                let pt = CGPoint { x: gx, y: gy };
                let ev = CGEvent::new_mouse_event(source, CGEventType::MouseMoved, pt, CGMouseButton::Left)
                    .map_err(|_| "Falha MouseMove".to_string())?;
                ev.post(core_graphics::event::CGEventTapLocation::HID);
            }
            InputEvent::MouseButton { button, pressed } => {
                let (et, cb) = match (button, pressed) {
                    (MouseButton::Left,  true)  => (CGEventType::LeftMouseDown,  CGMouseButton::Left),
                    (MouseButton::Left,  false) => (CGEventType::LeftMouseUp,    CGMouseButton::Left),
                    (MouseButton::Right, true)  => (CGEventType::RightMouseDown, CGMouseButton::Right),
                    (MouseButton::Right, false) => (CGEventType::RightMouseUp,   CGMouseButton::Right),
                    _ => return Ok(()),
                };
                let pos = get_cursor_position().map(|(x,y)| CGPoint{x,y}).unwrap_or(CGPoint{x:0.0,y:0.0});
                let ev = CGEvent::new_mouse_event(source, et, pos, cb)
                    .map_err(|_| "Falha MouseButton".to_string())?;
                ev.post(core_graphics::event::CGEventTapLocation::HID);
            }
            InputEvent::MouseScroll { dy, .. } => {
                use core_graphics::event::EventField;
                let ev = CGEvent::new(source.clone())
                    .map_err(|_| "Falha Scroll".to_string())?;
                ev.set_type(CGEventType::ScrollWheel);
                ev.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1, dy as i64);
                ev.post(core_graphics::event::CGEventTapLocation::HID);
            }
            InputEvent::KeyEvent { keycode, pressed, modifiers } => {
                let ev = CGEvent::new_keyboard_event(source, keycode as u16, pressed)
                    .map_err(|_| "Falha KeyEvent".to_string())?;
                let mut flags = CGEventFlags::empty();
                if modifiers.contains(Modifiers::SHIFT) { flags |= CGEventFlags::CGEventFlagShift; }
                if modifiers.contains(Modifiers::CTRL)  { flags |= CGEventFlags::CGEventFlagControl; }
                if modifiers.contains(Modifiers::ALT)   { flags |= CGEventFlags::CGEventFlagAlternate; }
                if modifiers.contains(Modifiers::META)  { flags |= CGEventFlags::CGEventFlagCommand; }
                ev.set_flags(flags);
                ev.post(core_graphics::event::CGEventTapLocation::HID);
            }
        }
        Ok(())
    }
}

/// Normaliza `event.location()` (pontos Quartz) para 0..1 com CONVENÇÃO PADRÃO:
/// (0,0) = canto **superior esquerdo**, (1,1) = canto **inferior direito**.
///
/// Quartz: Y=0 no topo do display principal, cresce para baixo → divisão directa (sem inversão).
/// Esta convenção é consistente com Windows (`normalize_point_against_display_rect`) e com
/// o sistema de coordenadas de `check_boundary` / `ScreenLayout`.
fn normalize_mouse_standard(loc_x: f64, loc_y: f64) -> (f32, f32) {
    use core_graphics::display::CGDisplay;
    let d = CGDisplay::main();
    let b = d.bounds();
    let w_pt = b.size.width.max(1.0);
    let h_pt = b.size.height.max(1.0);
    let rel_x = loc_x - b.origin.x;
    let rel_y = loc_y - b.origin.y;
    // Y cresce para baixo em Quartz → ny = rel_y / h, sem inversão.
    let nx = (rel_x / w_pt).clamp(0.0, 1.0) as f32;
    let ny = (rel_y / h_pt).clamp(0.0, 1.0) as f32;
    (nx, ny)
}

fn get_cursor_position() -> Result<(f64, f64), ()> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).map_err(|_| ())?;
    let ev = CGEvent::new(source).map_err(|_| ())?;
    let loc = ev.location();
    Ok((loc.x, loc.y))
}
