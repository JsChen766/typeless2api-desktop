use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager};

use crate::text_context;

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct InjectTarget {
    top: isize,
    edit: Option<isize>,
}

#[cfg(target_os = "windows")]
static TARGET: Mutex<Option<InjectTarget>> = Mutex::new(None);

#[cfg(target_os = "macos")]
static MAC_TARGET_PID: Mutex<Option<i32>> = Mutex::new(None);

pub fn paste_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd+V"
    } else {
        "Ctrl+V"
    }
}

#[cfg(target_os = "windows")]
fn hwnd_from_raw(raw: isize) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(raw as *mut _)
}

#[cfg(target_os = "windows")]
fn is_our_window(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    if hwnd.0 == std::ptr::null_mut() {
        return true;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid == 0 || pid == unsafe { GetCurrentProcessId() }
}

#[cfg(target_os = "windows")]
fn focused_edit_hwnd(
    top: windows::Win32::Foundation::HWND,
) -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
    };
    let mut pid = 0u32;
    let tid = unsafe { GetWindowThreadProcessId(top, Some(&mut pid)) };
    if tid == 0 {
        return None;
    }
    let mut info = unsafe { std::mem::zeroed::<GUITHREADINFO>() };
    info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if unsafe { GetGUIThreadInfo(tid, &mut info) }.is_err() {
        return None;
    }
    if info.hwndCaret.0 != std::ptr::null_mut() {
        return Some(info.hwndCaret);
    }
    if info.hwndFocus.0 != std::ptr::null_mut() && !is_our_window(info.hwndFocus) {
        return Some(info.hwndFocus);
    }
    None
}

pub fn capture_foreground_target() {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let top = unsafe { GetForegroundWindow() };
        if is_our_window(top) {
            return;
        }
        *TARGET.lock().expect("lock hwnd") = Some(InjectTarget {
            top: top.0 as isize,
            edit: focused_edit_hwnd(top).map(|h| h.0 as isize),
        });
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(app) = crate::macos_app::frontmost_app() {
            if crate::macos_app::is_our_bundle(&app.bundle_id) {
                return;
            }
            *MAC_TARGET_PID.lock().expect("lock pid") = Some(app.pid);
        }
    }
}

#[tauri::command]
pub fn capture_input_target() {
    capture_foreground_target();
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DictationContextHint {
    pub app_name: String,
    pub window_title: String,
    pub app_identifier: String,
    pub app_type: String,
    pub process_id: u32,
    pub window_id: isize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_state: Option<text_context::DictationCursorState>,
}

#[tauri::command]
pub fn dictation_context_hint() -> DictationContextHint {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow};
        let saved = TARGET
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|target| hwnd_from_raw(target.top)))
            .filter(|hwnd| unsafe { IsWindow(*hwnd).as_bool() } && !is_our_window(*hwnd));
        let hwnd = saved.unwrap_or_else(|| unsafe { GetForegroundWindow() });
        if hwnd.0 == std::ptr::null_mut() || is_our_window(hwnd) {
            return DictationContextHint {
                app_name: "Desktop App".into(),
                window_title: String::new(),
                app_identifier: "-1".into(),
                app_type: "native_app".into(),
                process_id: 0,
                window_id: 0,
                cursor_state: None,
            };
        }
        return windows_dictation_context_hint(hwnd);
    }
    #[cfg(target_os = "macos")]
    {
        let pid = MAC_TARGET_PID.lock().ok().and_then(|guard| *guard);
        let app = pid
            .and_then(crate::macos_app::app_for_pid)
            .or_else(crate::macos_app::frontmost_app)
            .filter(|app| !crate::macos_app::is_our_bundle(&app.bundle_id));
        if let Some(app) = app {
            return DictationContextHint {
                app_name: app.name,
                window_title: String::new(),
                app_identifier: app.bundle_id,
                app_type: "native_app".into(),
                process_id: app.pid as u32,
                window_id: 0,
                cursor_state: None,
            };
        }
        return DictationContextHint {
            app_name: "Desktop App".into(),
            window_title: String::new(),
            app_identifier: "-1".into(),
            app_type: "native_app".into(),
            process_id: 0,
            window_id: 0,
            cursor_state: None,
        };
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        DictationContextHint {
            app_name: "Desktop App".into(),
            window_title: String::new(),
            app_identifier: "-1".into(),
            app_type: "native_app".into(),
            process_id: 0,
            window_id: 0,
            cursor_state: None,
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_dictation_context_hint(
    hwnd: windows::Win32::Foundation::HWND,
) -> DictationContextHint {
    use std::path::Path;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextW, GetWindowThreadProcessId};

    if hwnd.0 == std::ptr::null_mut() {
        return DictationContextHint {
            app_name: "Desktop App".into(),
            window_title: String::new(),
            app_identifier: "-1".into(),
            app_type: "native_app".into(),
            process_id: 0,
            window_id: 0,
            cursor_state: None,
        };
    }

    let mut title_buf = [0u16; 1024];
    let len = unsafe { GetWindowTextW(hwnd, &mut title_buf) };
    let window_title = String::from_utf16_lossy(&title_buf[..len as usize]);

    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };

    let mut path = String::new();
    if pid != 0 {
        let handle = unsafe {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
        };
        if let Ok(handle) = handle {
            if !handle.is_invalid() {
                let mut buf = [0u16; 1024];
                let mut size = buf.len() as u32;
                let ok = unsafe {
                    QueryFullProcessImageNameW(
                        HANDLE(handle.0),
                        PROCESS_NAME_WIN32,
                        windows::core::PWSTR(buf.as_mut_ptr()),
                        &mut size,
                    )
                };
                if ok.is_ok() && size > 0 {
                    path = String::from_utf16_lossy(&buf[..size as usize]);
                }
            }
            let _ = unsafe { CloseHandle(handle) };
        }
    }

    let app_name = if path.is_empty() {
        "Desktop App".to_string()
    } else {
        Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("Desktop App")
            .to_string()
    };

    let edit_hwnd = TARGET
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|t| t.edit))
        .filter(|h| *h != 0)
        .or_else(|| focused_edit_hwnd(hwnd).map(|h| h.0 as isize));
    let cursor_state = edit_hwnd.and_then(text_context::capture_cursor_state);

    DictationContextHint {
        app_name,
        window_title,
        app_identifier: if path.is_empty() { "-1".into() } else { path },
        app_type: "native_app".into(),
        process_id: pid,
        window_id: hwnd.0 as isize,
        cursor_state,
    }
}

#[tauri::command]
pub fn inject_text(text: String) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        return inject_text_windows(&text);
    }

    #[cfg(target_os = "macos")]
    {
        return inject_text_macos(&text);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = text;
        Err("当前平台暂不支持直接输入，请使用剪贴板".into())
    }
}

#[cfg(target_os = "macos")]
fn send_cmd_v() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo
        .key(Key::Meta, Direction::Press)
        .map_err(|e| e.to_string())?;
    let paste = enigo.key(Key::Unicode('v'), Direction::Click);
    let release = enigo.key(Key::Meta, Direction::Release);
    paste.map_err(|e| e.to_string())?;
    release.map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn inject_text_macos(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| e.to_string())?;
    if !crate::macos_app::accessibility_trusted() {
        return Err(
            "辅助功能权限未生效：请在系统设置→隐私与安全→辅助功能中重新开启 typeless-client，然后重启客户端"
                .into(),
        );
    }
    let pid = MAC_TARGET_PID.lock().ok().and_then(|guard| *guard);
    let Some(pid) = pid else {
        return Err("没有记录到听写前的输入窗口".into());
    };
    for _ in 0..3 {
        let _ = crate::macos_app::activate_pid(pid);
        std::thread::sleep(Duration::from_millis(120));
        if crate::macos_app::frontmost_app().is_some_and(|app| app.pid == pid) {
            return send_cmd_v();
        }
    }
    Err("无法切回听写前的输入窗口".into())
}

#[cfg(target_os = "windows")]
fn window_class(hwnd: windows::Win32::Foundation::HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;
    let mut buf = [0u16; 256];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    if n <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

#[cfg(target_os = "windows")]
fn paste_to_hwnd(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, SMTO_ABORTIFHUNG, WM_PASTE,
    };
    let mut result = usize::default();
    unsafe {
        let _ = SendMessageTimeoutW(
            hwnd,
            WM_PASTE,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            400,
            Some(&mut result),
        );
    }
}

#[cfg(target_os = "windows")]
fn force_foreground(
    top: windows::Win32::Foundation::HWND,
    focus: Option<windows::Win32::Foundation::HWND>,
) {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
        ShowWindow, SW_SHOW,
    };
    unsafe {
        let foreground = GetForegroundWindow();
        let current = GetCurrentThreadId();
        let foreground_tid = GetWindowThreadProcessId(foreground, None);
        let target_tid = GetWindowThreadProcessId(top, None);
        let attach_foreground = foreground_tid != 0 && foreground_tid != current;
        let attach_target = target_tid != 0 && target_tid != current && target_tid != foreground_tid;
        if attach_foreground {
            let _ = AttachThreadInput(current, foreground_tid, true);
        }
        if attach_target {
            let _ = AttachThreadInput(current, target_tid, true);
        }
        let _ = ShowWindow(top, SW_SHOW);
        let _ = SetForegroundWindow(top);
        let _ = BringWindowToTop(top);
        if let Some(edit) = focus {
            let _ = SetFocus(edit);
        }
        if attach_target {
            let _ = AttachThreadInput(current, target_tid, false);
        }
        if attach_foreground {
            let _ = AttachThreadInput(current, foreground_tid, false);
        }
    }
}

#[cfg(target_os = "windows")]
fn strip_caption(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, HWND_TOPMOST,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
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
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_NOZORDER,
        );
    }
}

#[cfg(target_os = "windows")]
fn show_pill_without_focus(pill: &tauri::WebviewWindow) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE,
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
        WS_EX_NOACTIVATE,
    };
    let _ = pill.set_focusable(false);
    let raw = pill.hwnd().map_err(|e| e.to_string())?;
    let hwnd = hwnd_from_raw(raw.0 as isize);
    strip_caption(hwnd);
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE.0 as isize);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn send_ctrl_v() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_CONTROL,
    };
    let vk_v = VIRTUAL_KEY(0x56);
    let key = |vk: VIRTUAL_KEY, up: bool| {
        let scan = unsafe { MapVirtualKeyW(u32::from(vk.0), MAPVK_VK_TO_VSC) } as u16;
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    };
    let inputs = [key(VK_CONTROL, false), key(vk_v, false), key(vk_v, true), key(VK_CONTROL, true)];
    unsafe {
        let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(target_os = "windows")]
fn inject_text_windows(text: &str) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, GetForegroundWindow, IsWindow, ASFW_ANY,
    };

    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| e.to_string())?;

    let stored = *TARGET.lock().map_err(|_| "lock hwnd")?;
    let foreground = unsafe { GetForegroundWindow() };
    let top = stored
        .map(|t| hwnd_from_raw(t.top))
        .filter(|h| unsafe { IsWindow(*h).as_bool() } && !is_our_window(*h));
    let edit = stored
        .and_then(|t| t.edit)
        .map(hwnd_from_raw)
        .filter(|h| unsafe { IsWindow(*h).as_bool() } && !is_our_window(*h))
        .or_else(|| top.and_then(focused_edit_hwnd))
        .or_else(|| focused_edit_hwnd(foreground));

    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }
    if let Some(hwnd) = top {
        force_foreground(hwnd, edit);
        std::thread::sleep(Duration::from_millis(90));
    }

    if let Some(hwnd) = edit {
        let class = window_class(hwnd);
        let native_edit = class.to_ascii_lowercase().contains("edit");
        if native_edit {
            paste_to_hwnd(hwnd);
            return Ok(());
        }
        send_ctrl_v();
        return Ok(());
    }

    if !is_our_window(foreground) || top.is_some() {
        send_ctrl_v();
        return Ok(());
    }

    Err(format!(
        "未能切回输入窗口，结果已复制到剪贴板，请 {}",
        paste_shortcut()
    ))
}

pub async fn hide_pill_and_inject(app: tauri::AppHandle, text: String) -> Result<(), String> {
    let (hidden_tx, hidden_rx) = tokio::sync::oneshot::channel();
    let app_hide = app.clone();
    app.run_on_main_thread(move || {
        let _ = set_pill_visible(app_hide.clone(), false, "idle".into());
        let _ = hidden_tx.send(());
    })
    .map_err(|e| e.to_string())?;
    let _ = hidden_rx.await;
    // 胶囊不抢焦点；留一点时间让隐藏生效，再把前台交还给开口前的窗口
    tokio::time::sleep(Duration::from_millis(60)).await;

    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = done_tx.send(inject_text(text));
    })
    .map_err(|e| e.to_string())?;
    match tokio::time::timeout(Duration::from_secs(3), done_rx).await {
        Ok(Ok(result)) => result,
        _ => Err(format!(
            "等待注入超时，结果已复制到剪贴板，请 {}",
            paste_shortcut()
        )),
    }
}

static MAIN_THREAD_ID: std::sync::OnceLock<std::thread::ThreadId> = std::sync::OnceLock::new();

pub fn mark_main_thread() {
    let _ = MAIN_THREAD_ID.set(std::thread::current().id());
}

fn on_main_thread<T, F>(app: &tauri::AppHandle, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    if MAIN_THREAD_ID.get() == Some(&std::thread::current().id()) {
        return task();
    }
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = tx.send(task());
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(5))
        .map_err(|e| format!("等待窗口主线程操作失败：{e}"))?
}

#[derive(Clone, serde::Serialize)]
struct PillPhasePayload {
    phase: String,
    error: Option<String>,
}

fn place_pill(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> Result<(), String> {
    let monitor = app
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())
        .ok_or_else(|| "未找到显示器".to_string())?;
    let size = monitor.size();
    let scale = monitor.scale_factor();
    let pill_w = crate::window_position::PILL_LOGICAL_WIDTH;
    let pill_h = crate::window_position::PILL_LOGICAL_HEIGHT;
    #[cfg(target_os = "macos")]
    let (x, y) = {
        let origin = monitor.position();
        let visible = monitor.work_area();
        let position = crate::window_position::pill_position(
            crate::window_position::Area {
                left: origin.x as f64 / scale,
                top: origin.y as f64 / scale,
                width: size.width as f64 / scale,
                height: size.height as f64 / scale,
            },
            crate::window_position::Area {
                left: visible.position.x as f64 / scale,
                top: visible.position.y as f64 / scale,
                width: visible.size.width as f64 / scale,
                height: visible.size.height as f64 / scale,
            },
            pill_w,
            pill_h,
        );
        (position.x, position.y)
    };
    #[cfg(not(target_os = "macos"))]
    let x = (size.width as f64 / scale - pill_w) / 2.0;
    #[cfg(not(target_os = "macos"))]
    let y = size.height as f64 / scale - pill_h - 48.0;
    window
        .set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: pill_w,
            height: pill_h,
        }))
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_pill_visible(app: tauri::AppHandle, visible: bool, phase: String) -> Result<(), String> {
    set_pill_state(app, visible, phase, None)
}

pub fn set_pill_error(app: tauri::AppHandle, error: String) -> Result<(), String> {
    set_pill_state(app, true, "error".into(), Some(error))
}

fn set_pill_state(app: tauri::AppHandle, visible: bool, phase: String, error: Option<String>) -> Result<(), String> {
    let app_for_task = app.clone();
    on_main_thread(&app, move || set_pill_visible_on_main(&app_for_task, visible, phase, error))
}

fn set_pill_visible_on_main(app: &tauri::AppHandle, visible: bool, phase: String, error: Option<String>) -> Result<(), String> {
    let pill = app
        .get_webview_window("pill")
        .ok_or_else(|| "未找到胶囊窗口".to_string())?;
    if visible {
        let _ = pill.set_skip_taskbar(true);
        let _ = pill.set_always_on_top(true);
        let _ = pill.set_focusable(false);
        place_pill(&app, &pill)?;
        // pill.show() 会激活窗口，把正在输入的程序抢到后台。这里只做无焦点显示。
        #[cfg(target_os = "windows")]
        show_pill_without_focus(&pill)?;
        #[cfg(target_os = "macos")]
        show_pill_without_focus_mac(&pill)?;
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        pill.show().map_err(|e| e.to_string())?;
        let _ = app.emit_to(
            "pill",
            "pill-phase",
            PillPhasePayload {
                phase: phase.clone(),
                error,
            },
        );
    } else {
        let _ = app.emit_to(
            "pill",
            "pill-phase",
            PillPhasePayload {
                phase: "idle".into(),
                error: None,
            },
        );
        // 显示走的是 SW_SHOWNOACTIVATE，Tauri 仍认为窗口是隐藏的，只调 hide() 不会把它拿掉。
        #[cfg(target_os = "windows")]
        if let Ok(raw) = pill.hwnd() {
            let hwnd = hwnd_from_raw(raw.0 as isize);
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
        let _ = pill.hide();
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn show_pill_without_focus_mac(pill: &tauri::WebviewWindow) -> Result<(), String> {
    let _ = pill.set_focusable(false);
    let ptr = pill.ns_window().map_err(|e| e.to_string())?;
    crate::macos_app::order_front_without_key(ptr as *mut _);
    Ok(())
}

#[tauri::command]
pub fn position_pill_window(app: tauri::AppHandle) -> Result<(), String> {
    let app_for_task = app.clone();
    on_main_thread(&app, move || {
        let window = app_for_task
            .get_webview_window("pill")
            .ok_or_else(|| "未找到胶囊窗口".to_string())?;
        place_pill(&app_for_task, &window)
    })
}
