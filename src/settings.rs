// Native Win32 settings dialog (tray "Settings..." entry).
//
// Slice 3: panic-key field plus lock/unlock combo recorder fields, sharing
// one OK/Cancel commit. The two combo fields use the reusable hotkey_recorder
// subclass (see hotkey_recorder.rs) to capture a live key combo into an EDIT
// control; this module owns the policy that control deliberately doesn't
// enforce itself — see the zero-modifier rejection in dlg_proc below.
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
    UI::Controls::{CheckDlgButton, IsDlgButtonChecked, BST_CHECKED, BST_UNCHECKED},
    UI::WindowsAndMessaging::{
        DialogBoxParamW, EndDialog, GetDlgItem, GetDlgItemInt, SetDlgItemInt, SetDlgItemTextW,
        IDCANCEL, IDOK, WM_CLOSE, WM_COMMAND, WM_INITDIALOG,
    },
};

use crate::config::Config;
use crate::hotkey_recorder;

// Must match the numeric IDs in src/resource.rc's DIALOGEX block.
pub(crate) const IDD_SETTINGS: usize = 101;
pub(crate) const IDC_PANIC_VK: i32 = 1101;
pub(crate) const IDC_LOCK_COMBO: i32 = 1102;
pub(crate) const IDC_UNLOCK_COMBO: i32 = 1103;
pub(crate) const IDC_VALIDATION_ERROR: i32 = 1104;
pub(crate) const IDC_LOCK_ON_START: i32 = 1105;

const ERR_ZERO_MODIFIER: &str = "Lock and unlock combos both need at least one modifier key.";
const ERR_PANIC_RANGE: &str = "Panic key must be a VK code between 0 and 255.";

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

fn set_error(hwnd: HWND, msg: &str) {
    let wide = crate::to_wide(msg);
    unsafe { SetDlgItemTextW(hwnd, IDC_VALIDATION_ERROR, wide.as_ptr()) };
}

unsafe extern "system" fn dlg_proc(hwnd: HWND, msg: u32, wp: WPARAM, _lp: LPARAM) -> isize {
    match msg {
        WM_INITDIALOG => {
            let cfg = Config::get();
            SetDlgItemInt(hwnd, IDC_PANIC_VK, cfg.panic_vk.load(Relaxed), 0);

            let lock_edit = GetDlgItem(hwnd, IDC_LOCK_COMBO);
            hotkey_recorder::install(
                lock_edit,
                (cfg.lock_mods.load(Relaxed), cfg.lock_vk.load(Relaxed)),
            );

            let unlock_edit = GetDlgItem(hwnd, IDC_UNLOCK_COMBO);
            hotkey_recorder::install(
                unlock_edit,
                (cfg.unlock_mods.load(Relaxed), cfg.unlock_vk.load(Relaxed)),
            );

            CheckDlgButton(
                hwnd,
                IDC_LOCK_ON_START,
                if cfg.lock_on_start.load(Relaxed) {
                    BST_CHECKED
                } else {
                    BST_UNCHECKED
                },
            );

            set_error(hwnd, "");
            #[cfg(test)]
            TEST_HWND.store(hwnd as usize, Relaxed);
            1 // let Windows set default keyboard focus
        }

        WM_COMMAND => {
            let id = (wp & 0xFFFF) as i32;
            if id == IDOK {
                let mut translated: BOOL = 0;
                let panic_val = GetDlgItemInt(hwnd, IDC_PANIC_VK, &mut translated, 0);
                let panic_ok = translated != 0 && panic_val <= 255;

                let lock_edit = GetDlgItem(hwnd, IDC_LOCK_COMBO);
                let unlock_edit = GetDlgItem(hwnd, IDC_UNLOCK_COMBO);
                let (lock_mods, lock_vk) = hotkey_recorder::value(lock_edit);
                let (unlock_mods, unlock_vk) = hotkey_recorder::value(unlock_edit);
                // decide_action (hooks.rs) matches a combo whose mods == 0
                // unconditionally (the modifier-check clause is vacuously
                // true), so an unconstrained bare-key combo would fire on
                // every press of that key system-wide. Reject atomically —
                // either both combos are valid and everything commits
                // together, or nothing does. The panic key has no such
                // restriction (single-key by design, checked via a
                // hold-timer in hooks.rs, not decide_action).
                let combos_ok = lock_mods != 0 && unlock_mods != 0;
                // No validation of its own -- a checkbox can't be invalid --
                // so it just rides along in the same all-or-nothing commit as
                // the other fields, gated by their checks alone.
                let lock_on_start = IsDlgButtonChecked(hwnd, IDC_LOCK_ON_START) == BST_CHECKED;

                if combos_ok && panic_ok {
                    let cfg = Config::get();
                    cfg.panic_vk.store(panic_val, Relaxed);
                    cfg.lock_mods.store(lock_mods, Relaxed);
                    cfg.lock_vk.store(lock_vk, Relaxed);
                    cfg.unlock_mods.store(unlock_mods, Relaxed);
                    cfg.unlock_vk.store(unlock_vk, Relaxed);
                    cfg.lock_on_start.store(lock_on_start, Relaxed);
                    cfg.write_back();
                    EndDialog(hwnd, 1);
                } else if !combos_ok {
                    set_error(hwnd, ERR_ZERO_MODIFIER);
                } else {
                    set_error(hwnd, ERR_PANIC_RANGE);
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
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetDlgItem, GetWindowTextW, SendMessageW, SetForegroundWindow, BM_CLICK, WM_NEXTDLGCTL,
    };

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

    fn read_ini_int(key: &str) -> i32 {
        let ini = crate::config::exe_relative("wraith.ini");
        let sec = crate::to_wide("Wraith");
        let key_w = crate::to_wide(key);
        unsafe { GetPrivateProfileIntW(sec.as_ptr(), key_w.as_ptr(), -1, ini.as_ptr()) }
    }

    fn read_panic_key_from_ini() -> i32 {
        read_ini_int("PanicKey")
    }

    fn window_text(hwnd: HWND) -> String {
        let mut buf = [0u16; 128];
        let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) } as usize;
        String::from_utf16_lossy(&buf[..len])
    }

    // Real hardware-level key synthesis, same technique as
    // hotkey_recorder.rs's own tests. Serializes against
    // hotkey_recorder::SERIAL — GetAsyncKeyState/SendInput touch real global
    // keyboard state, and the combo recorder control (subclassed onto the
    // dialog's EDIT controls here) reads it exactly like it does in that
    // module's own tests, so both sets of tests must not run concurrently.
    fn send_key(vk: u16, down: bool) {
        let mut input: INPUT = unsafe { std::mem::zeroed() };
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous = INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if down { 0 } else { KEYEVENTF_KEYUP },
                time: 0,
                dwExtraInfo: 0,
            },
        };
        unsafe {
            SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
        }
    }

    // RAII guard: releases the synthesized key on drop (including during
    // unwinding from a failed assertion) so a stuck modifier can never leak
    // into a later test.
    struct HeldKey(u16);
    impl HeldKey {
        fn press(vk: u16) -> Self {
            send_key(vk, true);
            HeldKey(vk)
        }
    }
    impl Drop for HeldKey {
        fn drop(&mut self) {
            send_key(self.0, false);
        }
    }

    const VK_CONTROL: u16 = 0x11;
    const VK_SHIFT: u16 = 0x10;
    const VK_L: u16 = 0x4C;
    const VK_U: u16 = 0x55;
    const VK_A: u16 = 0x41;

    // Moves real keyboard focus onto one of the dialog's combo EDIT controls
    // via WM_NEXTDLGCTL (sent, not posted, so it's processed on the dialog's
    // own thread before this returns) plus SetForegroundWindow, so
    // subsequently-synthesized key events land on that control the same way
    // hotkey_recorder's own tests rely on real OS focus.
    fn focus_combo(hwnd: HWND, ctrl_id: i32) {
        unsafe {
            SetForegroundWindow(hwnd);
            let ctrl = GetDlgItem(hwnd, ctrl_id);
            assert!(!ctrl.is_null(), "combo control {ctrl_id} not found");
            SendMessageW(hwnd, WM_NEXTDLGCTL, ctrl as usize, 1);
        }
        std::thread::sleep(Duration::from_millis(60));
    }

    // Focuses the given combo field and types `mods` (held in order) + `key`,
    // releasing everything in reverse order before returning -- mirrors
    // hotkey_recorder's own combo-recording test exactly.
    fn type_combo(hwnd: HWND, ctrl_id: i32, mods: &[u16], key: u16) {
        focus_combo(hwnd, ctrl_id);
        let mut held: Vec<HeldKey> = Vec::new();
        for &m in mods {
            held.push(HeldKey::press(m));
            std::thread::sleep(Duration::from_millis(30));
        }
        let k = HeldKey::press(key);
        std::thread::sleep(Duration::from_millis(60));
        drop(k);
        while let Some(h) = held.pop() {
            drop(h);
        }
        std::thread::sleep(Duration::from_millis(60));
    }

    // Known-fixed combo state, deliberately different from the project's real
    // defaults (mods=7) so a test can tell "still seeded baseline" apart from
    // "actually got recommitted". Every new combo test calls this first, so
    // tests self-heal regardless of run order.
    fn set_baseline_config() {
        let cfg = Config::get();
        cfg.lock_mods.store(0x2, Relaxed); // Ctrl only
        cfg.lock_vk.store(VK_L as u32, Relaxed);
        cfg.unlock_mods.store(0x2, Relaxed); // Ctrl only
        cfg.unlock_vk.store(VK_U as u32, Relaxed);
        cfg.panic_vk.store(27, Relaxed);
        cfg.lock_on_start.store(false, Relaxed);
        cfg.write_back();
    }

    fn is_checked(hwnd: HWND, id: i32) -> bool {
        unsafe { IsDlgButtonChecked(hwnd, id) == BST_CHECKED }
    }

    fn read_lock_on_start_from_ini() -> i32 {
        read_ini_int("LockOnStart")
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

    #[test]
    fn dialog_seeds_lock_and_unlock_combos_from_config() {
        let _g = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set_baseline_config();

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        let lock_edit = unsafe { GetDlgItem(hwnd, IDC_LOCK_COMBO) };
        let unlock_edit = unsafe { GetDlgItem(hwnd, IDC_UNLOCK_COMBO) };
        assert_eq!(window_text(lock_edit), "Ctrl+L");
        assert_eq!(window_text(unlock_edit), "Ctrl+U");

        click(hwnd, IDCANCEL);
        t.join().unwrap();
    }

    #[test]
    fn ok_with_valid_combos_updates_all_atomics_and_ini() {
        let _g_cfg = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _g_key = crate::hotkey_recorder::SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set_baseline_config();

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        // Retype both combos to Ctrl+Shift+<key> -- distinct from the seeded
        // baseline (Ctrl-only) so a pass proves the new value was actually
        // captured and committed, not just left over from seeding.
        type_combo(hwnd, IDC_LOCK_COMBO, &[VK_CONTROL, VK_SHIFT], VK_L);
        type_combo(hwnd, IDC_UNLOCK_COMBO, &[VK_CONTROL, VK_SHIFT], VK_U);
        click(hwnd, IDOK);
        t.join().unwrap();

        let cfg = Config::get();
        assert_eq!(cfg.lock_mods.load(Relaxed), 0x2 | 0x4);
        assert_eq!(cfg.lock_vk.load(Relaxed), VK_L as u32);
        assert_eq!(cfg.unlock_mods.load(Relaxed), 0x2 | 0x4);
        assert_eq!(cfg.unlock_vk.load(Relaxed), VK_U as u32);
        assert_eq!(read_ini_int("LockModifiers"), (0x2 | 0x4) as i32);
        assert_eq!(read_ini_int("LockKey"), VK_L as i32);
        assert_eq!(read_ini_int("UnlockModifiers"), (0x2 | 0x4) as i32);
        assert_eq!(read_ini_int("UnlockKey"), VK_U as i32);

        set_baseline_config(); // restore
    }

    #[test]
    fn ok_with_zero_modifier_lock_combo_rejected_nothing_changes() {
        let _g_cfg = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _g_key = crate::hotkey_recorder::SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set_baseline_config();

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        // Bare key, no modifiers held -- lock_mods would end up 0.
        type_combo(hwnd, IDC_LOCK_COMBO, &[], VK_A);
        // Unlock combo edited to a different, individually-valid value so
        // this test also proves rejection is atomic: neither field commits,
        // not just the invalid one.
        type_combo(hwnd, IDC_UNLOCK_COMBO, &[VK_CONTROL, VK_SHIFT], VK_U);
        click(hwnd, IDOK);

        let cfg = Config::get();
        assert_eq!(cfg.lock_mods.load(Relaxed), 0x2);
        assert_eq!(cfg.lock_vk.load(Relaxed), VK_L as u32);
        assert_eq!(cfg.unlock_mods.load(Relaxed), 0x2);
        assert_eq!(cfg.unlock_vk.load(Relaxed), VK_U as u32);
        assert_eq!(read_ini_int("LockModifiers"), 0x2);
        assert_eq!(read_ini_int("UnlockModifiers"), 0x2);

        // Dialog is still up -- close it via Cancel so the thread can join.
        click(hwnd, IDCANCEL);
        t.join().unwrap();
    }

    #[test]
    fn ok_with_zero_modifier_unlock_combo_rejected_nothing_changes() {
        let _g_cfg = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _g_key = crate::hotkey_recorder::SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set_baseline_config();

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        // Lock combo edited to a different, individually-valid value; unlock
        // combo is the bare key with no modifiers -- unlock_mods would end
        // up 0.
        type_combo(hwnd, IDC_LOCK_COMBO, &[VK_CONTROL, VK_SHIFT], VK_L);
        type_combo(hwnd, IDC_UNLOCK_COMBO, &[], VK_A);
        click(hwnd, IDOK);

        let cfg = Config::get();
        assert_eq!(cfg.lock_mods.load(Relaxed), 0x2);
        assert_eq!(cfg.lock_vk.load(Relaxed), VK_L as u32);
        assert_eq!(cfg.unlock_mods.load(Relaxed), 0x2);
        assert_eq!(cfg.unlock_vk.load(Relaxed), VK_U as u32);
        assert_eq!(read_ini_int("LockModifiers"), 0x2);
        assert_eq!(read_ini_int("UnlockModifiers"), 0x2);

        // Dialog is still up -- close it via Cancel so the thread can join.
        click(hwnd, IDCANCEL);
        t.join().unwrap();
    }

    #[test]
    fn cancel_after_editing_all_three_fields_leaves_everything_unchanged() {
        let _g_cfg = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _g_key = crate::hotkey_recorder::SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set_baseline_config();

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        unsafe { SetDlgItemInt(hwnd, IDC_PANIC_VK, 200, 0) };
        type_combo(hwnd, IDC_LOCK_COMBO, &[VK_CONTROL, VK_SHIFT], VK_L);
        type_combo(hwnd, IDC_UNLOCK_COMBO, &[VK_CONTROL, VK_SHIFT], VK_U);
        click(hwnd, IDCANCEL);
        t.join().unwrap();

        let cfg = Config::get();
        assert_eq!(cfg.panic_vk.load(Relaxed), 27);
        assert_eq!(cfg.lock_mods.load(Relaxed), 0x2);
        assert_eq!(cfg.lock_vk.load(Relaxed), VK_L as u32);
        assert_eq!(cfg.unlock_mods.load(Relaxed), 0x2);
        assert_eq!(cfg.unlock_vk.load(Relaxed), VK_U as u32);
        assert_eq!(read_ini_int("PanicKey"), 27);
        assert_eq!(read_ini_int("LockModifiers"), 0x2);
        assert_eq!(read_ini_int("UnlockModifiers"), 0x2);
    }

    #[test]
    fn dialog_seeds_lock_on_start_checkbox_from_config() {
        let _g = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        // Starting state: unchecked.
        set_baseline_config();
        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();
        assert!(!is_checked(hwnd, IDC_LOCK_ON_START));
        click(hwnd, IDCANCEL);
        t.join().unwrap();

        // Starting state: checked.
        Config::get().lock_on_start.store(true, Relaxed);
        Config::get().write_back();
        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();
        assert!(is_checked(hwnd, IDC_LOCK_ON_START));
        click(hwnd, IDCANCEL);
        t.join().unwrap();

        set_baseline_config(); // restore
    }

    #[test]
    fn ok_with_lock_on_start_toggled_updates_atomic_and_ini() {
        let _g = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set_baseline_config(); // lock_on_start = false

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        unsafe { CheckDlgButton(hwnd, IDC_LOCK_ON_START, BST_CHECKED) };
        click(hwnd, IDOK);
        t.join().unwrap();

        assert!(Config::get().lock_on_start.load(Relaxed));
        assert_eq!(read_lock_on_start_from_ini(), 1);

        set_baseline_config(); // restore
    }

    #[test]
    fn ok_with_lock_on_start_toggled_and_invalid_panic_key_rejects_everything() {
        let _g = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set_baseline_config(); // lock_on_start = false

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        // Toggle the checkbox, but also push panic key out of range so the
        // whole commit must be rejected -- proving lock_on_start can't sneak
        // through as a side commit independent of the other fields' validation.
        unsafe { CheckDlgButton(hwnd, IDC_LOCK_ON_START, BST_CHECKED) };
        unsafe { SetDlgItemInt(hwnd, IDC_PANIC_VK, 999, 0) };
        click(hwnd, IDOK);

        assert!(!Config::get().lock_on_start.load(Relaxed));
        assert_eq!(read_lock_on_start_from_ini(), 0);

        // Dialog is still up -- close it via Cancel so the thread can join.
        click(hwnd, IDCANCEL);
        t.join().unwrap();
    }

    #[test]
    fn cancel_after_toggling_lock_on_start_leaves_everything_unchanged() {
        let _g = crate::config::CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set_baseline_config(); // lock_on_start = false

        let t = std::thread::spawn(|| show(std::ptr::null_mut()));
        let hwnd = wait_for_test_hwnd();

        unsafe { CheckDlgButton(hwnd, IDC_LOCK_ON_START, BST_CHECKED) };
        click(hwnd, IDCANCEL);
        t.join().unwrap();

        assert!(!Config::get().lock_on_start.load(Relaxed));
        assert_eq!(read_lock_on_start_from_ini(), 0);
    }
}
