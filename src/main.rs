#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod autostart;
mod config;
mod hooks;
mod hotkey_recorder;
mod settings;
mod tray;
mod updater;

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
    System::{
        LibraryLoader::GetModuleHandleW,
        Threading::{CreateMutexW, ExitProcess},
    },
    UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, MessageBoxW, RegisterClassExW,
        RegisterWindowMessageW, SetTimer, TranslateMessage, HWND_MESSAGE, MB_ICONERROR,
        MB_ICONINFORMATION, MB_OK, MSG, WNDCLASSEXW, WM_USER,
    },
};

pub(crate) const WM_TRAY_MSG: u32 = WM_USER + 1;
pub(crate) const WM_UPDATE_RESULT: u32 = WM_USER + 2;
pub(crate) const ID_LOCK: usize = 1001;
pub(crate) const ID_UNLOCK: usize = 1002;
pub(crate) const ID_AUTOSTART: usize = 1003;
pub(crate) const ID_EXIT: usize = 1004;
pub(crate) const ID_SETTINGS: usize = 1005;
pub(crate) const TIMER_PANIC:    usize = 2001;
pub(crate) const TIMER_WATCHDOG: usize = 2002;

pub(crate) static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

pub(crate) fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    unsafe {
        // 1. Single-instance guard
        let mutex_name = to_wide("Global\\WraithSingleInstance");
        let _mutex = CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr());
        let mutex_err = GetLastError();
        if _mutex.is_null() {
            MessageBoxW(
                std::ptr::null_mut(),
                to_wide("Failed to create mutex.").as_ptr(),
                to_wide("Wraith").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            ExitProcess(1);
        }
        if mutex_err == ERROR_ALREADY_EXISTS {
            MessageBoxW(
                std::ptr::null_mut(),
                to_wide("Wraith is already running.").as_ptr(),
                to_wide("Wraith").as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
            ExitProcess(0);
        }

        // 2. Config -- load and cache in OnceLock
        config::Config::get();

        app::startup_cleanup();

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
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        if RegisterClassExW(&wc) == 0 {
            MessageBoxW(
                std::ptr::null_mut(),
                to_wide("Failed to register window class.").as_ptr(),
                to_wide("Wraith").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            ExitProcess(1);
        }

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            std::ptr::null(),
            0,
            0, 0, 0, 0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            MessageBoxW(
                std::ptr::null_mut(),
                to_wide("Failed to create message window.").as_ptr(),
                to_wide("Wraith").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            ExitProcess(1);
        }

        // 4. Register WM_TASKBARCREATED for Explorer crash recovery
        TASKBAR_CREATED.store(
            RegisterWindowMessageW(to_wide("TaskbarCreated").as_ptr()),
            Relaxed,
        );

        // 5. Create tray icon, store pointer in APP_TRAY
        hooks::APP_TRAY.store(Box::into_raw(Box::new(tray::TrayIcon::new(hwnd))) as usize, Relaxed);

        // 6. Install low-level hooks (also stores APP_HWND) — exit on failure
        if let Err(e) = hooks::install(hwnd) {
            MessageBoxW(
                std::ptr::null_mut(),
                to_wide(e).as_ptr(),
                to_wide("Wraith").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            ExitProcess(1);
        }

        // 7. Lock on start if configured
        if config::Config::get().lock_on_start {
            app::lock();
        }

        // 8. Spawn update checker (background thread)
        updater::spawn();

        // 9. Watchdog: reinstall hooks every 5s to recover from silent removal
        //    (e.g. after Parsec virtual driver teardown mutates the hook chain)
        SetTimer(hwnd, TIMER_WATCHDOG, 5000, None);

        // 10. Message pump -- drives WH_KEYBOARD_LL / WH_MOUSE_LL callbacks
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if r <= 0 { break; } // 0 = WM_QUIT, negative = error
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
