use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::dictation::toggle_dictation;

static PRESS_LATCH: AtomicBool = AtomicBool::new(false);

/// 与前端 `toTauriShortcut` 对齐：Ctrl/Meta → CommandOrControl
pub fn to_plugin_accel(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "F9".into();
    }
    trimmed
        .split('+')
        .map(|part| {
            let p = part.trim();
            if p.eq_ignore_ascii_case("ctrl")
                || p.eq_ignore_ascii_case("control")
                || p.eq_ignore_ascii_case("meta")
                || p.eq_ignore_ascii_case("commandorcontrol")
            {
                "CommandOrControl"
            } else if p.eq_ignore_ascii_case("alt") {
                "Alt"
            } else if p.eq_ignore_ascii_case("shift") {
                "Shift"
            } else {
                p
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

pub fn clear_dictation_hotkey(app: &AppHandle) -> Result<(), String> {
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    PRESS_LATCH.store(false, Ordering::SeqCst);
    Ok(())
}

pub fn apply_dictation_hotkey(app: &AppHandle, accel: &str) -> Result<(), String> {
    let accel = to_plugin_accel(accel);
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    PRESS_LATCH.store(false, Ordering::SeqCst);
    gs.on_shortcut(accel.as_str(), move |app, _shortcut, event| {
        match event.state {
            ShortcutState::Pressed => {
                if PRESS_LATCH.swap(true, Ordering::SeqCst) {
                    return;
                }
                toggle_dictation(app.clone());
            }
            ShortcutState::Released => {
                PRESS_LATCH.store(false, Ordering::SeqCst);
            }
        }
    })
    .map_err(|e| format!("快捷键注册失败（{accel}）：{e}"))?;
    Ok(())
}
