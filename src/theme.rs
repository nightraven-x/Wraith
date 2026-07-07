// Dark-mode + Windows 11 rounded-corner theming for the settings dialog.
// Called once from settings.rs's WM_INITDIALOG -- purely cosmetic, no effect
// on hooks.rs, app.rs's lock()/unlock(), or the main message loop.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
};
use windows_sys::Win32::UI::Controls::SetWindowTheme;

use crate::to_wide;

const PERSONALIZE_KEY: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
const LIGHT_THEME_VALUE: &str = "AppsUseLightTheme";

/// Reads the user's system light/dark preference. Missing key or read
/// failure defaults to light -- matches Windows' own default before a user
/// has ever touched Settings > Personalization > Colors.
pub fn system_prefers_dark() -> bool {
    unsafe {
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            to_wide(PERSONALIZE_KEY).as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut hkey,
        ) != 0
        {
            return false;
        }
        let mut value: u32 = 1;
        let mut size = std::mem::size_of::<u32>() as u32;
        let ok = RegQueryValueExW(
            hkey,
            to_wide(LIGHT_THEME_VALUE).as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut value as *mut u32 as *mut u8,
            &mut size,
        ) == 0;
        RegCloseKey(hkey);
        ok && value == 0
    }
}

/// Applies dark titlebar + Win11 rounded corners to the dialog, and dark
/// flat rendering to each of its themed child controls. DWM attributes
/// that don't exist pre-Windows-10-1809 just fail (ignored HRESULT) -- the
/// dialog still renders fine via ComCtl32 v6, just without these extras.
pub fn apply(hwnd: HWND, dark: bool, controls: &[HWND]) {
    unsafe {
        let dark_flag: i32 = dark as i32;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark_flag as *const i32 as *const core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );

        let corner = DWMWCP_ROUND;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &corner as *const i32 as *const core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );

        let subapp = if dark { to_wide("DarkMode_Explorer") } else { to_wide("") };
        for &ctrl in controls {
            SetWindowTheme(ctrl, subapp.as_ptr(), std::ptr::null());
        }
    }
}
