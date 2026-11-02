//! Only the small public marker is polled; full JSON is read on paste.
use super::Summary;
use std::{
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

#[link(name = "user32")]
unsafe extern "system" {
    fn GetClipboardSequenceNumber() -> u32;
    fn OpenClipboard(window: *mut c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn GetClipboardData(format: u32) -> *mut c_void;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalSize(handle: *mut c_void) -> usize;
    fn GlobalLock(handle: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(handle: *mut c_void) -> i32;
}
struct Clipboard;
impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

pub(super) fn summary() -> Option<Summary> {
    static CACHE: OnceLock<Mutex<(u32, Option<Summary>)>> = OnceLock::new();
    let sequence = unsafe { GetClipboardSequenceNumber() };
    let mut cached = CACHE.get_or_init(|| Mutex::new((0, None))).lock().ok()?;
    if cached.0 == sequence {
        return cached.1.clone();
    }
    if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        return None;
    }
    let _clipboard = Clipboard;
    let value = read();
    *cached = (sequence, value.clone());
    value
}

fn read() -> Option<Summary> {
    if unsafe { IsClipboardFormatAvailable(13) } == 0 {
        return None;
    }
    let handle = unsafe { GetClipboardData(13) };
    if handle.is_null() {
        return None;
    }
    let size = unsafe { GlobalSize(handle) };
    if !(18..=4096).contains(&size) || !size.is_multiple_of(2) {
        return None;
    }
    let pointer = unsafe { GlobalLock(handle) };
    if pointer.is_null() {
        return None;
    }
    let words = unsafe { std::slice::from_raw_parts(pointer.cast::<u16>(), size / 2) };
    let end = words.iter().position(|v| *v == 0).unwrap_or(words.len());
    let text = String::from_utf16(&words[..end]).ok();
    unsafe {
        GlobalUnlock(handle);
    }
    let text = text?;
    let header = text
        .strip_prefix(super::PREFIX)
        .or_else(|| text.strip_prefix("SVR2CLIP\r\n"))?;
    serde_json::from_str(header).ok()
}
