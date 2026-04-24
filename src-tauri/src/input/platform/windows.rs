use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tracing::{error, info};

use crate::input::events::{InputEvent, MouseButton, Modifiers};
use super::{InputCapture, InputInjector};

type HookCallback = Box<dyn Fn(InputEvent) + Send + Sync + 'static>;

// RwLock permite leituras concorrentes no hot-path do hook (centenas de vezes/s)
// e escritas exclusivas apenas durante init/shutdown.
static HOOK_CB: OnceLock<std::sync::RwLock<Option<Arc<HookCallback>>>> = OnceLock::new();

fn get_hook_cell() -> &'static std::sync::RwLock<Option<Arc<HookCallback>>> {
    HOOK_CB.get_or_init(|| std::sync::RwLock::new(None))
}

fn set_hook_cb(cb: Option<Arc<HookCallback>>) {
    if let Ok(mut w) = get_hook_cell().write() {
        *w = cb;
    }
}

static PRIMARY_BOUNDS: OnceLock<(i32, i32, u32, u32)> = OnceLock::new();

fn primary_display_bounds() -> (i32, i32, u32, u32) {
    *PRIMARY_BOUNDS.get_or_init(|| {
        let mlay = crate::screen::layout::detect_monitors();
        mlay.monitors
            .iter()
            .find(|m| m.is_primary)
            .or_else(|| mlay.monitors.first())
            .map(|m| (m.x, m.y, m.width, m.height))
            .unwrap_or((0, 0, 1920, 1080))
    })
}

fn normalize_cursor_to_primary_01(x: i32, y: i32) -> (f32, f32) {
    let (left, top, w, h) = primary_display_bounds();
    crate::screen::layout::normalize_point_against_display_rect(left, top, w, h, x, y)
}

fn call_hook_cb(event: InputEvent) {
    if let Ok(r) = get_hook_cell().read() {
        if let Some(cb) = r.as_ref() {
            cb(event);
        }
    }
}

pub struct WindowsCapture {
    running: Arc<AtomicBool>,
}

impl WindowsCapture {
    pub fn new() -> Self {
        Self { running: Arc::new(AtomicBool::new(false)) }
    }
}

impl InputCapture for WindowsCapture {
    fn start(&self, callback: Box<dyn Fn(InputEvent) + Send + Sync + 'static>) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let running = Arc::clone(&self.running);
        set_hook_cb(Some(Arc::new(callback)));

        std::thread::spawn(move || {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowsHookExW, UnhookWindowsHookEx,
                WH_MOUSE_LL, WH_KEYBOARD_LL, MSG,
                HC_ACTION,
            };
            use windows::Win32::Foundation::{LPARAM, WPARAM, LRESULT};
            use windows::Win32::System::LibraryLoader::GetModuleHandleW;
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN,
            };

            unsafe extern "system" fn mouse_proc(
                n_code: i32,
                w_param: WPARAM,
                l_param: LPARAM,
            ) -> LRESULT {
                use windows::Win32::UI::WindowsAndMessaging::{
                    CallNextHookEx, MSLLHOOKSTRUCT,
                    WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP,
                    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_MOUSEWHEEL,
                };

                if n_code == HC_ACTION as i32 {
                    let data = &*(l_param.0 as *const MSLLHOOKSTRUCT);

                    let event = match w_param.0 as u32 {
                        v if v == WM_MOUSEMOVE => {
                            let (nx, ny) = normalize_cursor_to_primary_01(data.pt.x, data.pt.y);
                            Some(InputEvent::MouseMove { x: nx, y: ny })
                        }
                        v if v == WM_LBUTTONDOWN => Some(InputEvent::MouseButton {
                            button: MouseButton::Left, pressed: true,
                        }),
                        v if v == WM_LBUTTONUP => Some(InputEvent::MouseButton {
                            button: MouseButton::Left, pressed: false,
                        }),
                        v if v == WM_RBUTTONDOWN => Some(InputEvent::MouseButton {
                            button: MouseButton::Right, pressed: true,
                        }),
                        v if v == WM_RBUTTONUP => Some(InputEvent::MouseButton {
                            button: MouseButton::Right, pressed: false,
                        }),
                        v if v == WM_MOUSEWHEEL => {
                            let delta = ((data.mouseData >> 16) as i16) as f32 / 120.0;
                            Some(InputEvent::MouseScroll { dx: 0.0, dy: delta })
                        }
                        _ => None,
                    };

                    if let Some(ev) = event {
                        call_hook_cb(ev);
                    }
                }
                CallNextHookEx(None, n_code, w_param, l_param)
            }

            unsafe extern "system" fn keyboard_proc(
                n_code: i32,
                w_param: WPARAM,
                l_param: LPARAM,
            ) -> LRESULT {
                use windows::Win32::UI::WindowsAndMessaging::{
                    CallNextHookEx, KBDLLHOOKSTRUCT,
                    WM_KEYDOWN, WM_SYSKEYDOWN,
                };

                if n_code == HC_ACTION as i32 {
                    let data = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
                    let vk = w_param.0 as u32;
                    let pressed = vk == WM_KEYDOWN || vk == WM_SYSKEYDOWN;

                    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
                    let mut mods = Modifiers::NONE;
                    if GetAsyncKeyState(VK_SHIFT.0 as i32) < 0   { mods = Modifiers(mods.0 | Modifiers::SHIFT.0); }
                    if GetAsyncKeyState(VK_CONTROL.0 as i32) < 0 { mods = Modifiers(mods.0 | Modifiers::CTRL.0); }
                    if GetAsyncKeyState(VK_MENU.0 as i32) < 0    { mods = Modifiers(mods.0 | Modifiers::ALT.0); }
                    if GetAsyncKeyState(VK_LWIN.0 as i32) < 0
                    || GetAsyncKeyState(VK_RWIN.0 as i32) < 0    { mods = Modifiers(mods.0 | Modifiers::META.0); }

                    call_hook_cb(InputEvent::KeyEvent {
                        keycode: data.vkCode,
                        pressed,
                        modifiers: mods,
                    });
                }
                CallNextHookEx(None, n_code, w_param, l_param)
            }

            unsafe {
                let hmod = GetModuleHandleW(None).unwrap_or_default();
                let mouse_hook = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hmod, 0) {
                    Ok(h) => h,
                    Err(e) => {
                        error!("WindowsCapture: falha ao instalar hook de mouse: {}", e);
                        running.store(false, Ordering::SeqCst);
                        return;
                    }
                };
                let kb_hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), hmod, 0) {
                    Ok(h) => h,
                    Err(e) => {
                        error!("WindowsCapture: falha ao instalar hook de teclado: {}", e);
                        UnhookWindowsHookEx(mouse_hook).ok();
                        running.store(false, Ordering::SeqCst);
                        return;
                    }
                };

                info!("WindowsCapture: hooks instalados");

                let mut msg = MSG::default();
                while running.load(Ordering::SeqCst) {
                    use windows::Win32::UI::WindowsAndMessaging::{
                        PeekMessageW, TranslateMessage, DispatchMessageW, PM_REMOVE,
                    };
                    if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }

                UnhookWindowsHookEx(mouse_hook).ok();
                UnhookWindowsHookEx(kb_hook).ok();
                info!("WindowsCapture: hooks removidos");
            }
        });

        info!("WindowsCapture: captura iniciada");
        Ok(())
    }

    fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        set_hook_cb(None);
    }

    fn lock_cursor(&self) {
        if let Ok((x, y)) = get_cursor_pos_win() {
            unsafe {
                use windows::Win32::Foundation::RECT;
                use windows::Win32::UI::WindowsAndMessaging::ClipCursor;
                let rect = RECT { left: x, top: y, right: x + 1, bottom: y + 1 };
                ClipCursor(Some(&rect)).ok();
            }
        }
        info!("WindowsCapture: cursor bloqueado");
    }

    fn unlock_cursor(&self) {
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::ClipCursor;
            ClipCursor(None).ok();
        }
        info!("WindowsCapture: cursor liberado");
    }
}

pub struct WindowsInjector;

impl WindowsInjector {
    pub fn new() -> Self { Self }
}

impl InputInjector for WindowsInjector {
    fn inject(&self, event: InputEvent) -> Result<(), String> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, MOUSEINPUT,
            INPUT_MOUSE,
            MOUSEEVENTF_MOVE, MOUSEEVENTF_ABSOLUTE,
            MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
            MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
            MOUSEEVENTF_WHEEL,
        };

        let inputs: Vec<INPUT> = match event {
            InputEvent::MouseMove { x, y } => {
                let ax = (x * 65535.0) as i32;
                let ay = (y * 65535.0) as i32;
                vec![INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: ax,
                            dy: ay,
                            mouseData: 0,
                            dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                }]
            }
            InputEvent::MouseButton { button, pressed } => {
                let flags = match (button, pressed) {
                    (MouseButton::Left,  true)  => MOUSEEVENTF_LEFTDOWN,
                    (MouseButton::Left,  false) => MOUSEEVENTF_LEFTUP,
                    (MouseButton::Right, true)  => MOUSEEVENTF_RIGHTDOWN,
                    (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
                    _ => return Ok(()),
                };
                vec![INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0, dy: 0,
                            mouseData: 0,
                            dwFlags: flags,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                }]
            }
            InputEvent::MouseScroll { dy, .. } => {
                let wheel = (dy * 120.0) as i32;
                vec![INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0, dy: 0,
                            mouseData: wheel as u32,
                            dwFlags: MOUSEEVENTF_WHEEL,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                }]
            }
            InputEvent::KeyEvent { keycode, pressed, modifiers } => {
                let mut inputs = vec![];

                let mod_keys = [
                    (Modifiers::SHIFT, 0x10u16),
                    (Modifiers::CTRL,  0x11u16),
                    (Modifiers::ALT,   0x12u16),
                    (Modifiers::META,  0x5Bu16),
                ];

                if pressed {
                    for (m, vk) in &mod_keys {
                        if modifiers.contains(*m) {
                            inputs.push(make_key_input(*vk, false));
                        }
                    }
                }

                inputs.push(make_key_input(keycode as u16, !pressed));

                if !pressed {
                    for (m, vk) in &mod_keys {
                        if modifiers.contains(*m) {
                            inputs.push(make_key_input(*vk, true));
                        }
                    }
                }

                inputs
            }
        };

        if inputs.is_empty() { return Ok(()); }

        unsafe {
            let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            if sent != inputs.len() as u32 {
                return Err(format!("SendInput enviou {}/{} inputs", sent, inputs.len()));
            }
        }
        Ok(())
    }
}

fn make_key_input(vk: u16, key_up: bool) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, KEYBDINPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP,
    };
    let flags = if key_up { KEYEVENTF_KEYUP } else { Default::default() };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn get_cursor_pos_win() -> Result<(i32, i32), ()> {
    unsafe {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).map_err(|_| ())?;
        Ok((pt.x, pt.y))
    }
}
