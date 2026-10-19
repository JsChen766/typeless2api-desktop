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
static TAP_STARTING: AtomicBool = AtomicBool::new(false);
static TAP_ERROR: Mutex<Option<String>> = Mutex::new(None);

const TAP_FAIL_HINT: &str = "Fn 快捷键需要系统允许监听键盘：请在 隐私与安全性 中开启「辅助功能」和「输入监控」里的 typeless-client，然后完全退出应用（托盘退出）再重新打开。仅勾选权限而不重启，监听常会注册失败。";

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

pub fn macos_hotkey_diagnostics() -> String {
    let trusted = macos_app::accessibility_trusted();
    let listen = unsafe { CGPreflightListenEventAccess() };
    let tap_probe = probe_event_tap();
    format!(
        "辅助功能(Accessibility)={}；输入监控(ListenEvent)={}；CGEventTap探测={}；Bundle={}；路径={}",
        if trusted { "已授权" } else { "未授权" },
        if listen { "已授权" } else { "未授权" },
        if tap_probe { "成功" } else { "失败" },
        macos_app::bundle_identifier(),
        macos_app::executable_display_path()
    )
}

fn probe_event_tap() -> bool {
    unsafe {
        let mask = event_mask_for(&[CGEventType::FlagsChanged]);
        let tap = CGEventTapCreate(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            mask,
            stub_callback,
            ptr::null(),
        );
        if tap.is_null() {
            return false;
        }
        CFMachPortInvalidate(tap);
        CFRelease(tap as *const c_void);
        true
    }
}

unsafe extern "C" fn stub_callback(
    _proxy: CGEventTapProxy,
    _etype: CGEventType,
    event: CGEventRef,
    _user_info: *const c_void,
) -> CGEventRef {
    event
}

fn request_listen_event_access_if_needed() {
    unsafe {
        if !CGPreflightListenEventAccess() {
            CGRequestListenEventAccess();
        }
    }
}

pub fn apply_exclusive(app: &AppHandle, accel: &str) -> Result<(), String> {
    // 勿在此处自动打开「辅助功能」系统设置：AXIsProcessTrusted 在 ad-hoc 重签后常为 false，
    // 即用户已勾选旧条目也会反复弹窗。以 EventTap 能否创建为准，失败时再给出路径说明。
    request_listen_event_access_if_needed();

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

unsafe fn create_event_tap() -> CFMachPortRef {
    let mask = event_mask_for(&[
        CGEventType::KeyDown,
        CGEventType::KeyUp,
        CGEventType::FlagsChanged,
    ]);
    for location in [CGEventTapLocation::Session, CGEventTapLocation::HID] {
        let tap = CGEventTapCreate(
            location,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            mask,
            event_callback,
            ptr::null(),
        );
        if !tap.is_null() {
            return tap;
        }
    }
    ptr::null_mut()
}

fn ensure_event_tap() -> Result<(), String> {
    if TAP_READY.load(Ordering::SeqCst) {
        return Ok(());
    }
    if TAP_STARTING.swap(true, Ordering::SeqCst) {
        for _ in 0..160 {
            if TAP_READY.load(Ordering::SeqCst) {
                return Ok(());
            }
            if let Some(msg) = TAP_ERROR.lock().ok().and_then(|g| g.clone()) {
                TAP_STARTING.store(false, Ordering::SeqCst);
                return Err(msg);
            }
            thread::sleep(Duration::from_millis(25));
        }
        TAP_STARTING.store(false, Ordering::SeqCst);
        return Err(TAP_FAIL_HINT.into());
    }

    *TAP_ERROR.lock().map_err(|e| e.to_string())? = None;

    thread::spawn(|| {
        unsafe {
            let tap = create_event_tap();
            if tap.is_null() {
                log::error!("CGEventTapCreate failed (Session+HID)");
                let diag = macos_hotkey_diagnostics();
                let mismatch = if !macos_app::accessibility_trusted() {
                    format!("\n\n{}", macos_app::accessibility_mismatch_hint())
                } else {
                    String::new()
                };
                *TAP_ERROR.lock().expect("tap error") = Some(format!(
                    "{TAP_FAIL_HINT}\n（诊断：{diag}）{mismatch}"
                ));
                TAP_STARTING.store(false, Ordering::SeqCst);
                return;
            }

            let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0);
            if source.is_null() {
                CFMachPortInvalidate(tap);
                CFRelease(tap as *const c_void);
                *TAP_ERROR.lock().expect("tap error") =
                    Some("键盘监听源创建失败，请重启应用后重试。".into());
                TAP_STARTING.store(false, Ordering::SeqCst);
                return;
            }

            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
            TAP_PORT.store(tap as usize, Ordering::SeqCst);
            TAP_READY.store(true, Ordering::SeqCst);
            TAP_STARTING.store(false, Ordering::SeqCst);

            CFRunLoopRun();

            TAP_READY.store(false, Ordering::SeqCst);
            TAP_PORT.store(0, Ordering::SeqCst);
            CFRunLoopRemoveSource(run_loop, source, kCFRunLoopCommonModes);
            CFMachPortInvalidate(tap);
            CFRelease(source as *const c_void);
            CFRelease(tap as *const c_void);
        }
    });

    for _ in 0..160 {
        if TAP_READY.load(Ordering::SeqCst) {
            return Ok(());
        }
        if let Some(msg) = TAP_ERROR.lock().ok().and_then(|g| g.clone()) {
            TAP_STARTING.store(false, Ordering::SeqCst);
            return Err(msg);
        }
        thread::sleep(Duration::from_millis(25));
    }
    TAP_STARTING.store(false, Ordering::SeqCst);
    let mismatch = if !macos_app::accessibility_trusted() {
        format!("\n\n{}", macos_app::accessibility_mismatch_hint())
    } else {
        String::new()
    };
    Err(format!(
        "{}\n（诊断：{}）{}",
        TAP_FAIL_HINT,
        macos_hotkey_diagnostics(),
        mismatch
    ))
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
        // Fn/Globe 松开时常会连续收到 FlagsChanged + KeyUp，各触发一次「松开」分支；
        // 若不去重会 toggle 两次（开始立刻结束），体感像「按住才录」。
        let had_solo_press = SWALLOWED_DOWN.swap(false, Ordering::SeqCst);
        MOD_HELD.store(false, Ordering::SeqCst);
        if !had_solo_press {
            return ptr::null();
        }
        let chord = CHORD_USED.swap(false, Ordering::SeqCst);
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
