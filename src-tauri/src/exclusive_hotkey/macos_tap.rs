//! CGEventTap FFI（可返回 null 吞掉事件），与 core-graphics 高层封装不同。

use std::ffi::c_void;
use std::os::raw::c_long;

pub type CGEventMask = u64;
pub type CGKeyCode = u16;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub enum CGEventTapLocation {
    HID = 0,
    Session = 1,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CGEventType {
    Null = 0,
    KeyDown = 10,
    KeyUp = 11,
    FlagsChanged = 12,
    TapDisabledByTimeout = 0xFFFF_FFFE_u32 as u32,
    TapDisabledByUserInput = 0xFFFF_FFFF_u32 as u32,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug)]
pub enum CGEventTapPlacement {
    HeadInsertEventTap = 0,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug)]
pub enum CGEventTapOptions {
    Default = 0,
}

pub enum CGEvent {}
pub type CGEventRef = *const CGEvent;
pub type CGEventTapProxy = *const c_void;

pub type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    etype: CGEventType,
    event: CGEventRef,
    user_info: *const c_void,
) -> CGEventRef;

pub enum CFMachPort {}
pub type CFMachPortRef = *mut CFMachPort;
pub enum CFRunLoop {}
pub type CFRunLoopRef = *mut CFRunLoop;
pub type CFRunLoopMode = *const c_void;
pub enum CFAllocator {}
pub type CFAllocatorRef = *mut CFAllocator;
pub enum CFRunLoopSource {}
pub type CFRunLoopSourceRef = *mut CFRunLoopSource;
pub type CFIndex = c_long;

pub const KEYBOARD_EVENT_KEYCODE: u32 = 9;
pub const KEY_FUNCTION: CGKeyCode = 0x3F;

#[repr(C)]
#[derive(Clone, Copy)]
pub enum CGEventSourceStateID {
    CombinedSessionState = 0,
    HIDSystemState = 1,
}

pub enum CGEventSource {}
pub type CGEventSourceRef = *mut CGEventSource;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub fn CGEventTapCreate(
        tap: CGEventTapLocation,
        place: CGEventTapPlacement,
        options: CGEventTapOptions,
        events_of_interest: CGEventMask,
        callback: CGEventTapCallBack,
        user_info: *const c_void,
    ) -> CFMachPortRef;
    pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    pub fn CGEventSourceKeyState(state: CGEventSourceStateID, key: CGKeyCode) -> bool;
    pub fn CGEventSourceCreate(stateID: CGEventSourceStateID) -> CGEventSourceRef;
    pub fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        keycode: CGKeyCode,
        keydown: bool,
    ) -> CGEventRef;
    pub fn CGEventPost(tap: CGEventTapLocation, event: CGEventRef) -> ();
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub static kCFRunLoopCommonModes: CFRunLoopMode;
    pub static kCFAllocatorDefault: CFAllocatorRef;
    pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    pub fn CFRunLoopRun() -> ();
    pub fn CFRunLoopStop(rl: CFRunLoopRef) -> ();
    pub fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    pub fn CFMachPortInvalidate(port: CFMachPortRef);
    pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFRunLoopMode);
    pub fn CFRunLoopRemoveSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFRunLoopMode);
    pub fn CFRelease(cftype: *const c_void);
}

pub fn event_mask_for(types: &[CGEventType]) -> CGEventMask {
    types.iter().fold(0u64, |mask, t| mask | (1u64 << (*t as u32)))
}
