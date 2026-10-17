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

fn seed_api_key_from_workspace() -> String {
    let candidates = [
        PathBuf::from("gateway/data/client-api-key.txt"),
        PathBuf::from("../gateway/data/client-api-key.txt"),
        PathBuf::from("../../gateway/data/client-api-key.txt"),
        PathBuf::from("E:/vsProjects/reverse-typeless/gateway/data/client-api-key.txt"),
    ];
    for path in candidates {
        if let Ok(raw) = fs::read_to_string(&path) {
            let key = raw.trim();
            if key.starts_with("sk_") {
                return key.to_string();
            }
        }
    }
    String::new()
}

fn persist(app: &AppHandle, settings: &RuntimeSettings) {
    if let Ok(path) = settings_path(app) {
        if let Ok(raw) = serde_json::to_string_pretty(settings) {
            let _ = fs::write(path, raw);
        }
    }
}

pub fn load_runtime_settings(app: &AppHandle) -> RuntimeSettings {
    if let Ok(guard) = CACHE.lock() {
        if let Some(cached) = guard.as_ref() {
            if !cached.api_key.trim().is_empty() {
                return cached.clone();
            }
        }
    }

    let mut settings = RuntimeSettings::default();
    if let Ok(path) = settings_path(app) {
        if path.exists() {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(parsed) = serde_json::from_str::<RuntimeSettings>(&raw) {
                    settings = parsed;
                }
            }
        }
    }
    if settings.api_key.trim().is_empty() {
        settings.api_key = seed_api_key_from_workspace();
        if !settings.api_key.is_empty() {
            persist(app, &settings);
        }
    }
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(settings.clone());
    }
    settings
}

#[tauri::command]
pub fn update_hotkey(app: &AppHandle, hotkey: String) -> Result<(), String> {
    let mut settings = load_runtime_settings(app);
    settings.hotkey = hotkey.trim().to_string();
    if settings.hotkey.is_empty() {
        settings.hotkey = default_hotkey();
    }
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(settings.clone());
    }
    persist(app, &settings);
    Ok(())
}

#[tauri::command]
pub fn save_runtime_settings(
    app: AppHandle,
    base_url: String,
    api_key: String,
    locale: Option<String>,
    hotkey: Option<String>,
) -> Result<(), String> {
    let prev = load_runtime_settings(&app);
    let settings = RuntimeSettings {
        base_url,
        api_key,
        locale: locale.unwrap_or_else(|| "zh-CN".into()),
        hotkey: hotkey
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .unwrap_or(prev.hotkey),
    };
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(settings.clone());
    }
    persist(&app, &settings);
    Ok(())
}

#[tauri::command]
pub fn get_runtime_settings(app: AppHandle) -> RuntimeSettings {
    load_runtime_settings(&app)
}
