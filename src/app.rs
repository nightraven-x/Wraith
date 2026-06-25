use std::sync::atomic::Ordering::Relaxed;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::{
        LibraryLoader::GetModuleFileNameW,
        Power::{
            SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
        },
        Registry::{
            RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
            HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
        },
    },
    UI::WindowsAndMessaging::{
        DefWindowProcW, DestroyWindow, GetWindowLongPtrW, PostQuitMessage, SetWindowLongPtrW,
        GWLP_USERDATA, WM_COMMAND, WM_DESTROY, WM_LBUTTONDBLCLK, WM_RBUTTONUP, WM_CONTEXTMENU,
    },
};

use crate::{
    hooks::{self, LOCKED},
    to_wide,
    tray::TrayIcon,
    ID_AUTOSTART, ID_EXIT, ID_LOCK, ID_UNLOCK, WM_TRAY_MSG, WM_UPDATE_RESULT,
};

pub fn lock(hwnd: HWND) {
    LOCKED.store(true, Relaxed);
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
        tray_from_hwnd(hwnd).set_locked(true);
    }
}

pub fn unlock(hwnd: HWND) {
    LOCKED.store(false, Relaxed);
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS);
        tray_from_hwnd(hwnd).set_locked(false);
    }
}

pub fn toggle(hwnd: HWND) {
    if LOCKED.load(Relaxed) {
        unlock(hwnd);
    } else {
        lock(hwnd);
    }
}

pub fn set_autostart(enable: bool) {
    let run_key = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let value_name = to_wide("Wraith");
    unsafe {
        let mut hkey = 0isize;
        if RegOpenKeyExW(HKEY_CURRENT_USER, run_key.as_ptr(), 0, KEY_SET_VALUE, &mut hkey) != 0 {
            return;
        }
        if enable {
            let mut raw = [0u16; 510];
            let len = GetModuleFileNameW(0, raw.as_mut_ptr(), raw.len() as u32) as usize;
            // Wrap in double quotes so paths with spaces survive the Run key
            let mut quoted: Vec<u16> = Vec::with_capacity(len + 3);
            quoted.push(b'"' as u16);
            quoted.extend_from_slice(&raw[..len]);
            quoted.push(b'"' as u16);
            quoted.push(0u16);
            RegSetValueExW(
                hkey,
                value_name.as_ptr(),
                0,
                REG_SZ,
                quoted.as_ptr() as *const u8,
                (quoted.len() * 2) as u32,
            );
        } else {
            RegDeleteValueW(hkey, value_name.as_ptr());
        }
        RegCloseKey(hkey);
    }
}

pub fn is_autostart() -> bool {
    let run_key = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let value_name = to_wide("Wraith");
    unsafe {
        let mut hkey = 0isize;
        if RegOpenKeyExW(HKEY_CURRENT_USER, run_key.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) != 0 {
            return false;
        }
        let mut kind = 0u32;
        let mut size = 0u32;
        let found = RegQueryValueExW(
            hkey,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            std::ptr::null_mut(),
            &mut size,
        ) == 0;
        RegCloseKey(hkey);
        found
    }
}

pub fn store_tray(hwnd: HWND, tray: Box<TrayIcon>) {
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(tray) as isize);
    }
}

pub unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY_MSG => {
            let event = (lp as u32) & 0xFFFF;
            if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                tray_from_hwnd(hwnd).show_menu(hwnd, LOCKED.load(Relaxed));
            } else if event == WM_LBUTTONDBLCLK {
                toggle(hwnd);
            }
            0
        }

        WM_COMMAND => {
            let id = wp & 0xFFFF;
            if id == ID_LOCK {
                lock(hwnd);
            } else if id == ID_UNLOCK {
                unlock(hwnd);
            } else if id == ID_AUTOSTART {
                set_autostart(!is_autostart());
            } else if id == ID_EXIT {
                DestroyWindow(hwnd);
            }
            0
        }

        WM_UPDATE_RESULT => {
            if lp != 0 {
                let s = Box::from_raw(lp as *mut String);
                tray_from_hwnd(hwnd).show_balloon("Wraith Update", &s);
            }
            0
        }

        WM_DESTROY => {
            hooks::uninstall();
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayIcon;
            if !ptr.is_null() {
                (*ptr).destroy();
                drop(Box::from_raw(ptr));
            }
            PostQuitMessage(0);
            0
        }

        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn tray_from_hwnd(hwnd: HWND) -> &'static mut TrayIcon {
    &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayIcon)
}
