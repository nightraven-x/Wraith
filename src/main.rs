#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod config;
mod hooks;
mod tray;
mod updater;

use std::sync::atomic::Ordering::Relaxed;
use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
    System::{
        LibraryLoader::GetModuleHandleW,
        Threading::{CreateMutexW, ExitProcess},
    },
    UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, MessageBoxW,
        RegisterClassExW, TranslateMessage, HWND_MESSAGE, MB_ICONERROR, MB_OK, MSG,
        WNDCLASSEXW, WM_USER,
    },
};

pub(crate) const WM_TRAY_MSG: u32 = WM_USER + 1;
pub(crate) const WM_UPDATE_RESULT: u32 = WM_USER + 2;
pub(crate) const ID_LOCK: usize = 1001;
pub(crate) const ID_UNLOCK: usize = 1002;
pub(crate) const ID_AUTOSTART: usize = 1003;
pub(crate) const ID_EXIT: usize = 1004;
pub(crate) const TIMER_PANIC: usize = 2001;

pub(crate) fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    unsafe {
        // 1. Single-instance guard
        let mutex_name = to_wide("Global\\WraithSingleInstance");
        CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            MessageBoxW(
                0,
                to_wide("Wraith is already running.").as_ptr(),
                to_wide("Wraith").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            ExitProcess(0);
        }

        // 2. Config — load and cache in OnceLock
        config::Config::get();

        // 3. Register window class + create message-only window
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name = to_wide("WraithWindow");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(app::wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 0,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: 0,
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            std::ptr::null(),
            0,
            0, 0, 0, 0,
            HWND_MESSAGE,
            0,
            hinstance,
            std::ptr::null(),
        );

        // 4. Store HWND for hook callbacks and updater thread
        hooks::APP_HWND.store(hwnd as usize, Relaxed);

        // 5. Create tray icon, store pointer in GWLP_USERDATA
        let tray = Box::new(tray::TrayIcon::new(hwnd));
        app::store_tray(hwnd, tray);

        // 6. Install low-level hooks — exit on failure (running hookless is silent failure)
        if let Err(e) = hooks::install(hwnd) {
            MessageBoxW(
                hwnd,
                to_wide(e).as_ptr(),
                to_wide("Wraith").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            ExitProcess(1);
        }

        // 7. Lock on start if configured
        if config::Config::get().lock_on_start {
            app::lock(hwnd);
        }

        // 8. Spawn update checker (background thread)
        updater::spawn(hwnd);

        // 9. Message pump — drives WH_KEYBOARD_LL / WH_MOUSE_LL callbacks
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
