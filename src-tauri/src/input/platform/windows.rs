/// Captura e injecção de eventos de rato/teclado no Windows.
///
/// # Padrão Barrier/Input-Leap — portado do macOS
///
/// O mesmo problema que afectava o macOS existe aqui:
/// - `ClipCursor` na borda → o sistema recalcula cada movimento a partir da posição clampada
///   → todos os eventos WM_MOUSEMOVE reportam a mesma posição → cursor remoto imóvel.
///
/// Solução (idêntica ao Barrier/OSXScreen e ao nosso `macos.rs`):
/// 1. `lock_cursor`: chama `SetCursorPos(cx, cy)` para o CENTRO do ecrã.
///    Actualizar `WIN_PREV_{X,Y} = (cx, cy)` ANTES do warp para que o evento
///    sintético gerado pelo SetCursorPos tenha `delta = 0`.
/// 2. `mouse_proc` locked: `dx = pt.x - prev_x`, depois `prev = center`, depois warp.
///    O próximo evento sintético tem `dx = 0` → skip automático.
/// 3. Filtro "bogus zone": descartar deltas > 45% da largura/altura.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::OnceLock;
use tracing::{error, info};

use crate::input::events::{InputEvent, MouseButton, Modifiers};
use super::{InputCapture, InputInjector};

type HookCallback = Box<dyn Fn(InputEvent) + Send + Sync + 'static>;

// ── Callback do hook ───────────────────────────────────────────────────────────

static HOOK_CB: OnceLock<std::sync::RwLock<Option<std::sync::Arc<HookCallback>>>> = OnceLock::new();

fn get_hook_cell() -> &'static std::sync::RwLock<Option<std::sync::Arc<HookCallback>>> {
    HOOK_CB.get_or_init(|| std::sync::RwLock::new(None))
}

fn set_hook_cb(cb: Option<std::sync::Arc<HookCallback>>) {
    if let Ok(mut w) = get_hook_cell().write() {
        *w = cb;
    }
}

fn call_hook_cb(event: InputEvent) {
    if let Ok(r) = get_hook_cell().read() {
        if let Some(cb) = r.as_ref() {
            cb(event);
        }
    }
}

// ── Estado de captura do display primário ─────────────────────────────────────

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

fn normalize_cursor_virtual_desktop_01(x: i32, y: i32) -> (f32, f32) {
    let (left, top, w, h) = crate::screen::layout::desktop_bounding_box_cached();
    crate::screen::layout::normalize_point_against_display_rect(left, top, w, h, x, y)
}

// ── Estado Barrier (padrão idêntico ao macos.rs) ──────────────────────────────

/// Indica se o cursor está bloqueado para controlo remoto (servidor em modo remoto).
static WIN_CURSOR_LOCKED: AtomicBool = AtomicBool::new(false);

/// Centro do display primário em pixels lógicos — destino do warp.
static WIN_CENTER_X: AtomicI32 = AtomicI32::new(0);
static WIN_CENTER_Y: AtomicI32 = AtomicI32::new(0);

/// Última posição conhecida — actualizado para `center` ANTES de cada warp,
/// de modo a que o evento sintético do SetCursorPos tenha delta = 0.
static WIN_PREV_X: AtomicI32 = AtomicI32::new(0);
static WIN_PREV_Y: AtomicI32 = AtomicI32::new(0);

/// Posição virtual do cursor no ecrã remoto (bits de f32 guardados em AtomicU32).
static WIN_VIRT_X_BITS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static WIN_VIRT_Y_BITS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[inline]
fn virt_pos_load() -> (f32, f32) {
    let x = f32::from_bits(WIN_VIRT_X_BITS.load(Ordering::Acquire));
    let y = f32::from_bits(WIN_VIRT_Y_BITS.load(Ordering::Acquire));
    (x, y)
}

#[inline]
fn virt_pos_store(x: f32, y: f32) {
    WIN_VIRT_X_BITS.store(x.to_bits(), Ordering::Release);
    WIN_VIRT_Y_BITS.store(y.to_bits(), Ordering::Release);
}

// ── WindowsCapture ────────────────────────────────────────────────────────────

pub struct WindowsCapture {
    running: std::sync::Arc<AtomicBool>,
}

impl WindowsCapture {
    pub fn new() -> Self {
        Self { running: std::sync::Arc::new(AtomicBool::new(false)) }
    }
}

impl InputCapture for WindowsCapture {
    fn start(&self, callback: Box<dyn Fn(InputEvent) + Send + Sync + 'static>) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let running = std::sync::Arc::clone(&self.running);
        set_hook_cb(Some(std::sync::Arc::new(callback)));

        // Canal síncrono usado pelo thread para reportar o resultado da instalação dos
        // hooks. Sem isto, falhas de SetWindowsHookExW (UAC, KVM concorrente) ficam
        // silenciosas — captura aparenta "ligada" mas o servidor nunca recebe eventos.
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

        std::thread::spawn(move || {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowsHookExW, UnhookWindowsHookEx,
                WH_MOUSE_LL, WH_KEYBOARD_LL, MSG, HC_ACTION,
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
                    let msg_type = w_param.0 as u32;

                    // ── Modo bloqueado (cursor no PC remoto) ───────────────────────────
                    if WIN_CURSOR_LOCKED.load(Ordering::Acquire) {
                        let event = match msg_type {
                            v if v == WM_MOUSEMOVE => {
                                // Ler prev_pos e centro
                                let prev_x = WIN_PREV_X.load(Ordering::Acquire);
                                let prev_y = WIN_PREV_Y.load(Ordering::Acquire);
                                let cx = WIN_CENTER_X.load(Ordering::Acquire);
                                let cy = WIN_CENTER_Y.load(Ordering::Acquire);

                                let dx = (data.pt.x - prev_x) as f32;
                                let dy = (data.pt.y - prev_y) as f32;

                                // Actualizar prev_pos para centro ANTES do warp (padrão Barrier)
                                WIN_PREV_X.store(cx, Ordering::Release);
                                WIN_PREV_Y.store(cy, Ordering::Release);

                                // Warp para centro
                                use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
                                let _ = SetCursorPos(cx, cy);

                                // Filtrar evento sintético do warp (delta ≈ 0)
                                if dx.abs() < 0.5 && dy.abs() < 0.5 {
                                    return CallNextHookEx(None, n_code, w_param, l_param);
                                }

                                // Filtro bogus + normalização do delta: usar o rect virtual completo,
                                // coerente com `check_boundary` no servidor (`ScreenLayout` bbox).
                                let (_, _, w, h) = crate::screen::layout::desktop_bounding_box_cached();
                                if dx.abs() > w as f32 * 0.45 || dy.abs() > h as f32 * 0.45 {
                                    return CallNextHookEx(None, n_code, w_param, l_param);
                                }

                                // Acumular delta normalizado na posição virtual remota
                                let (vx, vy) = virt_pos_load();
                                let (nw, nh) = (w as f32, h as f32);
                                let new_vx = (vx + dx / nw).clamp(0.0, 1.0);
                                let new_vy = (vy + dy / nh).clamp(0.0, 1.0);
                                virt_pos_store(new_vx, new_vy);

                                Some(InputEvent::MouseMove { x: new_vx, y: new_vy })
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
                            _ => None,
                        };
                        if let Some(ev) = event {
                            call_hook_cb(ev);
                        }
                        // Suprimir evento local enquanto bloqueado: retornar LRESULT
                        // não-zero faz o Windows DESCARTAR o evento (o cursor não se
                        // mexe nesta máquina). Antes chamávamos CallNextHookEx, o que
                        // passava o evento adiante e o cursor local andava em paralelo
                        // com o cursor remoto — sintoma "duplicado/espelhado" que o
                        // utilizador relatou.
                        //
                        // Os eventos sintéticos do SetCursorPos (delta ≈ 0) já saem
                        // antes via early return com CallNextHookEx (acima), portanto
                        // o warp para o centro continua a funcionar.
                        return LRESULT(1);
                    }

                    // ── Modo local (cursor neste PC) ───────────────────────────────────
                    let event = match msg_type {
                            v if v == WM_MOUSEMOVE => {
                            let (nx, ny) = normalize_cursor_virtual_desktop_01(data.pt.x, data.pt.y);
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
                        let msg = format!("falha ao instalar WH_MOUSE_LL: {} (outro KVM em execução? tentar como Administrador)", e);
                        error!("WindowsCapture: {}", msg);
                        running.store(false, Ordering::SeqCst);
                        let _ = init_tx.send(Err(msg));
                        return;
                    }
                };
                let kb_hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), hmod, 0) {
                    Ok(h) => h,
                    Err(e) => {
                        let msg = format!("falha ao instalar WH_KEYBOARD_LL: {}", e);
                        error!("WindowsCapture: {}", msg);
                        UnhookWindowsHookEx(mouse_hook).ok();
                        running.store(false, Ordering::SeqCst);
                        let _ = init_tx.send(Err(msg));
                        return;
                    }
                };

                info!("WindowsCapture: hooks instalados (padrão Barrier)");
                let _ = init_tx.send(Ok(()));

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

        // Aguardar até 3 s pelo resultado da instalação dos hooks.
        match init_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(Ok(())) => {
                info!("WindowsCapture: captura iniciada");
                Ok(())
            }
            Ok(Err(msg)) => Err(msg),
            Err(_) => Err("timeout a aguardar instalação dos hooks Windows".to_string()),
        }
    }

    fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        set_hook_cb(None);
    }

    /// Travar o cursor — padrão Barrier (idêntico ao macOS):
    /// 1. Warp para o CENTRO do ecrã (nunca fica clampado na borda).
    /// 2. Actualizar `WIN_PREV_{X,Y} = center` ANTES do warp para que o evento
    ///    sintético gerado pelo SetCursorPos tenha `delta = 0` → descartado.
    fn lock_cursor(&self, entry_x: f32, entry_y: f32) {
        let (left, top, w, h) = primary_display_bounds();
        let cx = left + w as i32 / 2;
        let cy = top  + h as i32 / 2;

        WIN_CENTER_X.store(cx, Ordering::Release);
        WIN_CENTER_Y.store(cy, Ordering::Release);

        // Actualizar prev_pos para centro ANTES do warp (padrão Barrier!)
        WIN_PREV_X.store(cx, Ordering::Release);
        WIN_PREV_Y.store(cy, Ordering::Release);

        // Definir posição virtual de entrada no ecrã remoto
        virt_pos_store(entry_x, entry_y);

        // Activar modo bloqueado
        WIN_CURSOR_LOCKED.store(true, Ordering::Release);

        // Warp para centro e remover restrição de ClipCursor (se existir)
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{SetCursorPos, ClipCursor};
            ClipCursor(None).ok();
            let _ = SetCursorPos(cx, cy);
        }

        info!(
            "WindowsCapture: locked → warp para centro ({},{}) entry=({:.3},{:.3})",
            cx, cy, entry_x, entry_y
        );
    }

    fn unlock_cursor(&self) {
        WIN_CURSOR_LOCKED.store(false, Ordering::Release);
        info!("WindowsCapture: unlocked");
    }
}

// ── WindowsInjector ───────────────────────────────────────────────────────────

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
