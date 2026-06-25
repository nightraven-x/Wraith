use std::mem::size_of;
use windows_sys::Win32::{
    Foundation::{HWND, POINT},
    UI::{
        Shell::{
            Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO,
            NIIF_NOSOUND, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NOTIFYICONDATAW,
        },
        WindowsAndMessaging::{
            AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HICON, LoadIconW,
            SetForegroundWindow, TrackPopupMenu, IDI_APPLICATION, MF_CHECKED, MF_GRAYED,
            MF_SEPARATOR, MF_STRING, TPM_LEFTALIGN, TPM_RIGHTBUTTON,
        },
    },
};

use crate::{to_wide, ID_AUTOSTART, ID_EXIT, ID_LOCK, ID_UNLOCK, WM_TRAY_MSG};

const ICON_ID: u32 = 1;
const NOTIFYICON_VERSION_4: u32 = 4;

pub struct TrayIcon {
    hwnd: HWND,
    locked: bool,
}

impl TrayIcon {
    pub fn new(hwnd: HWND) -> Self {
        let icon = load_icon();
        let mut nid = blank_nid(hwnd);
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY_MSG;
        nid.hIcon = icon;
        copy_wide(&to_wide("Wraith - Unlocked"), &mut nid.szTip);

        unsafe {
            Shell_NotifyIconW(NIM_ADD, &nid);

            // NIM_SETVERSION must follow NIM_ADD; uVersion lives in Anonymous union
            let mut ver_nid = blank_nid(hwnd);
            ver_nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &ver_nid);
        }

        TrayIcon { hwnd, locked: false }
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
        let tip = if locked { "Wraith - Locked" } else { "Wraith - Unlocked" };
        let icon = load_icon();
        let mut nid = blank_nid(self.hwnd);
        nid.uFlags = NIF_ICON | NIF_TIP;
        nid.hIcon = icon;
        copy_wide(&to_wide(tip), &mut nid.szTip);
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid); }
    }

    pub fn show_balloon(&self, title: &str, msg: &str) {
        let mut nid = blank_nid(self.hwnd);
        nid.uFlags = NIF_INFO;
        copy_wide(&to_wide(title), &mut nid.szInfoTitle);
        copy_wide(&to_wide(msg), &mut nid.szInfo);
        nid.dwInfoFlags = NIIF_INFO | NIIF_NOSOUND;
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid); }
    }

    pub fn show_menu(&self, hwnd: HWND, locked: bool) {
        unsafe {
            let menu = CreatePopupMenu();
            if menu == 0 {
                return;
            }

            let lock_flags = MF_STRING | if locked { MF_GRAYED } else { 0 };
            let unlock_flags = MF_STRING | if !locked { MF_GRAYED } else { 0 };
            let autostart_flags =
                MF_STRING | if crate::app::is_autostart() { MF_CHECKED } else { 0 };

            AppendMenuW(menu, lock_flags, ID_LOCK, to_wide("Lock").as_ptr());
            AppendMenuW(menu, unlock_flags, ID_UNLOCK, to_wide("Unlock").as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, autostart_flags, ID_AUTOSTART, to_wide("Start with Windows").as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, MF_STRING, ID_EXIT, to_wide("Exit").as_ptr());

            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);

            SetForegroundWindow(hwnd);
            TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, std::ptr::null());
            DestroyMenu(menu);
        }
    }

    pub fn destroy(&mut self) {
        let nid = blank_nid(self.hwnd);
        unsafe { Shell_NotifyIconW(NIM_DELETE, &nid); }
    }
}

fn load_icon() -> HICON {
    unsafe { LoadIconW(0, IDI_APPLICATION) }
}

fn blank_nid(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = ICON_ID;
    nid
}

fn copy_wide(src: &[u16], dst: &mut [u16]) {
    let len = src.len().min(dst.len() - 1);
    dst[..len].copy_from_slice(&src[..len]);
    dst[len] = 0;
}
