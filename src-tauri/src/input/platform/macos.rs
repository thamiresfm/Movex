use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info, warn};

use crate::input::events::{InputEvent, MouseButton, Modifiers};
use super::{InputCapture, InputInjector};

pub struct MacOsCapture {
    locked:   Arc<Mutex<bool>>,
    lock_pos: Arc<Mutex<(f64, f64)>>,
    running:  Arc<AtomicBool>,
}

impl MacOsCapture {
    pub fn new() -> Self {
        Self {
            locked:   Arc::new(Mutex::new(false)),
            lock_pos: Arc::new(Mutex::new((0.0, 0.0))),
            running:  Arc::new(AtomicBool::new(false)),
        }
    }
}

impl InputCapture for MacOsCapture {
    fn start(&self, callback: Box<dyn Fn(InputEvent) + Send + Sync + 'static>) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let locked  = Arc::clone(&self.locked);
        let lock_pos = Arc::clone(&self.lock_pos);
        let running = Arc::clone(&self.running);

        std::thread::spawn(move || {
            use core_graphics::event::{
                CGEventTap, CGEventTapLocation, CGEventTapPlacement,
                CGEventTapOptions, CGEventType,
            };

            let callback = Arc::new(callback);
            let locked_inner   = Arc::clone(&locked);
            let lock_pos_inner = Arc::clone(&lock_pos);

            let tap_result = CGEventTap::new(
                CGEventTapLocation::HID,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                vec![
                    CGEventType::MouseMoved,
                    CGEventType::LeftMouseDown,
                    CGEventType::LeftMouseUp,
                    CGEventType::RightMouseDown,
                    CGEventType::RightMouseUp,
                    CGEventType::ScrollWheel,
                    CGEventType::KeyDown,
                    CGEventType::KeyUp,
                ],
                move |_proxy, event_type, event| {
                    // Cursor bloqueado na borda → travar posição e bloquear eventos de mouse
                    if *locked_inner.lock().unwrap() {
                        match event_type {
                            CGEventType::MouseMoved
                            | CGEventType::LeftMouseDown
                            | CGEventType::LeftMouseUp
                            | CGEventType::RightMouseDown
                            | CGEventType::RightMouseUp => {
                                let (lx, ly) = *lock_pos_inner.lock().unwrap();
                                use core_graphics::geometry::CGPoint;
                                let _ = core_graphics::display::CGDisplay::warp_mouse_cursor_position(
                                    CGPoint { x: lx, y: ly }
                                );
                                return None;
                            }
                            _ => {}
                        }
                    }

                    let (screen_w, screen_h) = get_screen_size();
                    let input = match event_type {
                        CGEventType::MouseMoved => {
                            let loc = event.location();
                            Some(InputEvent::MouseMove {
                                x: (loc.x / screen_w) as f32,
                                y: (loc.y / screen_h) as f32,
                            })
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
                            if flags.contains(CGEventFlags::CGEventFlagShift)   { mods = Modifiers(mods.0 | Modifiers::SHIFT.0); }
                            if flags.contains(CGEventFlags::CGEventFlagControl) { mods = Modifiers(mods.0 | Modifiers::CTRL.0); }
                            if flags.contains(CGEventFlags::CGEventFlagAlternate){ mods = Modifiers(mods.0 | Modifiers::ALT.0); }
                            if flags.contains(CGEventFlags::CGEventFlagCommand) { mods = Modifiers(mods.0 | Modifiers::META.0); }
                            let pressed = matches!(event_type, CGEventType::KeyDown);
                            Some(InputEvent::KeyEvent {
                                keycode,
                                pressed,
                                modifiers: mods,
                            })
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
                    let loop_src = tap.mach_port.create_runloop_source(0)
                        .expect("RunLoopSource");
                    let run_loop = unsafe { core_foundation::runloop::CFRunLoop::get_current() };
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

    fn lock_cursor(&self) {
        if let Ok(pos) = get_cursor_position() {
            *self.lock_pos.lock().unwrap() = pos;
        }
        *self.locked.lock().unwrap() = true;
        info!("MacOsCapture: cursor bloqueado");
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

        let (sw, sh) = get_screen_size();

        match event {
            InputEvent::MouseMove { x, y } => {
                let pt = CGPoint { x: x as f64 * sw, y: y as f64 * sh };
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
                // Usar mouse event com delta para scroll (API simplificada)
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

fn get_screen_size() -> (f64, f64) {
    use core_graphics::display::CGDisplay;
    let d = CGDisplay::main();
    (d.pixels_wide() as f64, d.pixels_high() as f64)
}

fn get_cursor_position() -> Result<(f64, f64), ()> {
    use core_graphics::event::{CGEvent, CGEventType};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    // Posição do cursor via evento dummy
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).map_err(|_| ())?;
    let ev = CGEvent::new(source).map_err(|_| ())?;
    let loc = ev.location();
    Ok((loc.x, loc.y))
}
