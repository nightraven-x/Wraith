// Wraith — WH_KEYBOARD_LL / WH_MOUSE_LL hooks + global atomics
// Step 4: install/uninstall, keyboard_proc, mouse_proc

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering::Relaxed};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::SystemInformation::GetTickCount,
    UI::{
        Input::KeyboardAndMouse::GetAsyncKeyState,
        WindowsAndMessaging::{
            CallNextHookEx, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
            KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL,
            WM_COMMAND, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
        },
    },
};

pub static LOCKED:   AtomicBool  = AtomicBool::new(false);
pub static APP_HWND: AtomicUsize = AtomicUsize::new(0); // HWND as usize
pub static APP_TRAY: AtomicUsize = AtomicUsize::new(0); // *mut TrayIcon as usize

static KB_HOOK:     AtomicUsize = AtomicUsize::new(0); // HHOOK as usize
static MOUSE_HOOK:  AtomicUsize = AtomicUsize::new(0); // HHOOK as usize
static PANIC_START: AtomicU32   = AtomicU32::new(0);   // GetTickCount() snapshot

/// Advance the panic-key hold timer. Returns true when the panic key has been
/// held for >= 3000ms and unlock should fire. Must be called on every TIMER_PANIC tick.
pub fn panic_key_tick() -> bool {
    let panic_vk = crate::config::Config::get().panic_vk;
    let held = (unsafe { GetAsyncKeyState(panic_vk as i32) } as u16) & 0x8000 != 0;
    if held {
        let now = unsafe { GetTickCount() };
        let start = PANIC_START.load(Relaxed);
        if start == 0 {
            PANIC_START.store(now, Relaxed);
            false
        } else {
            now.wrapping_sub(start) >= 3000
        }
    } else {
        PANIC_START.store(0, Relaxed);
        false
    }
}

/// Reset the panic hold timer. Call from unlock().
pub fn panic_reset() {
    PANIC_START.store(0, Relaxed);
}

pub fn install(hwnd: HWND) -> Result<(), &'static str> {
    APP_HWND.store(hwnd as usize, Relaxed);
    let kb = unsafe {
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), std::ptr::null_mut(), 0)
    };
    if kb.is_null() {
        return Err("Failed to install keyboard hook");
    }
    KB_HOOK.store(kb as usize, Relaxed);

    // Install mouse hook (clean up kb hook on failure)
    let ms = unsafe {
        SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), std::ptr::null_mut(), 0)
    };
    if ms.is_null() {
        unsafe { UnhookWindowsHookEx(kb); }
        KB_HOOK.store(0, Relaxed);
        return Err("Failed to install mouse hook");
    }
    MOUSE_HOOK.store(ms as usize, Relaxed);

    Ok(())
}

/// Reinstall both hooks. Called periodically to recover from silent hook removal
/// (e.g. Parsec virtual driver teardown modifying the hook chain mid-session).
pub fn watchdog() {
    let hwnd = APP_HWND.load(Relaxed) as HWND;
    if hwnd.is_null() { return; }
    uninstall();
    let _ = install(hwnd); // silent fail — next tick will retry
}

pub fn uninstall() {
    let kb = KB_HOOK.swap(0, Relaxed);
    if kb != 0 {
        unsafe { UnhookWindowsHookEx(kb as *mut core::ffi::c_void); }
    }

    let ms = MOUSE_HOOK.swap(0, Relaxed);
    if ms != 0 {
        unsafe { UnhookWindowsHookEx(ms as *mut core::ffi::c_void); }
    }
}

// Returns true for any modifier virtual key code (Shift, Ctrl, Alt, Win, left/right variants).
#[inline(always)]
fn is_modifier_vk(vk: u32) -> bool {
    matches!(vk,
        0x10 | 0x11 | 0x12        // VK_SHIFT, VK_CONTROL, VK_MENU (generic)
        | 0xA0 | 0xA1             // VK_LSHIFT, VK_RSHIFT
        | 0xA2 | 0xA3             // VK_LCONTROL, VK_RCONTROL
        | 0xA4 | 0xA5             // VK_LMENU, VK_RMENU
        | 0x5B | 0x5C             // VK_LWIN, VK_RWIN
    )
}

// Returns true if the modifier key required by `mod_bit` is currently held.
// mod_bit: MOD_ALT=0x1, MOD_CONTROL=0x2, MOD_SHIFT=0x4, MOD_WIN=0x8
#[inline(always)]
fn mod_held(mod_bit: u32) -> bool {
    // GetAsyncKeyState returns i16; bit 15 (MSB) set means key is down.
    // Cast to u16 so we can compare >= 0x8000 without sign issues.
    let held = |vk: i32| -> bool {
        (unsafe { GetAsyncKeyState(vk) } as u16) >= 0x8000
    };

    match mod_bit {
        0x1 => held(0x12),       // MOD_ALT    -> VK_MENU
        0x2 => held(0x11),       // MOD_CONTROL -> VK_CONTROL
        0x4 => held(0x10),       // MOD_SHIFT  -> VK_SHIFT
        0x8 => held(0x5B) || held(0x5C), // MOD_WIN -> VK_LWIN | VK_RWIN
        _   => false,
    }
}

// Returns true if every modifier bit required by `mods` is held.
#[inline(always)]
fn mods_held(mods: u32) -> bool {
    for bit in [0x1u32, 0x2, 0x4, 0x8] {
        if mods & bit != 0 && !mod_held(bit) {
            return false;
        }
    }
    true
}

unsafe extern "system" fn keyboard_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    // MANDATORY: nCode < 0 must short-circuit first — MSDN requirement.
    if n_code < 0 {
        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
    }

    let kb = &*(l_param as *const KBDLLHOOKSTRUCT);

    // LLKHF_INJECTED (bit 4) — synthetic input; always pass through.
    if kb.flags & 0x10 != 0 {
        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
    }

    // Only check combos on key-down events.
    if w_param == WM_KEYDOWN as WPARAM || w_param == WM_SYSKEYDOWN as WPARAM {
        let cfg = crate::config::Config::get();
        let hwnd = APP_HWND.load(Relaxed) as HWND;

        // Lock combo
        if kb.vkCode == cfg.lock_vk && mods_held(cfg.lock_mods) {
            PostMessageW(hwnd, WM_COMMAND, crate::ID_LOCK, 0);
            return 1; // consume — do NOT call CallNextHookEx
        }

        // Unlock combo
        if kb.vkCode == cfg.unlock_vk && mods_held(cfg.unlock_mods) {
            PostMessageW(hwnd, WM_COMMAND, crate::ID_UNLOCK, 0);
            return 1; // consume
        }
    }

    // Block all other physical keystrokes when locked.
    // Exception: modifier key-UP events pass through so the OS doesn't see
    // Ctrl/Shift/Alt as stuck when the lock combo transitions to locked state.
    if LOCKED.load(Relaxed) {
        let is_keyup = w_param == WM_KEYUP as WPARAM || w_param == WM_SYSKEYUP as WPARAM;
        if is_keyup && is_modifier_vk(kb.vkCode) {
            return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
        }
        return 1; // block — do NOT call CallNextHookEx
    }

    CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
}

unsafe extern "system" fn mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    // MANDATORY: nCode < 0 must short-circuit first.
    if n_code < 0 {
        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
    }

    let ms = &*(l_param as *const MSLLHOOKSTRUCT);

    // LLMHF_INJECTED (bit 0) — synthetic input; always pass through.
    if ms.flags & 0x01 != 0 {
        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
    }

    // Block all physical mouse events when locked.
    if LOCKED.load(Relaxed) {
        return 1; // block — do NOT call CallNextHookEx
    }

    CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
}

