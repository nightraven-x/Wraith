use std::mem::size_of;
use windows_sys::Win32::{
    Foundation::{HWND, POINT},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Shell::{
            Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO,
            NIIF_NOSOUND, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NOTIFYICONDATAW,
        },
        WindowsAndMessaging::{
            AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HICON, IDI_APPLICATION,
            LoadIconW, LoadImageW, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING,
            SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON,
            TrackPopupMenu,
        },
    },
};

use crate::{to_wide, ID_AUTOSTART, ID_EXIT, ID_LOCK, ID_SETTINGS, ID_UNLOCK, WM_TRAY_MSG};

const ICON_ID: u32 = 1;
const NOTIFYICON_VERSION_4: u32 = 4;

pub struct TrayIcon {
    hwnd:            HWND,
    h_icon_unlocked: HICON,
    h_icon_locked:   HICON,
    locked:          bool,
}

impl TrayIcon {
    pub fn new(hwnd: HWND) -> Self {
        let (h_icon_unlocked, h_icon_locked) = load_icons();
        let t = TrayIcon { hwnd, h_icon_unlocked, h_icon_locked, locked: false };
        t.add_to_shell();
        t
    }

    fn add_to_shell(&self) {
        let mut nid = blank_nid(self.hwnd);
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY_MSG;
        nid.hIcon = if self.locked { self.h_icon_locked } else { self.h_icon_unlocked };
        let tip = if self.locked { "Wraith - Locked" } else { "Wraith - Unlocked" };
        copy_wide(&to_wide(tip), &mut nid.szTip);
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &nid);
            // NIM_SETVERSION must follow NIM_ADD; enables WM_CONTEXTMENU + NIN_* on Vista+
            let mut ver_nid = blank_nid(self.hwnd);
            ver_nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &ver_nid);
        }
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
        let icon = if locked { self.h_icon_locked } else { self.h_icon_unlocked };
        let tip = if locked { "Wraith - Locked" } else { "Wraith - Unlocked" };
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

    pub fn show_menu(&self, hwnd: HWND) {
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return;
            }

            let lock_flags     = MF_STRING | if self.locked  { MF_GRAYED  } else { 0 };
            let unlock_flags   = MF_STRING | if !self.locked { MF_GRAYED  } else { 0 };
            let autostart_flags =
                MF_STRING | if crate::autostart::is_enabled() { MF_CHECKED } else { 0 };

            AppendMenuW(menu, lock_flags,      ID_LOCK,      to_wide("Lock").as_ptr());
            AppendMenuW(menu, unlock_flags,    ID_UNLOCK,    to_wide("Unlock").as_ptr());
            // Reachable both locked and unlocked — tray interaction already bypasses
            // the input hooks, so no special-casing is needed here.
            AppendMenuW(menu, MF_STRING,       ID_SETTINGS,  to_wide("Settings...").as_ptr());
            AppendMenuW(menu, MF_SEPARATOR,    0,            std::ptr::null());
            AppendMenuW(menu, autostart_flags, ID_AUTOSTART, to_wide("Start with Windows").as_ptr());
            AppendMenuW(menu, MF_SEPARATOR,    0,            std::ptr::null());
            AppendMenuW(menu, MF_STRING,       ID_EXIT,      to_wide("Exit").as_ptr());

            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);

            SetForegroundWindow(hwnd);
            TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
                pt.x, pt.y,
                0, hwnd,
                std::ptr::null(),
            );
            DestroyMenu(menu);
        }
    }

    // Re-add the tray icon after Explorer restarts (WM_TASKBARCREATED).
    pub fn re_add(&self) {
        self.add_to_shell();
    }

}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        let nid = blank_nid(self.hwnd);
        unsafe { Shell_NotifyIconW(NIM_DELETE, &nid); }
    }
}

// Load unlocked (resource 1) and locked (resource 2) icons.
// Falls back to IDI_APPLICATION if the .ico resources are not yet embedded.
fn load_icons() -> (HICON, HICON) {
    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let try_load = |resource_id: u16| -> HICON {
            // MAKEINTRESOURCEW(n): cast integer to *const u16 pointer
            let h = LoadImageW(
                hinstance,
                resource_id as usize as *const u16,
                1u32, // IMAGE_ICON
                0, 0, // 0x0 = system default icon size
                0u32, // LR_DEFAULTCOLOR
            );
            if !h.is_null() { h } else { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) }
        };
        (try_load(1), try_load(2))
    }
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
