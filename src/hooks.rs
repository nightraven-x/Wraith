
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering::Relaxed};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::SystemInformation::GetTickCount,
    UI::WindowsAndMessaging::{
        CallNextHookEx, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
        KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL,
        WM_COMMAND, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    },
};

pub static LOCKED:   AtomicBool  = AtomicBool::new(false);
pub static APP_HWND: AtomicUsize = AtomicUsize::new(0); // HWND as usize
pub static APP_TRAY: AtomicUsize = AtomicUsize::new(0); // *mut TrayIcon as usize

static KB_HOOK:     AtomicUsize = AtomicUsize::new(0); // HHOOK as usize
static MOUSE_HOOK:  AtomicUsize = AtomicUsize::new(0); // HHOOK as usize
static PANIC_START: AtomicU32   = AtomicU32::new(0);   // GetTickCount() snapshot

// Modifier hold state, tracked from the raw keydown/keyup events the hook already
// sees. GetAsyncKeyState is NOT reliable here: once LOCKED, this same hook returns 1
// (never calling CallNextHookEx) for modifier keydowns, and that stops Windows from
// updating the state GetAsyncKeyState reads — so combo checks always saw "not held".
// Bits: MOD_ALT=0x1, MOD_CONTROL=0x2, MOD_SHIFT=0x4, MOD_WIN=0x8.
static MOD_STATE:  AtomicU32  = AtomicU32::new(0);
static PANIC_HELD: AtomicBool = AtomicBool::new(false); // same reasoning, for the panic key

/// Advance the panic-key hold timer. Returns true when the panic key has been
/// held for >= 3000ms and unlock should fire. Must be called on every TIMER_PANIC tick.
pub fn panic_key_tick() -> bool {
    let held = PANIC_HELD.load(Relaxed);
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

// Maps a modifier virtual key code (generic or left/right variant) to its
// MOD_STATE bit, or 0 if `vk` is not a modifier key.
#[inline(always)]
fn modifier_bit(vk: u32) -> u32 {
    match vk {
        0x12 | 0xA4 | 0xA5 => 0x1, // VK_MENU, VK_LMENU, VK_RMENU     -> MOD_ALT
        0x11 | 0xA2 | 0xA3 => 0x2, // VK_CONTROL, VK_LCONTROL, VK_RCONTROL -> MOD_CONTROL
        0x10 | 0xA0 | 0xA1 => 0x4, // VK_SHIFT, VK_LSHIFT, VK_RSHIFT  -> MOD_SHIFT
        0x5B | 0x5C        => 0x8, // VK_LWIN, VK_RWIN                -> MOD_WIN
        _ => 0,
    }
}

#[inline(always)]
fn is_modifier_vk(vk: u32) -> bool {
    modifier_bit(vk) != 0
}

// Decides which command (if any) a key-down event fires, given the modifier
// bits already held. Pure — takes held_mods as a parameter rather than
// reading MOD_STATE itself, so it's testable without touching global state.
fn decide_action(vk: u32, held_mods: u32, cfg: &crate::config::Config) -> Option<usize> {
    let lock_mods = cfg.lock_mods.load(Relaxed);
    let unlock_mods = cfg.unlock_mods.load(Relaxed);
    if vk == cfg.lock_vk.load(Relaxed) && held_mods & lock_mods == lock_mods {
        Some(crate::ID_LOCK)
    } else if vk == cfg.unlock_vk.load(Relaxed) && held_mods & unlock_mods == unlock_mods {
        Some(crate::ID_UNLOCK)
    } else {
        None
    }
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

    let is_down = w_param == WM_KEYDOWN as WPARAM || w_param == WM_SYSKEYDOWN as WPARAM;
    let is_up = w_param == WM_KEYUP as WPARAM || w_param == WM_SYSKEYUP as WPARAM;

    // Track modifier and panic-key hold state ourselves from the raw event,
    // independent of whether this event ends up blocked below.
    if is_down || is_up {
        let bit = modifier_bit(kb.vkCode);
        if bit != 0 {
            if is_down { MOD_STATE.fetch_or(bit, Relaxed); } else { MOD_STATE.fetch_and(!bit, Relaxed); }
        }
        if kb.vkCode == crate::config::Config::get().panic_vk.load(Relaxed) {
            PANIC_HELD.store(is_down, Relaxed);
        }
    }

    // Only check combos on key-down events.
    if is_down {
        let cfg = crate::config::Config::get();
        if let Some(id) = decide_action(kb.vkCode, MOD_STATE.load(Relaxed), cfg) {
            let hwnd = APP_HWND.load(Relaxed) as HWND;
            PostMessageW(hwnd, WM_COMMAND, id, 0);
            return 1; // consume — do NOT call CallNextHookEx
        }
    }

    // Block all other physical keystrokes when locked.
    // Exception: modifier key-UP events pass through so the OS doesn't see
    // Ctrl/Shift/Alt as stuck when the lock combo transitions to locked state.
    if LOCKED.load(Relaxed) {
        if is_up && is_modifier_vk(kb.vkCode) {
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

#[cfg(test)]
mod tests {
    use super::decide_action;
    use crate::config::Config;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

    fn cfg() -> Config {
        Config {
            lock_mods: AtomicU32::new(7),   // MOD_ALT|MOD_CONTROL|MOD_SHIFT
            lock_vk: AtomicU32::new(76),    // 'L'
            unlock_mods: AtomicU32::new(7),
            unlock_vk: AtomicU32::new(85),  // 'U'
            panic_vk: AtomicU32::new(27),
            lock_on_start: AtomicBool::new(false),
        }
    }

    #[test]
    fn lock_combo_fires() {
        assert_eq!(decide_action(76, 7, &cfg()), Some(crate::ID_LOCK));
    }

    #[test]
    fn unlock_combo_fires() {
        assert_eq!(decide_action(85, 7, &cfg()), Some(crate::ID_UNLOCK));
    }

    #[test]
    fn wrong_vk_does_not_fire() {
        assert_eq!(decide_action(65, 7, &cfg()), None);
    }

    #[test]
    fn partial_mods_does_not_fire() {
        // Only Alt+Control held, Shift missing.
        assert_eq!(decide_action(76, 3, &cfg()), None);
    }

    #[test]
    fn extra_held_mods_still_fire() {
        // Win held in addition to the required combo — still matches.
        assert_eq!(decide_action(76, 0xF, &cfg()), Some(crate::ID_LOCK));
    }

    #[test]
    fn lock_checked_before_unlock_when_vks_collide() {
        let c = cfg();
        let lock_vk = c.lock_vk.load(Relaxed);
        c.unlock_vk.store(lock_vk, Relaxed); // pathological config: same key for both
        assert_eq!(decide_action(lock_vk, 7, &c), Some(crate::ID_LOCK));
    }
}

