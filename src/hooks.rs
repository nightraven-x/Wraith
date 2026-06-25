use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use windows_sys::Win32::Foundation::HWND;

pub static LOCKED:      AtomicBool  = AtomicBool::new(false);
pub static KB_HOOK:     AtomicUsize = AtomicUsize::new(0); // HHOOK as usize
pub static MOUSE_HOOK:  AtomicUsize = AtomicUsize::new(0); // HHOOK as usize
pub static APP_HWND:    AtomicUsize = AtomicUsize::new(0); // HWND as usize
pub static PANIC_START: AtomicU32   = AtomicU32::new(0);   // GetTickCount() snapshot

pub fn install(_hwnd: HWND) -> Result<(), &'static str> {
    // TODO (issue #4): SetWindowsHookExW(WH_KEYBOARD_LL, keyboard_proc, NULL, 0)
    // TODO (issue #4): SetWindowsHookExW(WH_MOUSE_LL, mouse_proc, NULL, 0)
    Ok(())
}

pub fn uninstall() {
    // TODO (issue #4): UnhookWindowsHookEx(KB_HOOK), UnhookWindowsHookEx(MOUSE_HOOK)
}

// keyboard_proc and mouse_proc implemented in issue #4
