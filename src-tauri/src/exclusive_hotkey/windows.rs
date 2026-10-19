use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::AppHandle;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
    PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use super::normalize_exclusive_label;
use crate::dictation::toggle_dictation;

static TARGET_VK: AtomicU32 = AtomicU32::new(0);
static MOD_HELD: AtomicBool = AtomicBool::new(false);
static CHORD_USED: AtomicBool = AtomicBool::new(false);
static SWALLOWED_DOWN: AtomicBool = AtomicBool::new(false);
static TOGGLE_LATCH: AtomicBool = AtomicBool::new(false);
static EXCLUSIVE_APP: Mutex<Option<AppHandle>> = Mutex::new(None);

struct HookThread {
    thread_id: u32,
}

static HOOK_THREAD: Mutex<Option<HookThread>> = Mutex::new(None);

pub fn is_exclusive_accel(accel: &str) -> bool {
    normalize_exclusive_label(accel).is_some()
}

fn vk_for_accel(accel: &str) -> Option<u32> {
    match normalize_exclusive_label(accel)? {
        "AltRight" => Some(VK_RMENU.0 as u32),
        "AltLeft" => Some(VK_LMENU.0 as u32),
        "ControlRight" => Some(VK_RCONTROL.0 as u32),
        "ControlLeft" => Some(VK_LCONTROL.0 as u32),
        "ShiftRight" => Some(VK_RSHIFT.0 as u32),
        "ShiftLeft" => Some(VK_LSHIFT.0 as u32),
        _ => None,
    }
}

pub fn apply_exclusive(app: &AppHandle, accel: &str) -> Result<(), String> {
    let vk = vk_for_accel(accel).ok_or_else(|| format!("不支持的独占键：{accel}"))?;
    *EXCLUSIVE_APP.lock().map_err(|e| e.to_string())? = Some(app.clone());
    TARGET_VK.store(vk, Ordering::SeqCst);
    MOD_HELD.store(false, Ordering::SeqCst);
    CHORD_USED.store(false, Ordering::SeqCst);
    SWALLOWED_DOWN.store(false, Ordering::SeqCst);
    TOGGLE_LATCH.store(false, Ordering::SeqCst);
    ensure_hook_thread()?;
    Ok(())
}

pub fn clear_exclusive() -> Result<(), String> {
    TARGET_VK.store(0, Ordering::SeqCst);
    MOD_HELD.store(false, Ordering::SeqCst);
    CHORD_USED.store(false, Ordering::SeqCst);
    SWALLOWED_DOWN.store(false, Ordering::SeqCst);
    TOGGLE_LATCH.store(false, Ordering::SeqCst);
    *EXCLUSIVE_APP.lock().map_err(|e| e.to_string())? = None;
    if let Some(thread) = HOOK_THREAD.lock().map_err(|e| e.to_string())?.take() {
        unsafe {
            let _ = PostThreadMessageW(thread.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
    Ok(())
}

fn ensure_hook_thread() -> Result<(), String> {
    if HOOK_THREAD.lock().map_err(|e| e.to_string())?.is_some() {
        return Ok(());
    }

    std::thread::spawn(|| {
        unsafe {
            let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), None, 0)
            {
                Ok(h) => h,
                Err(e) => {
                    log::error!("SetWindowsHookExW 失败: {e}");
                    return;
                }
            };
            let tid = GetCurrentThreadId();
            *HOOK_THREAD.lock().expect("hook thread lock") = Some(HookThread { thread_id: tid });

            let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let _ = UnhookWindowsHookEx(hook);
            *HOOK_THREAD.lock().expect("hook thread lock") = None;
        }
    });

    for _ in 0..20 {
        if HOOK_THREAD.lock().map_err(|e| e.to_string())?.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err("键盘钩子线程启动超时".into())
}

fn is_key_down(msg: u32) -> bool {
    msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN
}

fn is_key_up(msg: u32) -> bool {
    msg == WM_KEYUP || msg == WM_SYSKEYUP
}

fn pass_next(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { CallNextHookEx(HHOOK::default(), code, wparam, lparam) }
}

fn send_vk(vk: u32, key_up: bool) {
    let flags = if key_up { KEYEVENTF_KEYUP } else { windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0) };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk as u16),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code != HC_ACTION as i32 {
        return pass_next(code, wparam, lparam);
    }

    let target = TARGET_VK.load(Ordering::SeqCst);
    if target == 0 {
        return pass_next(code, wparam, lparam);
    }

    let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    let vk = info.vkCode;
    let msg = wparam.0 as u32;

    if vk == target {
        if is_key_down(msg) {
            MOD_HELD.store(true, Ordering::SeqCst);
            CHORD_USED.store(false, Ordering::SeqCst);
            SWALLOWED_DOWN.store(true, Ordering::SeqCst);
            // 单按：先吞掉按下，避免触发系统「单键」行为（如 Alt 菜单）
            return LRESULT(1);
        }
        if is_key_up(msg) {
            MOD_HELD.store(false, Ordering::SeqCst);
            SWALLOWED_DOWN.store(false, Ordering::SeqCst);
            let chord = CHORD_USED.swap(false, Ordering::SeqCst);
            if chord {
                return pass_next(code, wparam, lparam);
            }
            if !TOGGLE_LATCH.swap(true, Ordering::SeqCst) {
                if let Ok(guard) = EXCLUSIVE_APP.lock() {
                    if let Some(app) = guard.clone() {
                        let app_for_main = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            toggle_dictation(app_for_main);
                        });
                    }
                }
            }
            TOGGLE_LATCH.store(false, Ordering::SeqCst);
            return LRESULT(1);
        }
        return pass_next(code, wparam, lparam);
    }

    if is_key_down(msg) && MOD_HELD.load(Ordering::SeqCst) {
        CHORD_USED.store(true, Ordering::SeqCst);
        if SWALLOWED_DOWN.swap(false, Ordering::SeqCst) {
            send_vk(target, false);
        }
    }

    pass_next(code, wparam, lparam)
}
