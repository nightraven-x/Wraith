use std::sync::atomic::Ordering::Relaxed;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::{
        Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED},
        Registry::{
            RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD,
        },
    },
    UI::WindowsAndMessaging::{
        DefWindowProcW, DestroyWindow, KillTimer, PostQuitMessage,
        SetTimer, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY,
        WM_LBUTTONDBLCLK, WM_RBUTTONUP, WM_TIMER,
    },
};

use crate::{
    hooks::{self, APP_HWND, APP_TRAY, LOCKED},
    to_wide,
    tray::TrayIcon,
    ID_AUTOSTART, ID_EXIT, ID_LOCK, ID_SETTINGS, ID_UNLOCK, TIMER_PANIC, TIMER_WATCHDOG,
    WM_TRAY_MSG, WM_UPDATE_RESULT,
};

const POLICY_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System";
const DISABLE_TM: &str = "DisableTaskMgr";

fn task_mgr_block() {
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        // Create key if absent (Policies\System may not exist on clean installs).
        if RegCreateKeyExW(
            HKEY_CURRENT_USER, to_wide(POLICY_KEY).as_ptr(),
            0, std::ptr::null_mut(), 0, KEY_SET_VALUE,
            std::ptr::null_mut(), &mut hkey, std::ptr::null_mut(),
        ) != 0 { return; }
        let val: u32 = 1;
        RegSetValueExW(hkey, to_wide(DISABLE_TM).as_ptr(), 0, REG_DWORD,
            (&val as *const u32).cast(), 4);
        RegCloseKey(hkey);
    }
}

fn task_mgr_unblock() {
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER, to_wide(POLICY_KEY).as_ptr(),
            0, KEY_SET_VALUE, &mut hkey,
        ) != 0 { return; }
        RegDeleteValueW(hkey, to_wide(DISABLE_TM).as_ptr());
        RegCloseKey(hkey);
    }
}

/// Remove DisableTaskMgr on startup — cleans up if Wraith crashed while locked.
pub(crate) fn startup_cleanup() {
    task_mgr_unblock();
}

pub fn lock() {
    if LOCKED.load(Relaxed) { return; }
    LOCKED.store(true, Relaxed);
    task_mgr_block();
    let hwnd = APP_HWND.load(Relaxed) as HWND;
    unsafe {
        SetTimer(hwnd, TIMER_PANIC, 100, None);
        SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
        tray().set_locked(true);
    }
}

pub fn unlock() {
    if !LOCKED.load(Relaxed) { return; }
    LOCKED.store(false, Relaxed);
    task_mgr_unblock();
    let hwnd = APP_HWND.load(Relaxed) as HWND;
    unsafe { KillTimer(hwnd, TIMER_PANIC); }
    hooks::panic_reset();
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS);
        tray().set_locked(false);
    }
}

pub fn toggle() {
    if LOCKED.load(Relaxed) { unlock(); } else { lock(); }
}

pub unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY_MSG => {
            let event = (lp as u32) & 0xFFFF;
            if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                tray().show_menu(hwnd);
            } else if event == WM_LBUTTONDBLCLK {
                toggle();
            }
            0
        }

        WM_COMMAND => {
            let id = wp & 0xFFFF;
            if id == ID_LOCK {
                lock();
            } else if id == ID_UNLOCK {
                unlock();
            } else if id == ID_AUTOSTART {
                if crate::autostart::is_enabled() { crate::autostart::disable(); }
                else { crate::autostart::enable(); }
            } else if id == ID_SETTINGS {
                crate::settings::show(hwnd);
            } else if id == ID_EXIT {
                DestroyWindow(hwnd);
            }
            0
        }

        WM_TIMER => {
            if wp == TIMER_PANIC && LOCKED.load(Relaxed) && hooks::panic_key_tick() {
                unlock();
            } else if wp == TIMER_WATCHDOG {
                hooks::watchdog();
            }
            0
        }

        WM_UPDATE_RESULT => {
            if lp != 0 {
                let s = Box::from_raw(lp as *mut String);
                tray().show_balloon("Wraith Update", &s);
            }
            0
        }

        WM_DESTROY => {
            hooks::uninstall();
            let ptr = APP_TRAY.swap(0, Relaxed) as *mut TrayIcon;
            if !ptr.is_null() {
                drop(Box::from_raw(ptr)); // Drop impl handles NIM_DELETE
            }
            PostQuitMessage(0);
            0
        }

        _ => {
            let tc = crate::TASKBAR_CREATED.load(Relaxed);
            if tc != 0 && msg == tc {
                tray().re_add();
                return 0;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
}

fn tray() -> &'static mut TrayIcon {
    unsafe { &mut *(APP_TRAY.load(Relaxed) as *mut TrayIcon) }
}

