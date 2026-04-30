//! Captura e injecção de eventos de rato/teclado no macOS.
//!
//! # Padrão Barrier/Input-Leap (implementação comprovada)
//!
//! O Barrier resolve o problema "cursor clipped at edge → delta=0" assim:
//! 1. `lock_cursor`: warp para o CENTRO do ecrã (não para a borda).
//!    Actualizar `prev_pos = center` ANTES do warp para que o evento sintético
//!    gerado pelo CGWarpMouseCursorPosition tenha `delta = center - center = 0`.
//! 2. Callback locked: `dx = event.location().x - prev_pos.x`,
//!    depois `prev_pos = center`, depois warp para centro.
//!    O próximo evento sintético tem `dx = 0` → skip automático.
//! 3. Filtro "bogus zone": descartar deltas > 90% da semi-largura
//!    (artefacto residual de qualquer warp inesperado).
//!
//! Esta abordagem garante que:
//! - O cursor nunca fica clipado na borda → deltas sempre não-nulos
//! - O evento sintético do warp tem sempre delta=0 → sem loop infinito
//! - A velocidade do cursor inclui a aceleração do macOS (via location diff)

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info};

use crate::input::events::{InputEvent, MouseButton, Modifiers};
use super::{InputCapture, InputInjector};

pub struct MacOsCapture {
    locked:   Arc<Mutex<bool>>,
    /// Centro do display principal — posição para onde o cursor é warped quando locked.
    center:   Arc<Mutex<(f64, f64)>>,
    /// Última posição conhecida do cursor (equivalente a m_xCursor/m_yCursor do Barrier).
    /// Actualizado para `center` ANTES de cada warp para que o evento sintético do warp
    /// tenha delta zero.
    prev_pos: Arc<Mutex<(f64, f64)>>,
    running:  Arc<AtomicBool>,
    /// Posição virtual do cursor no ecrã remoto (0..1, convenção padrão 0=topo-esq).
    virt_pos: Arc<Mutex<(f32, f32)>>,
}

impl MacOsCapture {
    pub fn new() -> Self {
        Self {
            locked:   Arc::new(Mutex::new(false)),
            center:   Arc::new(Mutex::new((0.0, 0.0))),
            prev_pos: Arc::new(Mutex::new((0.0, 0.0))),
            running:  Arc::new(AtomicBool::new(false)),
            virt_pos: Arc::new(Mutex::new((0.5, 0.5))),
        }
    }
}

impl InputCapture for MacOsCapture {
    fn start(&self, callback: Box<dyn Fn(InputEvent) + Send + Sync + 'static>) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let locked   = Arc::clone(&self.locked);
        let center   = Arc::clone(&self.center);
        let prev_pos = Arc::clone(&self.prev_pos);
        let running  = Arc::clone(&self.running);
        let virt_pos = Arc::clone(&self.virt_pos);

        std::thread::spawn(move || {
            use core_graphics::event::{
                CGEventTap, CGEventTapLocation, CGEventTapPlacement,
                CGEventTapOptions, CGEventType,
            };

            let callback   = Arc::new(callback);
            let locked_c   = Arc::clone(&locked);
            let center_c   = Arc::clone(&center);
            let prev_pos_c = Arc::clone(&prev_pos);
            let virt_pos_c = Arc::clone(&virt_pos);

            let tap_result = CGEventTap::new(
                CGEventTapLocation::HID,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                vec![
                    CGEventType::MouseMoved,
                    CGEventType::LeftMouseDown,
                    CGEventType::LeftMouseUp,
                    CGEventType::LeftMouseDragged,   // arrastar no PC remoto
                    CGEventType::RightMouseDown,
                    CGEventType::RightMouseUp,
                    CGEventType::RightMouseDragged,  // arrastar no PC remoto
                    CGEventType::ScrollWheel,
                    CGEventType::KeyDown,
                    CGEventType::KeyUp,
                ],
                move |_proxy, event_type, event| {
                    let is_locked = locked_c.lock().map(|g| *g).unwrap_or(false);

                    if is_locked {
                        match event_type {
                            // ── Movimento quando cursor está no PC remoto ──────────────────────
                            CGEventType::MouseMoved
                            | CGEventType::LeftMouseDragged
                            | CGEventType::RightMouseDragged => {
                                let loc = event.location();

                                // Ler prev_pos e centro antes de qualquer actualização
                                let (prev_x, prev_y) = {
                                    let pp = prev_pos_c.lock().unwrap_or_else(|p| p.into_inner());
                                    *pp
                                };
                                let (cx, cy) = {
                                    let cc = center_c.lock().unwrap_or_else(|p| p.into_inner());
                                    *cc
                                };

                                let dx = (loc.x - prev_x) as f32;
                                let dy = (loc.y - prev_y) as f32;

                                // ── PADRÃO BARRIER ─────────────────────────────────────────────
                                // Actualizar prev_pos para CENTRO antes do warp.
                                // Quando o CGWarpMouseCursorPosition gerar o evento sintético,
                                // ele chegará com loc == center e dx = center - center = 0.
                                {
                                    let mut pp = prev_pos_c.lock().unwrap_or_else(|p| p.into_inner());
                                    *pp = (cx, cy);
                                }
                                let _ = core_graphics::display::CGDisplay::warp_mouse_cursor_position(
                                    core_graphics::geometry::CGPoint { x: cx, y: cy }
                                );

                                // Descartar eventos com delta zero (evento sintético do warp)
                                if dx.abs() < 0.5 && dy.abs() < 0.5 {
                                    return None;
                                }

                                // Filtro bogus zone: descartar deltas impossíveis (> 90% da semi-largura)
                                // como faz o Barrier para filtrar artefactos residuais do warp.
                                let (w, h) = {
                                    use core_graphics::display::CGDisplay;
                                    let b = CGDisplay::main().bounds();
                                    (b.size.width as f32, b.size.height as f32)
                                };
                                if dx.abs() > w * 0.45 || dy.abs() > h * 0.45 {
                                    return None;
                                }

                                // Acumular delta normalizado na posição virtual remota.
                                // (incluindo aceleração do macOS — loc.diff tem aceleração ao contrário de MOUSE_EVENT_DELTA)
                                let (vx, vy) = {
                                    let mut vp = virt_pos_c.lock().unwrap_or_else(|p| p.into_inner());
                                    vp.0 = (vp.0 + dx / w).clamp(0.0, 1.0);
                                    vp.1 = (vp.1 + dy / h).clamp(0.0, 1.0);
                                    *vp
                                };
                                callback(InputEvent::MouseMove { x: vx, y: vy });
                                return None;
                            }

                            // ── Botões: suprimir localmente, reencaminhar ao PC remoto ─────────
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
                                return None;
                            }
                            // Scroll encaminhado ao PC remoto e suprimido localmente para
                            // evitar que o scroll se aplique simultaneamente nos dois PCs.
                            CGEventType::ScrollWheel => {
                                use core_graphics::event::EventField;
                                let dy = event.get_integer_value_field(
                                    EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1) as f32;
                                let dx = event.get_integer_value_field(
                                    EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2) as f32;
                                callback(InputEvent::MouseScroll { dx, dy });
                                return None;
                            }
                            _ => {}
                        }
                    }

                    // ── Eventos locais (cursor neste PC) ───────────────────────────────────────
                    let input = match event_type {
                        CGEventType::MouseMoved
                        | CGEventType::LeftMouseDragged
                        | CGEventType::RightMouseDragged => {
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
                            if flags.contains(CGEventFlags::CGEventFlagShift)     { mods = Modifiers(mods.0 | Modifiers::SHIFT.0); }
                            if flags.contains(CGEventFlags::CGEventFlagControl)   { mods = Modifiers(mods.0 | Modifiers::CTRL.0); }
                            if flags.contains(CGEventFlags::CGEventFlagAlternate) { mods = Modifiers(mods.0 | Modifiers::ALT.0); }
                            if flags.contains(CGEventFlags::CGEventFlagCommand)   { mods = Modifiers(mods.0 | Modifiers::META.0); }
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
                    info!("MacOsCapture: CGEventTap criado (padrão Barrier)");
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
                    error!("CGEventTap falhou: {:?} — verifique permissão de Acessibilidade", e);
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

    /// Travar o cursor — abordagem Barrier:
    /// 1. Warp para o CENTRO do ecrã (o cursor nunca fica clipado na borda)
    /// 2. Actualizar `prev_pos = center` ANTES do warp para que o evento sintético
    ///    gerado pelo warp tenha `delta = center - center = 0` → é descartado automaticamente
    fn lock_cursor(&self, entry_x: f32, entry_y: f32) {
        use core_graphics::display::CGDisplay;
        let b = CGDisplay::main().bounds();
        let cx = b.origin.x + b.size.width  / 2.0;
        let cy = b.origin.y + b.size.height / 2.0;

        // unwrap_or_else: recupera o valor mesmo se outra thread sofreu panic com o lock.
        *self.center.lock().unwrap_or_else(|p| p.into_inner()) = (cx, cy);
        *self.prev_pos.lock().unwrap_or_else(|p| p.into_inner()) = (cx, cy);
        *self.virt_pos.lock().unwrap_or_else(|p| p.into_inner()) = (entry_x, entry_y);
        *self.locked.lock().unwrap_or_else(|p| p.into_inner()) = true;

        let _ = CGDisplay::warp_mouse_cursor_position(
            core_graphics::geometry::CGPoint { x: cx, y: cy }
        );

        info!(
            "MacOsCapture: locked → warp para centro ({:.0},{:.0}); entry=({:.3},{:.3})",
            cx, cy, entry_x, entry_y
        );
    }

    fn unlock_cursor(&self) {
        *self.locked.lock().unwrap_or_else(|p| p.into_inner()) = false;
        info!("MacOsCapture: unlocked");
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
                // Convenção padrão: x=0 esq, x=1 dir, y=0 topo, y=1 base.
                // Quartz: origin na top-left, Y cresce para baixo → mapeamento directo.
                use core_graphics::display::CGDisplay;
                let b = CGDisplay::main().bounds();
                let gx = b.origin.x + (x as f64) * b.size.width;
                let gy = b.origin.y + (y as f64) * b.size.height;
                let ev = CGEvent::new_mouse_event(
                    source, CGEventType::MouseMoved,
                    CGPoint { x: gx, y: gy }, CGMouseButton::Left,
                ).map_err(|_| "Falha MouseMove".to_string())?;
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
                let pos = get_cursor_position()
                    .map(|(x, y)| CGPoint { x, y })
                    .unwrap_or(CGPoint { x: 0.0, y: 0.0 });
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
/// Consistente com Windows (`normalize_point_against_display_rect`) e `check_boundary`.
fn normalize_mouse_standard(loc_x: f64, loc_y: f64) -> (f32, f32) {
    use core_graphics::display::CGDisplay;
    let b = CGDisplay::main().bounds();
    let w = b.size.width.max(1.0);
    let h = b.size.height.max(1.0);
    let nx = ((loc_x - b.origin.x) / w).clamp(0.0, 1.0) as f32;
    let ny = ((loc_y - b.origin.y) / h).clamp(0.0, 1.0) as f32;
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
