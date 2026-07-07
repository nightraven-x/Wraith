// Native Win32 settings dialog (tray "Settings..." entry).
//
// Slice 1: only the panic-key field. Lock/unlock combo recorder fields are a
// separate, parallel slice — not wired in here.
//
// DialogBoxParamW is modal: it pumps its own message loop on the calling
// thread until EndDialog() is called, so this is safe to call directly from
// wnd_proc (main thread) without blocking the hook pump — hooks live on the
// same thread's message queue, and DialogBoxParamW still dispatches all
// messages on that queue while the dialog is up.

use std::sync::atomic::Ordering::Relaxed;
use windows_sys::Win32::{
    Foundation::{BOOL, HWND, LPARAM, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        DialogBoxParamW, EndDialog, GetDlgItemInt, SetDlgItemInt, IDCANCEL, IDOK, WM_CLOSE,
        WM_COMMAND, WM_INITDIALOG,
    },
};

use crate::config::Config;

// Must match the numeric IDs in src/resource.rc's DIALOGEX block.
pub(crate) const IDD_SETTINGS: usize = 101;
pub(crate) const IDC_PANIC_VK: i32 = 1101;

// Test-only handshake: DialogBoxParamW is modal and pumps its own message
// loop on whatever thread calls it, so a test driving the dialog from a
// second thread has no other way to learn the real HWND once it exists.
// Set in WM_INITDIALOG, consumed by the test thread. Compiled out of release
// builds entirely.
#[cfg(test)]
static TEST_HWND: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Open the settings dialog modally. Blocks the caller (main thread) until
/// the user closes it via OK or Cancel; the dialog pumps its own messages
/// in the meantime, so this is safe to call from wnd_proc.
pub fn show(hwnd: HWND) {
    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        DialogBoxParamW(
            hinstance,
            IDD_SETTINGS as *const u16,
            hwnd,
            Some(dlg_proc),
            0,
        );
    }
}

unsafe extern "system" fn dlg_proc(hwnd: HWND, msg: u32, wp: WPARAM, _lp: LPARAM) -> isize {
    match msg {
        WM_INITDIALOG => {
            let cfg = Config::get();
            SetDlgItemInt(hwnd, IDC_PANIC_VK, cfg.panic_vk.load(Relaxed), 0);
            #[cfg(test)]
            TEST_HWND.store(hwnd as usize, Relaxed);
            1 // let Windows set default keyboard focus
        }

        WM_COMMAND => {
            let id = (wp & 0xFFFF) as i32;
            if id == IDOK {
                let mut translated: BOOL = 0;
                let val = GetDlgItemInt(hwnd, IDC_PANIC_VK, &mut translated, 0);
                // Plausible VK code: parsed cleanly and in range 0-255.
                if translated != 0 && val <= 255 {
                    let cfg = Config::get();
                    cfg.panic_vk.store(val, Relaxed);
                    cfg.write_back();
                    EndDialog(hwnd, 1);
                }
                // Invalid input: leave the dialog open, no state changed.
                1
            } else if id == IDCANCEL {
                EndDialog(hwnd, 0);
                1
            } else {
                0
            }
        }

        WM_CLOSE => {
            EndDialog(hwnd, 0);
            1
        }

        _ => 0,
    }
}

// Behaviors 4 & 5: drive the real modal dialog end-to-end. DialogBoxParamW
// blocks the calling thread pumping its own message queue, so it runs on a
// background thread here while the test thread drives it via real Win32
// messages (SetDlgItemInt + a real BM_CLICK on the OK/Cancel button) — no
// mocks, the same DlgProc production code runs both paths.
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::System::WindowsProgramming::GetPrivateProfileIntW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetDlgItem, SendMessageW, BM_CLICK};

    fn wait_for_test_hwnd() -> HWND {
        let start = Instant::now();
        loop {
            let h = TEST_HWND.swap(0, Relaxed);
            if h != 0 {
                return h as HWND;
            }
            if start.elapsed() > Duration::from_secs(5) {
                panic!("settings dialog never signalled WM_INITDIALOG");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn click(hwnd: HWND, id: i32) {
        unsafe {
            let btn = GetDlgItem(hwnd, id);
            assert!(!btn.is_null(), "dialog control {id} not found");
            SendMessageW(btn, BM_CLICK, 0, 0);
        }
    }

    fn read_panic_key_from_ini() -> i32 {
        let ini = crate::config::exe_relative("wraith.ini");
        let sec = crate::to_wide("Wraith");
        let key = crate::to_wide("PanicKey");
        unsafe { GetPrivateProfileIntW(sec.as_ptr(), key.as_ptr(), -1, ini.as_ptr()) }
    }

    #[test]
    fn ok_click_updates_atomic_and_persists_to_ini() {
        let _g = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        Config::get().panic_vk.store(27, Relaxed);
        Config::get().write_back();

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        unsafe { SetDlgItemInt(hwnd, IDC_PANIC_VK, 99, 0) };
        click(hwnd, IDOK);
        t.join().unwrap();

        assert_eq!(Config::get().panic_vk.load(Relaxed), 99);
        assert_eq!(read_panic_key_from_ini(), 99);

        Config::get().panic_vk.store(27, Relaxed);
        Config::get().write_back();
    }

    #[test]
    fn cancel_click_leaves_atomic_and_ini_unchanged() {
        let _g = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        Config::get().panic_vk.store(27, Relaxed);
        Config::get().write_back();

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        // Edit the field but cancel — neither the atomic nor the ini should move.
        unsafe { SetDlgItemInt(hwnd, IDC_PANIC_VK, 250, 0) };
        click(hwnd, IDCANCEL);
        t.join().unwrap();

        assert_eq!(Config::get().panic_vk.load(Relaxed), 27);
        assert_eq!(read_panic_key_from_ini(), 27);
    }

    #[test]
    fn invalid_value_rejected_dialog_stays_open() {
        let _g = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        Config::get().panic_vk.store(27, Relaxed);
        Config::get().write_back();

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        // 999 is out of the plausible VK range (0-255) -- OK must reject it
        // and keep the dialog open rather than accept/clamp/silently correct it.
        unsafe { SetDlgItemInt(hwnd, IDC_PANIC_VK, 999, 0) };
        click(hwnd, IDOK);

        // Config must be untouched by the rejected attempt.
        assert_eq!(Config::get().panic_vk.load(Relaxed), 27);
        assert_eq!(read_panic_key_from_ini(), 27);

        // Dialog is still up -- close it via Cancel so the thread can join.
        click(hwnd, IDCANCEL);
        t.join().unwrap();
    }
}
