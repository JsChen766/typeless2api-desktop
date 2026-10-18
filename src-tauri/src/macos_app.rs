//! macOS：前台应用、激活、胶囊不抢焦点。依赖 AppKit，仅在 macOS 编译。
#![cfg(target_os = "macos")]

use std::ffi::CStr;
use std::os::raw::{c_char, c_void};

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

const ACTIVATE_IGNORING_OTHERS: u64 = 1;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
}

pub fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

#[derive(Clone, Debug)]
pub struct FrontApp {
    pub pid: i32,
    pub name: String,
    pub bundle_id: String,
}

pub fn frontmost_app() -> Option<FrontApp> {
    unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return None;
        }
        let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
        running_app_info(app)
    }
}

pub fn app_for_pid(pid: i32) -> Option<FrontApp> {
    unsafe {
        let app: *mut AnyObject =
            msg_send![class!(NSRunningApplication), runningApplicationWithProcessIdentifier: pid];
        running_app_info(app)
    }
}

pub fn activate_pid(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    unsafe {
        let app: *mut AnyObject =
            msg_send![class!(NSRunningApplication), runningApplicationWithProcessIdentifier: pid];
        if app.is_null() {
            return false;
        }
        let ok: bool = msg_send![app, activateWithOptions: ACTIVATE_IGNORING_OTHERS];
        ok
    }
}

pub fn is_our_bundle(bundle_id: &str) -> bool {
    bundle_id == "com.reverse-typeless.client"
}

/// 显示窗口但不使其成为 key window，避免听写胶囊抢走输入焦点。
pub fn order_front_without_key(ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    unsafe {
        let window = ns_window as *mut AnyObject;
        let _: () = msg_send![window, orderFrontRegardless];
    }
}

unsafe fn running_app_info(app: *mut AnyObject) -> Option<FrontApp> {
    if app.is_null() {
        return None;
    }
    let pid: i32 = msg_send![app, processIdentifier];
    if pid <= 0 {
        return None;
    }
    let name = nsstring_to_string(msg_send![app, localizedName])
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Desktop App".into());
    let bundle_id = nsstring_to_string(msg_send![app, bundleIdentifier])
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "-1".into());
    Some(FrontApp {
        pid,
        name,
        bundle_id,
    })
}

unsafe fn nsstring_to_string(s: *mut AnyObject) -> Option<String> {
    if s.is_null() {
        return None;
    }
    let utf8: *const c_char = msg_send![s, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}
