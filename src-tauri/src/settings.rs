use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize, Deserialize)]
pub struct RuntimeSettings {
    pub base_url: String,
    pub api_key: String,
    pub locale: String,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
}

fn default_hotkey() -> String {
    "F9".into()
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:8787".into(),
            api_key: String::new(),
            locale: "zh-CN".into(),
            hotkey: default_hotkey(),
        }
    }
}

static CACHE: Mutex<Option<RuntimeSettings>> = Mutex::new(None);

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("runtime-settings.json"))
}

fn persist(app: &AppHandle, settings: &RuntimeSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| format!("保存设置失败：{e}"))
}

pub fn load_runtime_settings(app: &AppHandle) -> RuntimeSettings {
    if let Ok(mut guard) = CACHE.lock() {
        let settings = guard.get_or_insert_with(|| load_settings_from_disk(app));
        return settings.clone();
    }
    load_settings_from_disk(app)
}

fn update_settings(app: &AppHandle, change: impl FnOnce(&mut RuntimeSettings)) -> Result<(), String> {
    let mut guard = CACHE.lock().map_err(|e| e.to_string())?;
    let mut settings = guard.clone().unwrap_or_else(|| load_settings_from_disk(app));
    change(&mut settings);
    persist(app, &settings)?;
    *guard = Some(settings);
    Ok(())
}

fn load_settings_from_disk(app: &AppHandle) -> RuntimeSettings {
    settings_path(app)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn update_hotkey(app: &AppHandle, hotkey: String) -> Result<(), String> {
    update_settings(app, |settings| {
        settings.hotkey = hotkey.trim().to_string();
        if settings.hotkey.is_empty() {
            settings.hotkey = default_hotkey();
        }
    })
}

#[tauri::command]
pub fn save_runtime_settings(
    app: AppHandle,
    base_url: String,
    api_key: String,
    locale: Option<String>,
    hotkey: Option<String>,
) -> Result<(), String> {
    update_settings(&app, |settings| {
        settings.base_url = base_url;
        settings.api_key = api_key;
        settings.locale = locale.unwrap_or_else(|| "zh-CN".into());
        if let Some(hotkey) = hotkey.filter(|h| !h.trim().is_empty()) {
            settings.hotkey = hotkey.trim().to_string();
        }
    })
}

#[tauri::command]
pub fn get_runtime_settings(app: AppHandle) -> RuntimeSettings {
    load_runtime_settings(&app)
}
