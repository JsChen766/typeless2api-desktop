use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use tauri::AppHandle;

use super::macos_tap::*;
use super::normalize_exclusive_label;
use crate::dictation::toggle_dictation;
use crate::macos_app;

static TARGET_KEY: AtomicU16 = AtomicU16::new(0);
static MOD_HELD: AtomicBool = AtomicBool::new(false);
static CHORD_USED: AtomicBool = AtomicBool::new(false);
static SWALLOWED_DOWN: AtomicBool = AtomicBool::new(false);
static TOGGLE_LATCH: AtomicBool = AtomicBool::new(false);
static EXCLUSIVE_APP: Mutex<Option<AppHandle>> = Mutex::new(None);
static TAP_PORT: AtomicUsize = AtomicUsize::new(0);
static TAP_READY: AtomicBool = AtomicBool::new(false);

pub fn is_fn_accel(accel: &str) -> bool {
    matches!(
        accel.trim().to_ascii_lowercase().as_str(),
        "fn" | "globe" | "function"
    )
}

pub fn is_exclusive_accel(accel: &str) -> bool {
    is_fn_accel(accel) || normalize_exclusive_label(accel).is_some()
}

fn keycode_for_accel(accel: &str) -> Option<u16> {
    if is_fn_accel(accel) {
        return Some(KEY_FUNCTION);
    }
    match normalize_exclusive_label(accel)? {
        "AltRight" => Some(0x3D),
        "AltLeft" => Some(0x3A),
        "ControlRight" => Some(0x3E),
        "ControlLeft" => Some(0x3B),
        "ShiftRight" => Some(0x3C),
        "ShiftLeft" => Some(0x38),
        _ => None,
    }
}

pub fn apply_exclusive(app: &AppHandle, accel: &str) -> Result<(), String> {
    if !macos_app::accessibility_trusted() {
        macos_app::request_accessibility_trust(true);
        if !macos_app::accessibility_trusted() {
            return Err(
                "需要「辅助功能」权限才能使用 Fn/修饰键听写：请在 系统设置 → 隐私与安全性 → 辅助功能 中启用本应用，并将键盘「按 🌐/Fn 键以」设为「无操作」后重试。"
                    .into(),
            );
        }
    }

    let key = keycode_for_accel(accel).ok_or_else(|| format!("不支持的独占键：{accel}"))?;
    *EXCLUSIVE_APP.lock().map_err(|e| e.to_string())? = Some(app.clone());
    TARGET_KEY.store(key, Ordering::SeqCst);
    MOD_HELD.store(false, Ordering::SeqCst);
    CHORD_USED.store(false, Ordering::SeqCst);
    SWALLOWED_DOWN.store(false, Ordering::SeqCst);
    TOGGLE_LATCH.store(false, Ordering::SeqCst);
    ensure_event_tap()?;
    Ok(())
}

pub fn clear_exclusive() -> Result<(), String> {
    TARGET_KEY.store(0, Ordering::SeqCst);
    MOD_HELD.store(false, Ordering::SeqCst);
    CHORD_USED.store(false, Ordering::SeqCst);
    SWALLOWED_DOWN.store(false, Ordering::SeqCst);
    *EXCLUSIVE_APP.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

fn ensure_event_tap() -> Result<(), String> {
    if TAP_READY.load(Ordering::SeqCst) {
        return Ok(());
    }

    thread::spawn(|| {
        unsafe {
            let mask = event_mask_for(&[
                CGEventType::KeyDown,
                CGEventType::KeyUp,
                CGEventType::FlagsChanged,
            ]);
            let tap = CGEventTapCreate(
                CGEventTapLocation::HID,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                mask,
                event_callback,
                ptr::null(),
            );
            if tap.is_null() {
                log::error!("CGEventTapCreate 返回 null");
                return;
            }

            let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0);
            if source.is_null() {
                CFMachPortInvalidate(tap);
                CFRelease(tap as *const c_void);
                return;
            }

            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
            TAP_PORT.store(tap as usize, Ordering::SeqCst);
            TAP_READY.store(true, Ordering::SeqCst);

            CFRunLoopRun();

            TAP_READY.store(false, Ordering::SeqCst);
            TAP_PORT.store(0, Ordering::SeqCst);
            CFRunLoopRemoveSource(run_loop, source, kCFRunLoopCommonModes);
            CFMachPortInvalidate(tap);
            CFRelease(source as *const c_void);
            CFRelease(tap as *const c_void);
        }
    });

    for _ in 0..40 {
        if TAP_READY.load(Ordering::SeqCst) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err("macOS 键盘监听线程未就绪（请确认已授予辅助功能权限）".into())
}

fn post_key(keycode: u16, keydown: bool) {
    unsafe {
        let source = CGEventSourceCreate(CGEventSourceStateID::CombinedSessionState);
        if source.is_null() {
            return;
        }
        let event = CGEventCreateKeyboardEvent(source, keycode, keydown);
        CFRelease(source as *const c_void);
        if event.is_null() {
            return;
        }
        CGEventPost(CGEventTapLocation::HID, event);
        CFRelease(event as *const c_void);
    }
}

fn is_target_event(event: CGEventRef, target: u16) -> bool {
    if event.is_null() {
        return false;
    }
    let keycode =
        unsafe { CGEventGetIntegerValueField(event, KEYBOARD_EVENT_KEYCODE) as u16 };
    keycode == target
}

fn key_is_down(keycode: u16) -> bool {
    unsafe { CGEventSourceKeyState(CGEventSourceStateID::HIDSystemState, keycode) }
}

unsafe extern "C" fn event_callback(
    _proxy: CGEventTapProxy,
    etype: CGEventType,
    event: CGEventRef,
    _user_info: *const c_void,
) -> CGEventRef {
    if matches!(
        etype,
        CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput
    ) {
        let port = TAP_PORT.load(Ordering::SeqCst) as CFMachPortRef;
        if !port.is_null() {
            CGEventTapEnable(port, true);
        }
        return event;
    }

    let target = TARGET_KEY.load(Ordering::SeqCst);
    if target == 0 {
        return event;
    }

    let target_event = is_target_event(event, target);

    if target_event
        && (etype == CGEventType::FlagsChanged
            || etype == CGEventType::KeyDown
            || etype == CGEventType::KeyUp)
    {
        let down = key_is_down(target);
        if down {
            MOD_HELD.store(true, Ordering::SeqCst);
            CHORD_USED.store(false, Ordering::SeqCst);
            SWALLOWED_DOWN.store(true, Ordering::SeqCst);
            return ptr::null();
        }
        MOD_HELD.store(false, Ordering::SeqCst);
        let chord = CHORD_USED.swap(false, Ordering::SeqCst);
        SWALLOWED_DOWN.store(false, Ordering::SeqCst);
        if chord {
            return event;
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
        return ptr::null();
    }

    if etype == CGEventType::KeyDown && MOD_HELD.load(Ordering::SeqCst) {
        CHORD_USED.store(true, Ordering::SeqCst);
        if SWALLOWED_DOWN.swap(false, Ordering::SeqCst) {
            post_key(target, true);
        }
    }

    event
}
