// Reusable hotkey-recorder control: subclasses an existing EDIT control so it
// displays a human-readable combo (e.g. "Ctrl+Shift+L") and, while focused,
// replaces its content with whatever combo the user next presses.
//
// Behaviour (see GitHub issue #13):
//   - Field shows the current combo as human-readable text.
//   - On focus, the next keypress replaces the content with the detected combo.
//   - Modifier-only presses (e.g. holding Ctrl alone) are shown live but not
//     committed until a non-modifier key is also pressed.
//   - Escape cancels recording and restores the previously committed value.
//
// Storage format matches the project's Config type: `(mods: u32, vk: u32)`
// with MOD_ALT=0x1, MOD_CONTROL=0x2, MOD_SHIFT=0x4, MOD_WIN=0x8.
//
// This module is deliberately dumb/reusable: it does not reject zero-modifier
// combos or otherwise validate the captured value — that policy belongs to
// whatever dialog consumes it.

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowTextW, GWLP_USERDATA,
    GWLP_WNDPROC, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_NCDESTROY, WM_SYSCHAR, WM_SYSKEYDOWN,
    WM_SYSKEYUP, WNDPROC,
};

const VK_ESCAPE: u32 = 0x1B;

// Per-instance state, stored via GWLP_USERDATA on the EDIT control itself
// (free for this purpose — nothing else on a plain EDIT control uses it).
struct State {
    orig_proc: WNDPROC,
    mods: u32,
    vk: u32,
    previewing: bool, // true while a modifier is held but nothing has committed yet
}

/// Subclass `hwnd` (must be an existing EDIT control) into a hotkey recorder,
/// seeding its displayed text from `initial` = (mods, vk).
pub fn install(hwnd: HWND, initial: (u32, u32)) {
    unsafe {
        let state = Box::new(State {
            orig_proc: None,
            mods: initial.0,
            vk: initial.1,
            previewing: false,
        });
        let ptr = Box::into_raw(state);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);

        let prev = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, subclass_proc as *const () as isize);
        (*ptr).orig_proc = std::mem::transmute(prev);

        set_text(hwnd, initial.0, initial.1);
    }
}

/// Read back the currently committed (mods, vk) value.
pub fn value(hwnd: HWND) -> (u32, u32) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
        if ptr.is_null() {
            return (0, 0);
        }
        ((*ptr).mods, (*ptr).vk)
    }
}

// Maps a modifier virtual key code (generic or left/right variant) to its
// mods bit, or 0 if `vk` is not a modifier key. Mirrors hooks.rs's
// modifier_bit — duplicated locally to keep this module self-contained.
fn modifier_bit(vk: u32) -> u32 {
    match vk {
        0x12 | 0xA4 | 0xA5 => 0x1, // VK_MENU, VK_LMENU, VK_RMENU     -> MOD_ALT
        0x11 | 0xA2 | 0xA3 => 0x2, // VK_CONTROL, VK_LCONTROL, VK_RCONTROL -> MOD_CONTROL
        0x10 | 0xA0 | 0xA1 => 0x4, // VK_SHIFT, VK_LSHIFT, VK_RSHIFT  -> MOD_SHIFT
        0x5B | 0x5C => 0x8,        // VK_LWIN, VK_RWIN                -> MOD_WIN
        _ => 0,
    }
}

fn is_modifier_vk(vk: u32) -> bool {
    modifier_bit(vk) != 0
}

// Reads which modifiers are currently physically held via GetAsyncKeyState.
// Safe to call from a subclass proc: unlike Wraith's low-level hooks, this
// control never blocks the event, so Windows keeps hardware state current.
fn held_mods() -> u32 {
    unsafe {
        let mut m = 0u32;
        if GetAsyncKeyState(0x11) as u16 & 0x8000 != 0 { m |= 0x2; } // VK_CONTROL
        if GetAsyncKeyState(0x10) as u16 & 0x8000 != 0 { m |= 0x4; } // VK_SHIFT
        if GetAsyncKeyState(0x12) as u16 & 0x8000 != 0 { m |= 0x1; } // VK_MENU
        if GetAsyncKeyState(0x5B) as u16 & 0x8000 != 0 { m |= 0x8; } // VK_LWIN
        if GetAsyncKeyState(0x5C) as u16 & 0x8000 != 0 { m |= 0x8; } // VK_RWIN
        m
    }
}

fn key_name(vk: u32) -> String {
    match vk {
        0x30..=0x39 => ((vk as u8) as char).to_string(), // '0'-'9'
        0x41..=0x5A => ((vk as u8) as char).to_string(), // 'A'-'Z'
        0x70..=0x87 => format!("F{}", vk - 0x70 + 1),     // F1-F24
        0x08 => "Backspace".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Enter".to_string(),
        0x1B => "Escape".to_string(),
        0x20 => "Space".to_string(),
        0x21 => "PageUp".to_string(),
        0x22 => "PageDown".to_string(),
        0x23 => "End".to_string(),
        0x24 => "Home".to_string(),
        0x25 => "Left".to_string(),
        0x26 => "Up".to_string(),
        0x27 => "Right".to_string(),
        0x28 => "Down".to_string(),
        0x2D => "Insert".to_string(),
        0x2E => "Delete".to_string(),
        _ => format!("VK{:#04X}", vk),
    }
}

// Formats a combo as human-readable text. `vk == 0` means "modifiers only,
// no key yet" (used for the live preview while a modifier is held alone).
fn format_combo(mods: u32, vk: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    if mods & 0x2 != 0 { parts.push("Ctrl".to_string()); }
    if mods & 0x4 != 0 { parts.push("Shift".to_string()); }
    if mods & 0x1 != 0 { parts.push("Alt".to_string()); }
    if mods & 0x8 != 0 { parts.push("Win".to_string()); }
    if vk != 0 { parts.push(key_name(vk)); }
    if parts.is_empty() { "(none)".to_string() } else { parts.join("+") }
}

fn set_text(hwnd: HWND, mods: u32, vk: u32) {
    let wide = crate::to_wide(&format_combo(mods, vk));
    unsafe { SetWindowTextW(hwnd, wide.as_ptr()); }
}

fn set_preview_text(hwnd: HWND, mods: u32) {
    let wide = crate::to_wide(&format_combo(mods, 0));
    unsafe { SetWindowTextW(hwnd, wide.as_ptr()); }
}

unsafe extern "system" fn subclass_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    if ptr.is_null() {
        // Should never happen (install() always sets this before subclassing),
        // but never blindly deref a null pointer in a wndproc.
        return CallWindowProcW(None, hwnd, msg, wp, lp);
    }
    let state = &mut *ptr;

    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let vk = wp as u32;

            if vk == VK_ESCAPE {
                // Cancel: restore previously committed value, discard any preview.
                state.previewing = false;
                set_text(hwnd, state.mods, state.vk);
                return 0;
            }

            let held = held_mods();
            if is_modifier_vk(vk) {
                // Modifier alone so far — show it live, do not commit.
                state.previewing = true;
                set_preview_text(hwnd, held);
            } else {
                // Non-modifier key — commit mods currently held plus this key.
                state.mods = held;
                state.vk = vk;
                state.previewing = false;
                set_text(hwnd, state.mods, state.vk);
            }
            0
        }

        WM_KEYUP | WM_SYSKEYUP => {
            let vk = wp as u32;
            if state.previewing && is_modifier_vk(vk) {
                let held = held_mods();
                if held == 0 {
                    // Last modifier released without a following non-modifier key:
                    // nothing commits, revert display to the committed value.
                    state.previewing = false;
                    set_text(hwnd, state.mods, state.vk);
                } else {
                    set_preview_text(hwnd, held);
                }
            }
            0
        }

        // Swallow character messages — this control shows a captured combo,
        // never literal typed text.
        WM_CHAR | WM_SYSCHAR => 0,

        WM_NCDESTROY => {
            let orig = state.orig_proc;
            let r = CallWindowProcW(orig, hwnd, msg, wp, lp);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr));
            r
        }

        _ => CallWindowProcW(state.orig_proc, hwnd, msg, wp, lp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, Once};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetWindowTextW,
        PeekMessageW, RegisterClassExW, SetForegroundWindow, ShowWindow, TranslateMessage,
        ES_AUTOHSCROLL, MSG, PM_REMOVE, SW_SHOWNOACTIVATE, WNDCLASSEXW, WS_CHILD,
        WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
    };

    // GetAsyncKeyState / SendInput touch REAL global keyboard state, so tests
    // must not run concurrently or they'll corrupt each other's modifier reads.
    static SERIAL: Mutex<()> = Mutex::new(());
    static REGISTER_CLASS: Once = Once::new();

    fn register_class_once() {
        REGISTER_CLASS.call_once(|| unsafe {
            let hinstance = GetModuleHandleW(std::ptr::null());
            let class_name = crate::to_wide("HotkeyRecorderTestWindow");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: 0,
                lpfnWndProc: Some(DefWindowProcW),
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
            RegisterClassExW(&wc);
        });
    }

    // Creates a real, visible top-level window + child EDIT control, gives the
    // EDIT control focus. Returns (top_level, edit).
    fn create_test_edit() -> (HWND, HWND) {
        register_class_once();
        unsafe {
            let hinstance = GetModuleHandleW(std::ptr::null());
            let class_name = crate::to_wide("HotkeyRecorderTestWindow");
            let top = CreateWindowExW(
                0,
                class_name.as_ptr(),
                crate::to_wide("hotkey_recorder test").as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                0, 0, 220, 120,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            );
            assert!(!top.is_null(), "failed to create test top-level window");

            let edit_class = crate::to_wide("EDIT");
            let edit = CreateWindowExW(
                0,
                edit_class.as_ptr(),
                crate::to_wide("").as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                10, 10, 180, 24,
                top,
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            );
            assert!(!edit.is_null(), "failed to create test EDIT control");

            ShowWindow(top, SW_SHOWNOACTIVATE);
            SetForegroundWindow(top);
            SetFocus(edit);
            pump_for(50);
            (top, edit)
        }
    }

    fn destroy_test_window(top: HWND) {
        unsafe { DestroyWindow(top); }
        pump_for(20);
    }

    // Drains and dispatches this thread's message queue for at least `ms`
    // milliseconds, giving SendInput-generated events time to arrive.
    fn pump_for(ms: u64) {
        let deadline = Instant::now() + Duration::from_millis(ms);
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while Instant::now() < deadline {
            unsafe {
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

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
        unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32); }
    }

    fn window_text(hwnd: HWND) -> String {
        let mut buf = [0u16; 128];
        let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) } as usize;
        String::from_utf16_lossy(&buf[..len])
    }

    // RAII guard around a synthesized key-down: releases the key on drop, even
    // during unwinding from a failed assertion. Without this, a panic between
    // "press modifier" and "release modifier" leaves it stuck down for the rest
    // of the process — GetAsyncKeyState is real global state, so a leaked
    // held-down modifier silently corrupts every later test in the same run.
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
    const VK_A: u16 = 0x41;
    const VK_ESC: u16 = 0x1B;

    #[test]
    fn subclass_seeds_initial_value_without_breaking_the_control() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (top, edit) = create_test_edit();

        install(edit, (0x6, VK_L as u32)); // Ctrl+Shift+L

        assert_eq!(value(edit), (0x6, VK_L as u32));
        assert_eq!(window_text(edit), "Ctrl+Shift+L");

        destroy_test_window(top);
    }

    #[test]
    fn records_a_plain_non_modifier_key() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (top, edit) = create_test_edit();
        install(edit, (0, 0));

        let a = HeldKey::press(VK_A);
        pump_for(80);
        drop(a);
        pump_for(80);

        assert_eq!(value(edit), (0, VK_A as u32));
        assert_eq!(window_text(edit), "A");

        destroy_test_window(top);
    }

    #[test]
    fn records_ctrl_shift_l() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (top, edit) = create_test_edit();
        install(edit, (0, 0));

        let ctrl = HeldKey::press(VK_CONTROL);
        pump_for(50);
        let shift = HeldKey::press(VK_SHIFT);
        pump_for(50);
        let l = HeldKey::press(VK_L);
        pump_for(80);
        drop(l);
        drop(shift);
        drop(ctrl);
        pump_for(80);

        assert_eq!(value(edit), (0x2 | 0x4, VK_L as u32));
        assert_eq!(window_text(edit), "Ctrl+Shift+L");

        destroy_test_window(top);
    }

    #[test]
    fn modifier_alone_does_not_commit() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (top, edit) = create_test_edit();
        install(edit, (0x6, VK_L as u32)); // seed Ctrl+Shift+L as the prior value

        let ctrl = HeldKey::press(VK_CONTROL);
        pump_for(80);
        // While held, it's shown live but not committed.
        assert_eq!(value(edit), (0x6, VK_L as u32));
        assert_eq!(window_text(edit), "Ctrl");

        drop(ctrl);
        pump_for(80);

        // Released without a following non-modifier key: nothing commits.
        assert_eq!(value(edit), (0x6, VK_L as u32));
        assert_eq!(window_text(edit), "Ctrl+Shift+L");

        destroy_test_window(top);
    }

    #[test]
    fn escape_restores_previous_value() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (top, edit) = create_test_edit();
        install(edit, (0x6, VK_L as u32)); // seed Ctrl+Shift+L as the prior value

        // Shift, not Ctrl: Ctrl+Escape is the OS-reserved Start Menu hotkey and
        // never reaches our window, so it can't be used to exercise this path.
        let shift = HeldKey::press(VK_SHIFT);
        pump_for(50);
        send_key(VK_ESC, true);
        pump_for(80);
        send_key(VK_ESC, false);

        assert_eq!(value(edit), (0x6, VK_L as u32));
        assert_eq!(window_text(edit), "Ctrl+Shift+L");

        drop(shift);
        pump_for(50);

        destroy_test_window(top);
    }
}
