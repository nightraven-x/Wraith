use windows_sys::Win32::System::{
    LibraryLoader::GetModuleFileNameW,
    Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
    },
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "Wraith";

pub fn enable() {
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, w(RUN_KEY).as_ptr(), 0, KEY_SET_VALUE, &mut hkey) != 0 {
            return;
        }
        let mut raw = [0u16; 510];
        let len = GetModuleFileNameW(std::ptr::null_mut(), raw.as_mut_ptr(), raw.len() as u32) as usize;
        // Quote the path so spaces in the install directory survive the Run key.
        let mut quoted: Vec<u16> = Vec::with_capacity(len + 3);
        quoted.push(b'"' as u16);
        quoted.extend_from_slice(&raw[..len]);
        quoted.push(b'"' as u16);
        quoted.push(0u16);
        RegSetValueExW(hkey, w(VALUE_NAME).as_ptr(), 0, REG_SZ,
            quoted.as_ptr() as *const u8, (quoted.len() * 2) as u32);
        RegCloseKey(hkey);
    }
}

pub fn disable() {
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, w(RUN_KEY).as_ptr(), 0, KEY_SET_VALUE, &mut hkey) != 0 {
            return;
        }
        RegDeleteValueW(hkey, w(VALUE_NAME).as_ptr());
        RegCloseKey(hkey);
    }
}

pub fn is_enabled() -> bool {
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, w(RUN_KEY).as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) != 0 {
            return false;
        }
        let mut kind = 0u32;
        let mut size = 0u32;
        let found = RegQueryValueExW(hkey, w(VALUE_NAME).as_ptr(),
            std::ptr::null_mut(), &mut kind, std::ptr::null_mut(), &mut size) == 0;
        RegCloseKey(hkey);
        found
    }
}

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
