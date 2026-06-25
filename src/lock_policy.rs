// Manages DisableTaskMgr policy: applied on lock, removed on unlock and at startup.
// Startup removal cleans up leftover state if Wraith crashed while locked.

use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD,
};

const POLICY_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System";
const DISABLE_TM: &str = "DisableTaskMgr";

/// Block Task Manager for the current session. Called on lock.
pub fn apply() {
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        // Create key if absent (Policies\System may not exist on clean installs).
        if RegCreateKeyExW(
            HKEY_CURRENT_USER, w(POLICY_KEY).as_ptr(),
            0, std::ptr::null_mut(), 0, KEY_SET_VALUE,
            std::ptr::null_mut(), &mut hkey, std::ptr::null_mut(),
        ) != 0 { return; }
        let val: u32 = 1;
        RegSetValueExW(hkey, w(DISABLE_TM).as_ptr(), 0, REG_DWORD,
            (&val as *const u32).cast(), 4);
        RegCloseKey(hkey);
    }
}

/// Restore Task Manager access. Called on unlock and at startup (crash cleanup).
pub fn remove() {
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER, w(POLICY_KEY).as_ptr(),
            0, KEY_SET_VALUE, &mut hkey,
        ) != 0 { return; }
        RegDeleteValueW(hkey, w(DISABLE_TM).as_ptr());
        RegCloseKey(hkey);
    }
}

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
