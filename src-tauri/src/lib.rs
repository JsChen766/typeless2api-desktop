mod dictation;
mod exclusive_hotkey;
mod hotkey;
mod inject;
#[cfg(target_os = "macos")]
mod macos_app;
#[cfg(any(windows, target_os = "macos"))]
mod opus;
mod settings;
mod text_context;
mod window_position;

use dictation::{
    dictation_cancel, dictation_start, dictation_stop, dictation_toggle,
};
use settings::{get_runtime_settings, save_runtime_settings};
use inject::{
    capture_input_target, dictation_context_hint, inject_text, position_pill_window,
    set_pill_visible,
};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{
    Listener, Manager, WindowEvent,
    menu::{Menu, MenuItem, MenuItemKind},
    tray::{TrayIconBuilder, TrayIconEvent},
};

static PANEL_SHOWN_AT: Mutex<Option<Instant>> = Mutex::new(None);
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            capture_input_target,
            dictation_context_hint,
            dictation_start,
            dictation_stop,
            dictation_cancel,
            dictation_toggle,
            quit_app,
            inject_text,
            position_pill_window,
            set_pill_visible,
            get_runtime_settings,
            save_runtime_settings,
            set_dictation_hotkey,
            clear_dictation_hotkey_cmd,
            macos_accessibility_trusted,
            macos_open_accessibility_settings
        ])
        .setup(|app| {
            inject::mark_main_thread();
            let handle = app.handle().clone();
            app.listen("pill-action", move |event| {
                let payload = event.payload();
                let action: String = serde_json::from_str::<serde_json::Value>(payload)
                    .ok()
                    .and_then(|v| {
                        v.get("action")
                            .and_then(|a| a.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();
                let app = handle.clone();
                tauri::async_runtime::spawn(async move {
                    if action == "cancel" {
                        let _ = dictation_cancel(app).await;
                    } else if action == "complete" {
                        let _ = dictation_stop(app).await;
                    }
                });
            });
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let open_settings = MenuItem::with_id(app, "open-settings", "打开设置…", true, None::<&str>)?;
            if let Some(app_menu) = app.menu() {
                if let Some(MenuItemKind::Submenu(submenu)) = app_menu.items()?.into_iter().next() {
                    submenu.insert(&open_settings, 1)?;
                }
            }
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_settings, &quit])?;

            let icon = app.default_window_icon().expect("missing application icon");

            TrayIconBuilder::with_id("tray")
                .icon(icon.clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "open-settings" {
                        toggle_panel(app);
                    } else if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, button_state: tauri::tray::MouseButtonState::Up, .. }
                    ) {
                        toggle_panel(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Some(main) = app.get_webview_window("main") {
                let w = main.clone();
                main.on_window_event(move |event| match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        let _ = w.hide();
                        api.prevent_close();
                    }
                    WindowEvent::Focused(false) => {
                        #[cfg(target_os = "macos")]
                        if cursor_over_dock(&w) {
                            return;
                        }
                        let just_opened = PANEL_SHOWN_AT
                            .lock()
                            .ok()
                            .and_then(|guard| *guard)
                            .is_some_and(|at| at.elapsed() < Duration::from_millis(300));
                        if !just_opened {
                            let _ = w.hide();
                        }
                    }
                    _ => {}
                });
                let _ = main.hide();
            }

            let settings = settings::load_runtime_settings(app.handle());
            if let Err(err) = hotkey::apply_dictation_hotkey(app.handle(), &settings.hotkey) {
                log::warn!("初始快捷键未注册: {err}");
            }

            if let Some(pill) = app.get_webview_window("pill") {
                let app_handle = app.handle().clone();
                pill.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = set_pill_visible(app_handle.clone(), false, "idle".into());
                        let app2 = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = dictation_cancel(app2).await;
                        });
                    }
                });
                let _ = set_pill_visible(app.handle().clone(), false, "idle".into());
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                remember_dock_anchor(app);
                toggle_panel(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn set_dictation_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
    let previous = settings::load_runtime_settings(&app).hotkey;
    if let Err(err) = hotkey::apply_dictation_hotkey(&app, &hotkey) {
        let _ = hotkey::apply_dictation_hotkey(&app, &previous);
        return Err(err);
    }
    if let Err(err) = settings::update_hotkey(&app, hotkey) {
        let _ = hotkey::apply_dictation_hotkey(&app, &previous);
        return Err(err);
    }
    Ok(())
}

#[tauri::command]
fn clear_dictation_hotkey_cmd(app: tauri::AppHandle) -> Result<(), String> {
    hotkey::clear_dictation_hotkey(&app)
}

#[tauri::command]
fn macos_accessibility_trusted() -> bool {
    #[cfg(target_os = "macos")]
    {
        return macos_app::accessibility_trusted();
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[tauri::command]
fn macos_open_accessibility_settings() -> bool {
    #[cfg(target_os = "macos")]
    {
        return macos_app::request_accessibility_trust(true);
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

fn toggle_panel(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    show_panel(app);
}

fn show_panel(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        place_panel(app, &window);
        let _ = window.set_focus();
        return;
    }
    // 弹窗还没出现时，前台仍是用户正在输入的程序
    inject::capture_foreground_target();
    place_panel(app, &window);
    if let Ok(mut shown) = PANEL_SHOWN_AT.lock() {
        *shown = Some(Instant::now());
    }
    let _ = window.show();
    let _ = window.set_focus();
}

pub(crate) fn show_settings_panel(app: &tauri::AppHandle) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || show_panel(&app));
}

fn place_panel(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    let (width, height) = (340.0, 580.0);
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let _ = window.set_focusable(true);
        if let Ok(raw) = window.hwnd() {
            let hwnd = windows::Win32::Foundation::HWND(raw.0 as *mut _);
            strip_panel_caption(hwnd);
        }
        let mut cursor = POINT::default();
        if unsafe { GetCursorPos(&mut cursor) }.is_err() {
            return;
        }
        let scale = window.scale_factor().unwrap_or(1.0);
        let (work_left, work_top, work_right, work_bottom) = window
            .primary_monitor()
            .ok()
            .flatten()
            .map(|monitor| {
                let origin = monitor.position();
                let size = monitor.size();
                let factor = monitor.scale_factor();
                (
                    origin.x as f64 / factor,
                    origin.y as f64 / factor,
                    (origin.x as f64 + size.width as f64) / factor,
                    (origin.y as f64 + size.height as f64) / factor,
                )
            })
            .unwrap_or((0.0, 0.0, 1920.0, 1080.0));
        let cursor_x = cursor.x as f64 / scale;
        let cursor_y = cursor.y as f64 / scale;
        let mut x = cursor_x - width + 28.0;
        let mut y = cursor_y - height - 8.0;
        if y < work_top {
            y = cursor_y + 12.0;
        }
        if y + height > work_bottom {
            y = (work_bottom - height - 8.0).max(work_top);
        }
        if x < work_left {
            x = work_left + 8.0;
        }
        if x + width > work_right {
            x = (work_right - width - 8.0).max(work_left);
        }
        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = window.set_focusable(true);
        let area = window
            .primary_monitor()
            .ok()
            .flatten()
            .map(|monitor| {
                let visible = monitor.work_area();
                let factor = monitor.scale_factor();
                window_position::Area {
                    left: visible.position.x as f64 / factor,
                    top: visible.position.y as f64 / factor,
                    width: visible.size.width as f64 / factor,
                    height: visible.size.height as f64 / factor,
                }
            })
            .unwrap_or(window_position::Area {
                left: 0.0,
                top: 24.0,
                width: 1512.0,
                height: 890.0,
            });
        let anchor_x = settings::load_runtime_settings(app).dock_anchor_x;
        let position = window_position::settings_position(area, anchor_x, width, height);
        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition {
            x: position.x,
            y: position.y,
        }));
    }
}

#[cfg(target_os = "macos")]
fn remember_dock_anchor(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Some(monitor) = window.primary_monitor().ok().flatten() else {
        return;
    };
    let Ok(cursor) = window.cursor_position() else {
        return;
    };
    let x = cursor.x / monitor.scale_factor();
    if let Err(err) = settings::update_dock_anchor(app, x) {
        log::warn!("保存程序坞位置失败: {err}");
    }
}

#[cfg(target_os = "macos")]
fn cursor_over_dock(window: &tauri::WebviewWindow) -> bool {
    let Some(monitor) = window.primary_monitor().ok().flatten() else {
        return false;
    };
    let Ok(cursor) = window.cursor_position() else {
        return false;
    };
    let screen_bottom = monitor.position().y as f64 + monitor.size().height as f64;
    let visible = monitor.work_area();
    let visible_bottom = visible.position.y as f64 + visible.size.height as f64;
    let dock_top = if screen_bottom - visible_bottom > 24.0 {
        visible_bottom
    } else {
        screen_bottom - 96.0
    };
    cursor.y >= dock_top - 8.0 && cursor.y <= screen_bottom + 8.0
}

#[cfg(target_os = "windows")]
fn strip_panel_caption(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
        WS_SYSMENU, WS_THICKFRAME,
    };
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let chrome = (WS_CAPTION.0
            | WS_THICKFRAME.0
            | WS_SYSMENU.0
            | WS_MINIMIZEBOX.0
            | WS_MAXIMIZEBOX.0) as isize;
        let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, style & !chrome);
        let _ = SetWindowPos(
            hwnd,
            windows::Win32::Foundation::HWND(std::ptr::null_mut()),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
    }
}
