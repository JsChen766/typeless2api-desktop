use tauri::AppHandle;

pub fn is_exclusive_accel(_accel: &str) -> bool {
    false
}

pub fn apply_exclusive(_app: &AppHandle, accel: &str) -> Result<(), String> {
    Err(format!("独占快捷键「{accel}」仅支持 Windows / macOS"))
}

pub fn clear_exclusive() -> Result<(), String> {
    Ok(())
}
