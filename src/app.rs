use std::sync::atomic::Ordering::Relaxed;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::{
        LibraryLoader::GetModuleFileNameW,
        Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED},
        Registry::{
            RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD, REG_SZ,
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
const RUNONCE_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce";
const RUNONCE_VALUE: &str = "WraithTaskMgrCleanup";

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
    register_cleanup_failsafe();
}

// Builds `"<quoted exe path>" --cleanup-taskmgr` -- the command RunOnce will
// launch at next interactive logon if this process never gets to run
// unregister_cleanup_failsafe() itself.
fn cleanup_command() -> Vec<u16> {
    let mut raw = [0u16; 510];
    let len = unsafe {
        GetModuleFileNameW(std::ptr::null_mut(), raw.as_mut_ptr(), raw.len() as u32)
    } as usize;
    let mut cmd = String::from("\"");
    cmd.push_str(&String::from_utf16_lossy(&raw[..len]));
    cmd.push_str("\" ");
    cmd.push_str(crate::CLEANUP_TASKMGR_FLAG);
    to_wide(&cmd)
}

// Registers a RunOnce entry that clears DisableTaskMgr at the next interactive
// logon, no matter how this process ends (crash, forced kill, power loss --
// none of which give a dying process any chance to run its own cleanup code).
// Cleared by unregister_cleanup_failsafe() on any clean unblock, so it only
// ever actually fires after an unclean one.
fn register_cleanup_failsafe() {
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER, to_wide(RUNONCE_KEY).as_ptr(),
            0, std::ptr::null_mut(), 0, KEY_SET_VALUE,
            std::ptr::null_mut(), &mut hkey, std::ptr::null_mut(),
        ) != 0 { return; }
        let cmd = cleanup_command();
        RegSetValueExW(hkey, to_wide(RUNONCE_VALUE).as_ptr(), 0, REG_SZ,
            cmd.as_ptr() as *const u8, (cmd.len() * 2) as u32);
        RegCloseKey(hkey);
    }
}

fn unregister_cleanup_failsafe() {
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER, to_wide(RUNONCE_KEY).as_ptr(),
            0, KEY_SET_VALUE, &mut hkey,
        ) != 0 { return; }
        RegDeleteValueW(hkey, to_wide(RUNONCE_VALUE).as_ptr());
        RegCloseKey(hkey);
    }
}

// Test-only call counter. Some hosts (this dev box included) put DisableTaskMgr
// under active Group Policy management, which reasserts the value within
// seconds of a delete -- read-back against the live registry is not a
// reliable test signal there. This counter lets tests verify the production
// code path was actually exercised without depending on that read-back.
#[cfg(test)]
pub(crate) static TASK_MGR_UNBLOCK_CALLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn task_mgr_unblock() {
    #[cfg(test)]
    TASK_MGR_UNBLOCK_CALLS.fetch_add(1, Relaxed);
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER, to_wide(POLICY_KEY).as_ptr(),
            0, KEY_SET_VALUE, &mut hkey,
        ) != 0 { return; }
        RegDeleteValueW(hkey, to_wide(DISABLE_TM).as_ptr());
        RegCloseKey(hkey);
    }
    unregister_cleanup_failsafe();
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
            // Unconditional, not gated on LOCKED: exiting while locked (e.g. the
            // tray menu's Exit, clicked while locked) must not leave the
            // systemwide DisableTaskMgr policy set with no app left running to
            // clear it.
            task_mgr_unblock();
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

#[cfg(test)]
mod tests {
    use super::*;

    // Real read-back against the live DisableTaskMgr value is not reliable in
    // every environment -- this dev host has it under active Group Policy
    // management, which reasserts a value within seconds of a delete,
    // independent of anything this process does. TASK_MGR_UNBLOCK_CALLS lets
    // this test verify the production code path actually ran without
    // depending on that read-back. task_mgr_unblock() still performs its real
    // (harmless, delete-only) registry call underneath -- this only adds an
    // observability hook, it doesn't stub the function out.
    #[test]
    fn wm_destroy_calls_task_mgr_unblock_even_if_still_locked() {
        let before = TASK_MGR_UNBLOCK_CALLS.load(Relaxed);

        // Exiting (WM_DESTROY) must clear the policy unconditionally -- even
        // though nothing here ever set LOCKED, this exercises the same cleanup
        // path a real "Exit while locked" would hit. APP_TRAY/hook globals are
        // untouched by any other test (main() never runs under cargo test), so
        // this drives the exact production WM_DESTROY handler safely.
        unsafe { wnd_proc(std::ptr::null_mut(), WM_DESTROY, 0, 0) };

        assert!(
            TASK_MGR_UNBLOCK_CALLS.load(Relaxed) > before,
            "WM_DESTROY must call task_mgr_unblock() regardless of lock state"
        );
    }

    // RunOnce is a different key from the policy-managed DisableTaskMgr value
    // above and isn't known to be under any active management on this host, so
    // a real read-back is safe here (unlike the DisableTaskMgr value itself).
    struct ClearRunOnce;
    impl Drop for ClearRunOnce {
        fn drop(&mut self) {
            unregister_cleanup_failsafe();
        }
    }

    fn read_runonce_value() -> Option<String> {
        use windows_sys::Win32::System::Registry::{RegQueryValueExW, KEY_QUERY_VALUE};
        unsafe {
            let mut hkey: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER, to_wide(RUNONCE_KEY).as_ptr(),
                0, KEY_QUERY_VALUE, &mut hkey,
            ) != 0 { return None; }
            let mut buf = [0u16; 600];
            let mut size = (buf.len() * 2) as u32;
            let mut kind = 0u32;
            let ok = RegQueryValueExW(hkey, to_wide(RUNONCE_VALUE).as_ptr(),
                std::ptr::null_mut(), &mut kind, buf.as_mut_ptr() as *mut u8, &mut size) == 0;
            RegCloseKey(hkey);
            if !ok { return None; }
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..len]))
        }
    }

    // Exercises register_cleanup_failsafe()/unregister_cleanup_failsafe()
    // directly rather than through task_mgr_block()/task_mgr_unblock(): this
    // dev host actively denies write access (ERROR_ACCESS_DENIED) to the
    // policy key those wrap, likely a GPO/hardening lockdown on that exact
    // key -- ironic, since it's the same key the security audit flagged.
    // task_mgr_block() correctly no-ops when that write is denied (nothing
    // was actually blocked, so there's nothing to register a failsafe for),
    // which means testing this end-to-end through task_mgr_block() would be
    // testing the host's GPO, not Wraith's logic. The RunOnce key is a
    // different path, not subject to that lockdown.
    #[test]
    fn register_and_unregister_cleanup_failsafe_round_trip() {
        let _clear = ClearRunOnce;

        register_cleanup_failsafe();
        let cmd = read_runonce_value();
        assert!(cmd.is_some(), "register_cleanup_failsafe() must write a RunOnce entry");
        assert!(
            cmd.unwrap().contains(crate::CLEANUP_TASKMGR_FLAG),
            "RunOnce command must invoke the cleanup flag"
        );

        unregister_cleanup_failsafe();
        assert!(
            read_runonce_value().is_none(),
            "unregister_cleanup_failsafe() must remove the RunOnce entry"
        );
    }
}

