//! 开口前采集光标附近文字，对齐官方 audio_context.text_insertion_point.cursor_state。

const MAX_FIELD_CHARS: usize = 12_000;
const MAX_SIDE_CHARS: usize = 4_000;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DictationCursorState {
    pub cursor_position: i32,
    pub has_text_selected: bool,
    pub selected_text: String,
    pub text_before_cursor: String,
    pub text_after_cursor: String,
    pub full_field_content: String,
}

fn truncate_tail(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    s[s.len() - max..].to_string()
}

fn truncate_head(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    s[..max].to_string()
}

fn split_at_cursor(full: &str, start: usize, end: usize) -> DictationCursorState {
    let len = full.len();
    let start = start.min(len);
    let end = end.min(len).max(start);
    let selected = if end > start {
        truncate_head(&full[start..end], MAX_SIDE_CHARS)
    } else {
        String::new()
    };
    DictationCursorState {
        cursor_position: start as i32,
        has_text_selected: end > start,
        selected_text: selected,
        text_before_cursor: truncate_tail(&full[..start], MAX_SIDE_CHARS),
        text_after_cursor: truncate_head(&full[end..], MAX_SIDE_CHARS),
        full_field_content: truncate_head(full, MAX_FIELD_CHARS),
    }
}

#[cfg(target_os = "windows")]
pub fn capture_cursor_state(edit_hwnd: isize) -> Option<DictationCursorState> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsWindow;

    let hwnd = HWND(edit_hwnd as *mut _);
    if hwnd.0 == std::ptr::null_mut() || !unsafe { IsWindow(hwnd).as_bool() } {
        return None;
    }

    if let Some(state) = capture_win32_edit(hwnd) {
        if !state.full_field_content.is_empty()
            || !state.text_before_cursor.is_empty()
            || !state.text_after_cursor.is_empty()
        {
            return Some(state);
        }
    }
    capture_uia_focus_text()
}

#[cfg(target_os = "windows")]
fn capture_uia_focus_text() -> Option<DictationCursorState> {
    use uiautomation::patterns::{UITextPattern, UIValuePattern};
    use uiautomation::UIAutomation;
    let automation = UIAutomation::new().ok()?;
    let element = automation.get_focused_element().ok()?;
    if let Ok(pattern) = element.get_pattern::<UITextPattern>() {
        if let Ok(range) = pattern.get_document_range() {
            let full = range.get_text(-1).ok()?;
            if !full.is_empty() {
                let pos = full.len();
                if let Ok((is_active, caret)) = pattern.get_caret_range() {
                    if is_active {
                        if let Ok(before) = caret.get_text(-1) {
                            let start = before.len().min(full.len());
                            return Some(split_at_cursor(&full, start, start));
                        }
                    }
                }
                return Some(split_at_cursor(&full, pos, pos));
            }
        }
    }
    if let Ok(value) = element.get_pattern::<UIValuePattern>() {
        let text = value.get_value().ok()?;
        if !text.is_empty() {
            let pos = text.len();
            return Some(split_at_cursor(&text, pos, pos));
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn capture_win32_edit(hwnd: windows::Win32::Foundation::HWND) -> Option<DictationCursorState> {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_GETTEXT, WM_GETTEXTLENGTH};

    const EM_GETSEL: u32 = 0x00B0;

    let len = unsafe { SendMessageW(hwnd, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)) }.0 as usize;
    if len == 0 {
        return None;
    }
    let cap = len.min(MAX_FIELD_CHARS) + 1;
    let mut buf = vec![0u16; cap];
    let read = unsafe {
        SendMessageW(
            hwnd,
            WM_GETTEXT,
            WPARAM(cap),
            LPARAM(buf.as_mut_ptr() as isize),
        )
    }
    .0 as usize;
    if read == 0 {
        return None;
    }
    let full = String::from_utf16_lossy(&buf[..read]);
    let sel = unsafe { SendMessageW(hwnd, EM_GETSEL, WPARAM(0), LPARAM(0)) }.0 as u32;
    let start = (sel & 0xFFFF) as usize;
    let end = ((sel >> 16) & 0xFFFF) as usize;
    Some(split_at_cursor(&full, start, end))
}

#[cfg(not(target_os = "windows"))]
pub fn capture_cursor_state(_edit_hwnd: isize) -> Option<DictationCursorState> {
    None
}
