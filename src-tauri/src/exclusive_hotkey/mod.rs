//! 独占听写键：低层钩子拦截按键，不再传递给系统/其他应用（如右 Alt / AltGr）。

#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_tap;

#[cfg(not(any(windows, target_os = "macos")))]
mod unsupported;

#[cfg(windows)]
pub use windows::{apply_exclusive, clear_exclusive, is_exclusive_accel};

#[cfg(target_os = "macos")]
pub use macos::{apply_exclusive, clear_exclusive, is_exclusive_accel, macos_hotkey_diagnostics};

#[cfg(not(any(windows, target_os = "macos")))]
pub use unsupported::{apply_exclusive, clear_exclusive, is_exclusive_accel};

pub fn normalize_exclusive_label(accel: &str) -> Option<&'static str> {
    match accel.trim().to_ascii_lowercase().as_str() {
        "altright" | "rightalt" | "ralt" => Some("AltRight"),
        "altleft" | "leftalt" | "lalt" => Some("AltLeft"),
        "controlright" | "rightcontrol" | "rctrl" | "ctrlright" => Some("ControlRight"),
        "controlleft" | "leftcontrol" | "lctrl" | "ctrlleft" => Some("ControlLeft"),
        "shiftright" | "rightshift" | "rshift" => Some("ShiftRight"),
        "shiftleft" | "leftshift" | "lshift" => Some("ShiftLeft"),
        _ => None,
    }
}
