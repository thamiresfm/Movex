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
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tracing::{error, info};

// ── Sensibilidade do cursor (lado receptor / injector) ─────────────────────────

// 1.2f32 em IEEE 754 single-precision = 0x3F99999A
static MOUSE_SENS: AtomicU32 = AtomicU32::new(0x3F99999A);

// Última posição virtual (0..1) injectada. u32::MAX = não inicializada.
static INJ_PREV_X: AtomicU32 = AtomicU32::new(u32::MAX);
static INJ_PREV_Y: AtomicU32 = AtomicU32::new(u32::MAX);

/// Atualiza a sensibilidade do cursor e reinicia o tracking de posição
/// para que o próximo evento use posicionamento absoluto (sem salto).
pub fn set_sensitivity(s: f32) {
    MOUSE_SENS.store(s.clamp(0.1, 5.0).to_bits(), Ordering::Release);
    INJ_PREV_X.store(u32::MAX, Ordering::Release);
    INJ_PREV_Y.store(u32::MAX, Ordering::Release);
}

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
    /// Posição do cursor IMEDIATAMENTE ANTES do `lock_cursor` (= antes de
    /// cruzar a borda). Usado por `unlock_cursor` para restaurar o cursor
    /// onde o utilizador o "deixou" — evita o sintoma "cursor reaparece no
    /// centro do ecrã" que parece "preso/agarrando".
    pre_lock: Arc<Mutex<Option<(f64, f64)>>>,
}

impl MacOsCapture {
    pub fn new() -> Self {
        Self {
            locked:   Arc::new(Mutex::new(false)),
            center:   Arc::new(Mutex::new((0.0, 0.0))),
            prev_pos: Arc::new(Mutex::new((0.0, 0.0))),
            running:  Arc::new(AtomicBool::new(false)),
            virt_pos: Arc::new(Mutex::new((0.5, 0.5))),
            pre_lock: Arc::new(Mutex::new(None)),
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

        // Canal síncrono usado pelo thread para reportar o resultado da criação do
        // CGEventTap. Sem isto, a falha fica silenciosa quando Acessibilidade está
        // desligada: a captura "está iniciada" mas nunca dispara nenhum evento.
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

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
                    CGEventType::FlagsChanged,       // modificadores (Shift/Ctrl/Alt/Cmd)
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

                                let (_, _, bw, bh) =
                                    crate::screen::layout::desktop_bounding_box_cached();
                                let w = bw as f32;
                                let h = bh as f32;
                                let w_nonempty = w.max(1.0);
                                let h_nonempty = h.max(1.0);
                                if dx.abs() > w_nonempty * 0.45 || dy.abs() > h_nonempty * 0.45 {
                                    return None;
                                }

                                // Acumular delta normalizado na posição virtual remota.
                                // Dividir por metade da bbox: o cursor está no centro do servidor,
                                // com apenas metade do ecrã disponível em cada direção.
                                // Usar bbox_w inteiro dava virt_pos_max=0.5 (parede no meio do ecrã remoto).
                                let (vx, vy) = {
                                    let mut vp = virt_pos_c.lock().unwrap_or_else(|p| p.into_inner());
                                    vp.0 = (vp.0 + dx / (w_nonempty * 0.5)).clamp(0.0, 1.0);
                                    vp.1 = (vp.1 + dy / (h_nonempty * 0.5)).clamp(0.0, 1.0);
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
                            // Teclado encaminhado para o PC remoto e SUPRIMIDO localmente
                            // (returning None) para que a tecla não seja digitada nas duas
                            // máquinas em simultâneo. Sem isto, o utilizador pressionaria
                            // 'a' no teclado físico e via 'aa' aparecer (uma local + uma
                            // remota injetada de volta via TCP).
                            CGEventType::KeyDown | CGEventType::KeyUp => {
                                use core_graphics::event::{EventField, CGEventFlags};
                                let mac_kc = event.get_integer_value_field(
                                    EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                                let hid = crate::input::keycodes::mac_to_hid(mac_kc)?;
                                let flags = event.get_flags();
                                let mut mods = Modifiers::NONE;
                                if flags.contains(CGEventFlags::CGEventFlagShift)     { mods = Modifiers(mods.0 | Modifiers::SHIFT.0); }
                                if flags.contains(CGEventFlags::CGEventFlagControl)   { mods = Modifiers(mods.0 | Modifiers::CTRL.0); }
                                if flags.contains(CGEventFlags::CGEventFlagAlternate) { mods = Modifiers(mods.0 | Modifiers::ALT.0); }
                                if flags.contains(CGEventFlags::CGEventFlagCommand)   { mods = Modifiers(mods.0 | Modifiers::META.0); }
                                let pressed = matches!(event_type, CGEventType::KeyDown);
                                callback(InputEvent::KeyEvent { keycode: hid, pressed, modifiers: mods });
                                return None;
                            }
                            // Modificadores (Shift/Ctrl/Alt/Cmd) geram FlagsChanged, não KeyDown/KeyUp.
                            // Sem este handler ficam presos no `_ => {}` e executam localmente em vez
                            // de serem encaminhados para o PC remoto.
                            CGEventType::FlagsChanged => {
                                use core_graphics::event::{EventField, CGEventFlags};
                                let mac_kc = event.get_integer_value_field(
                                    EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                                let Some(hid) = crate::input::keycodes::mac_to_hid(mac_kc) else {
                                    return None;
                                };
                                let flags = event.get_flags();
                                let mod_bit: u64 = match mac_kc {
                                    0x38 | 0x3C => CGEventFlags::CGEventFlagShift.bits(),
                                    0x3B | 0x3E => CGEventFlags::CGEventFlagControl.bits(),
                                    0x3A | 0x3D => CGEventFlags::CGEventFlagAlternate.bits(),
                                    0x37 | 0x36 => CGEventFlags::CGEventFlagCommand.bits(),
                                    _ => { return None; }
                                };
                                let pressed = flags.bits() & mod_bit != 0;
                                let mut mods = Modifiers::NONE;
                                if flags.contains(CGEventFlags::CGEventFlagShift)     { mods = Modifiers(mods.0 | Modifiers::SHIFT.0); }
                                if flags.contains(CGEventFlags::CGEventFlagControl)   { mods = Modifiers(mods.0 | Modifiers::CTRL.0); }
                                if flags.contains(CGEventFlags::CGEventFlagAlternate) { mods = Modifiers(mods.0 | Modifiers::ALT.0); }
                                if flags.contains(CGEventFlags::CGEventFlagCommand)   { mods = Modifiers(mods.0 | Modifiers::META.0); }
                                callback(InputEvent::KeyEvent { keycode: hid, pressed, modifiers: mods });
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
                            let mac_kc = event.get_integer_value_field(
                                EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                            // Converter mac → HID. Sem mapa, descartar.
                            let hid = crate::input::keycodes::mac_to_hid(mac_kc)?;
                            let flags = event.get_flags();
                            let mut mods = Modifiers::NONE;
                            if flags.contains(CGEventFlags::CGEventFlagShift)     { mods = Modifiers(mods.0 | Modifiers::SHIFT.0); }
                            if flags.contains(CGEventFlags::CGEventFlagControl)   { mods = Modifiers(mods.0 | Modifiers::CTRL.0); }
                            if flags.contains(CGEventFlags::CGEventFlagAlternate) { mods = Modifiers(mods.0 | Modifiers::ALT.0); }
                            if flags.contains(CGEventFlags::CGEventFlagCommand)   { mods = Modifiers(mods.0 | Modifiers::META.0); }
                            let pressed = matches!(event_type, CGEventType::KeyDown);
                            Some(InputEvent::KeyEvent { keycode: hid, pressed, modifiers: mods })
                        }
                        CGEventType::FlagsChanged => {
                            use core_graphics::event::{EventField, CGEventFlags};
                            let mac_kc = event.get_integer_value_field(
                                EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                            let hid = match crate::input::keycodes::mac_to_hid(mac_kc) {
                                Some(h) => h,
                                None => return Some(event.to_owned()),
                            };
                            let flags = event.get_flags();
                            let mod_bit: u64 = match mac_kc {
                                0x38 | 0x3C => CGEventFlags::CGEventFlagShift.bits(),
                                0x3B | 0x3E => CGEventFlags::CGEventFlagControl.bits(),
                                0x3A | 0x3D => CGEventFlags::CGEventFlagAlternate.bits(),
                                0x37 | 0x36 => CGEventFlags::CGEventFlagCommand.bits(),
                                _ => return Some(event.to_owned()),
                            };
                            let pressed = flags.bits() & mod_bit != 0;
                            let mut mods = Modifiers::NONE;
                            if flags.contains(CGEventFlags::CGEventFlagShift)     { mods = Modifiers(mods.0 | Modifiers::SHIFT.0); }
                            if flags.contains(CGEventFlags::CGEventFlagControl)   { mods = Modifiers(mods.0 | Modifiers::CTRL.0); }
                            if flags.contains(CGEventFlags::CGEventFlagAlternate) { mods = Modifiers(mods.0 | Modifiers::ALT.0); }
                            if flags.contains(CGEventFlags::CGEventFlagCommand)   { mods = Modifiers(mods.0 | Modifiers::META.0); }
                            Some(InputEvent::KeyEvent { keycode: hid, pressed, modifiers: mods })
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
                            let msg = "falha ao criar RunLoopSource".to_string();
                            error!("MacOsCapture: {}", msg);
                            running.store(false, Ordering::SeqCst);
                            let _ = init_tx.send(Err(msg));
                            return;
                        }
                    };
                    let run_loop = core_foundation::runloop::CFRunLoop::get_current();
                    run_loop.add_source(&loop_src, unsafe {
                        core_foundation::runloop::kCFRunLoopDefaultMode
                    });
                    tap.enable();
                    let _ = init_tx.send(Ok(()));
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
                    let msg = format!("CGEventTap falhou: {:?} — verifique permissão de Acessibilidade", e);
                    error!("{}", msg);
                    running.store(false, Ordering::SeqCst);
                    let _ = init_tx.send(Err(msg));
                }
            }
        });

        // Aguardar até 3 s pela criação do CGEventTap.
        match init_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(Ok(())) => {
                info!("MacOsCapture: captura iniciada");
                Ok(())
            }
            Ok(Err(msg)) => Err(msg),
            Err(_) => Err("timeout a aguardar criação do CGEventTap".to_string()),
        }
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

        // Capturar a posição actual do cursor ANTES do warp — o unlock_cursor
        // vai restaurar o cursor para esta posição.
        if let Ok((px, py)) = get_cursor_position() {
            *self.pre_lock.lock().unwrap_or_else(|p| p.into_inner()) = Some((px, py));
        }

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

        // Restaurar cursor para a posição pré-lock, com margem de 80 px da
        // borda — evita o "mouse agarrando" em que o cursor reaparece no
        // centro depois do utilizador voltar do PC remoto.
        let pre = self.pre_lock.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some((pre_x, pre_y)) = pre {
            use core_graphics::display::CGDisplay;
            let b = CGDisplay::main().bounds();
            const MARGIN: f64 = 80.0;
            let min_x = b.origin.x + MARGIN;
            let min_y = b.origin.y + MARGIN;
            let max_x = b.origin.x + b.size.width  - MARGIN;
            let max_y = b.origin.y + b.size.height - MARGIN;
            let x = pre_x.clamp(min_x, max_x);
            let y = pre_y.clamp(min_y, max_y);
            *self.prev_pos.lock().unwrap_or_else(|p| p.into_inner()) = (x, y);
            let _ = CGDisplay::warp_mouse_cursor_position(
                core_graphics::geometry::CGPoint { x, y }
            );
            info!(
                "MacOsCapture: unlocked → restaurado para ({:.0},{:.0}) (pre-lock + margem)",
                x, y
            );
        } else {
            info!("MacOsCapture: unlocked (sem pre-lock guardado)");
        }
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
                use core_graphics::display::CGDisplay;
                use core_graphics::event::EventField;
                let b = CGDisplay::main().bounds();

                let prev_x_bits = INJ_PREV_X.load(Ordering::Acquire);
                let prev_y_bits = INJ_PREV_Y.load(Ordering::Acquire);
                INJ_PREV_X.store(x.to_bits(), Ordering::Release);
                INJ_PREV_Y.store(y.to_bits(), Ordering::Release);

                let (gx, gy, dx_px, dy_px) = if prev_x_bits == u32::MAX {
                    // Primeira injeção ou reset: posicionar absolutamente
                    let gx = b.origin.x + x as f64 * b.size.width;
                    let gy = b.origin.y + y as f64 * b.size.height;
                    (gx, gy, 0i64, 0i64)
                } else {
                    let prev_x = f32::from_bits(prev_x_bits) as f64;
                    let prev_y = f32::from_bits(prev_y_bits) as f64;
                    let dx_virt = x as f64 - prev_x;
                    let dy_virt = y as f64 - prev_y;

                    // Salto grande = cursor acabou de entrar neste ecrã → posicionar absolutamente
                    if dx_virt.abs() > 0.2 || dy_virt.abs() > 0.2 {
                        let gx = b.origin.x + x as f64 * b.size.width;
                        let gy = b.origin.y + y as f64 * b.size.height;
                        (gx, gy, 0i64, 0i64)
                    } else {
                        // Movimento normal: aplicar sensibilidade e injetar relativamente
                        // ao cursor actual para evitar que tamanhos diferentes de ecrã
                        // façam o cursor parecer "pesado".
                        let sens = f32::from_bits(MOUSE_SENS.load(Ordering::Acquire)) as f64;
                        let dx_px = (dx_virt * b.size.width * sens).round() as i64;
                        let dy_px = (dy_virt * b.size.height * sens).round() as i64;
                        let (cur_x, cur_y) = get_cursor_position().unwrap_or((
                            b.origin.x + prev_x * b.size.width,
                            b.origin.y + prev_y * b.size.height,
                        ));
                        let gx = (cur_x + dx_px as f64)
                            .clamp(b.origin.x, b.origin.x + b.size.width  - 1.0);
                        let gy = (cur_y + dy_px as f64)
                            .clamp(b.origin.y, b.origin.y + b.size.height - 1.0);
                        (gx, gy, dx_px, dy_px)
                    }
                };

                let ev = CGEvent::new_mouse_event(
                    source, CGEventType::MouseMoved,
                    CGPoint { x: gx, y: gy }, CGMouseButton::Left,
                ).map_err(|_| "Falha MouseMove".to_string())?;
                // Delta informativo para apps que consumam kCGMouseEventDeltaX/Y
                if dx_px != 0 || dy_px != 0 {
                    ev.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, dx_px);
                    ev.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, dy_px);
                }
                // Session em vez de HID: bypassa os hardware taps (incluindo o nosso
                // CGEventTap) e evita feedback loop com o MacOsCapture.
                ev.post(core_graphics::event::CGEventTapLocation::Session);
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
                // Converter HID → keycode macOS. Sem mapa, descartar para não
                // chamar CGEvent com keycode arbitrário (poderia digitar lixo).
                let Some(mac_kc) = crate::input::keycodes::hid_to_mac(keycode) else {
                    return Ok(());
                };
                // Teclado usa CombinedSessionState + Session: em macOS Catalina+ a
                // injeção de teclado via HIDSystemState+HID pode ser descartada
                // silenciosamente mesmo com Acessibilidade concedida. Session é o
                // nível correto para injeção sintética de teclas.
                use core_graphics::event_source::CGEventSourceStateID;
                let kb_source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
                    .or_else(|_| CGEventSource::new(CGEventSourceStateID::HIDSystemState))
                    .map_err(|_| "Falha ao criar CGEventSource para teclado".to_string())?;
                let ev = CGEvent::new_keyboard_event(kb_source, mac_kc, pressed)
                    .map_err(|_| "Falha KeyEvent".to_string())?;
                let mut flags = CGEventFlags::empty();
                if modifiers.contains(Modifiers::SHIFT) { flags |= CGEventFlags::CGEventFlagShift; }
                if modifiers.contains(Modifiers::CTRL)  { flags |= CGEventFlags::CGEventFlagControl; }
                if modifiers.contains(Modifiers::ALT)   { flags |= CGEventFlags::CGEventFlagAlternate; }
                if modifiers.contains(Modifiers::META)  { flags |= CGEventFlags::CGEventFlagCommand; }
                ev.set_flags(flags);
                ev.post(core_graphics::event::CGEventTapLocation::Session);
            }
        }
        Ok(())
    }
}

/// Normaliza coordenadas absolutas Quartz no rect virtual completo (*bounding box*
/// de todos os monitores), coerente com `server.rs`/`check_boundary` e com Windows.
fn normalize_mouse_standard(loc_x: f64, loc_y: f64) -> (f32, f32) {
    let (left, top, w, h) = crate::screen::layout::desktop_bounding_box_cached();
    crate::screen::layout::normalize_point_against_display_rect(
        left, top, w, h, loc_x as i32, loc_y as i32,
    )
}

fn get_cursor_position() -> Result<(f64, f64), ()> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).map_err(|_| ())?;
    let ev = CGEvent::new(source).map_err(|_| ())?;
    let loc = ev.location();
    Ok((loc.x, loc.y))
}
