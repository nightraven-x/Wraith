// Wraith — background update checker
// Step 8: WinHTTP GET to GitHub releases API, version compare, PostMessageW WM_UPDATE_RESULT

use std::sync::atomic::Ordering::Relaxed;

use windows_sys::Win32::{
    Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
        WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetOption,
    },
    UI::WindowsAndMessaging::PostMessageW,
};

use crate::hooks::APP_HWND;

const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: u32 = 0;
const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
const WINHTTP_OPTION_CONNECT_TIMEOUT: u32 = 4;
const WINHTTP_OPTION_RECEIVE_TIMEOUT: u32 = 6;

/// Parse the `tag_name` value from a GitHub releases JSON response.
/// Returns the raw tag string (e.g. `"v1.2.3"`) including the leading `v`.
fn parse_tag(body: &str) -> Option<&str> {
    let after = body.split_once("\"tag_name\"")?.1;
    let after_colon = after.split_once(':')?.1;
    let trimmed = after_colon.trim_start();
    if !trimmed.starts_with('"') {
        return None;
    }
    let inner = &trimmed[1..];
    inner.split('"').next()
}

/// Parse a version string (`"1.2.3"` or `"v1.2.3"`) into a comparable tuple.
fn parse_ver(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let mut parts = s.splitn(3, '.').map(|p| p.parse::<u32>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

/// Owns the session/connect/request WinHTTP handles for one fetch.
/// Closes whichever handles were acquired, in reverse order, on drop —
/// including on an early return mid-`open()`, so every failure path in
/// `fetch_latest` is cleanup-free.
struct HttpHandles {
    session: *mut core::ffi::c_void,
    connect: *mut core::ffi::c_void,
    request: *mut core::ffi::c_void,
}

impl HttpHandles {
    unsafe fn open(agent: &[u16], host: &[u16], path: &[u16], method: &[u16]) -> Option<Self> {
        let mut h = HttpHandles {
            session: std::ptr::null_mut(),
            connect: std::ptr::null_mut(),
            request: std::ptr::null_mut(),
        };

        h.session = WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if h.session.is_null() {
            return None;
        }

        h.connect = WinHttpConnect(h.session, host.as_ptr(), 443, 0);
        if h.connect.is_null() {
            return None;
        }

        h.request = WinHttpOpenRequest(
            h.connect,
            method.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),               // lpszVersion: *const u16 (NULL = HTTP/1.1)
            std::ptr::null(),               // lpszReferrer: *const u16
            std::ptr::null::<*const u16>(), // lplszAcceptTypes: *const *const u16
            WINHTTP_FLAG_SECURE,
        );
        if h.request.is_null() {
            return None;
        }

        Some(h)
    }
}

impl Drop for HttpHandles {
    fn drop(&mut self) {
        unsafe {
            if !self.request.is_null() {
                WinHttpCloseHandle(self.request);
            }
            if !self.connect.is_null() {
                WinHttpCloseHandle(self.connect);
            }
            if !self.session.is_null() {
                WinHttpCloseHandle(self.session);
            }
        }
    }
}

/// Fetch the latest GitHub release body via WinHTTP.
/// Returns `None` on any network or API error.
unsafe fn fetch_latest() -> Option<Vec<u8>> {
    let agent = crate::to_wide("Wraith-Updater/1.0");
    let host = crate::to_wide("api.github.com");
    let path = crate::to_wide("/repos/shadow-dragon-2002/Wraith/releases/latest");
    let method = crate::to_wide("GET");

    let h = HttpHandles::open(&agent, &host, &path, &method)?;

    let timeout = 10000u32;
    WinHttpSetOption(h.request, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout as *const u32 as *const core::ffi::c_void, 4);
    WinHttpSetOption(h.request, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout as *const u32 as *const core::ffi::c_void, 4);

    let sent = WinHttpSendRequest(
        h.request,
        std::ptr::null(),     // lpszHeaders: *const u16 (WINHTTP_NO_ADDITIONAL_HEADERS)
        0,                    // dwHeadersLength
        std::ptr::null_mut(), // lpOptional: *mut c_void (no request body)
        0,                    // dwOptionalLength
        0,                    // dwTotalLength
        0,                    // dwContext
    );
    if sent == 0 {
        return None;
    }

    let received = WinHttpReceiveResponse(h.request, std::ptr::null_mut());
    if received == 0 {
        return None;
    }

    let mut body: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let mut bytes_read: u32 = 0;
        let ok = WinHttpReadData(
            h.request,
            buf.as_mut_ptr() as *mut core::ffi::c_void,
            buf.len() as u32,
            &mut bytes_read,
        );
        if ok == 0 || bytes_read == 0 {
            break;
        }
        body.extend_from_slice(&buf[..bytes_read as usize]);
    }

    Some(body)
}

/// Spawn a background thread that checks for a newer GitHub release.
/// Returns immediately. Posts `WM_UPDATE_RESULT` with a heap `Box<String>` as LPARAM
/// if a newer version is found; silent on error or when up to date.
pub fn spawn() {
    std::thread::spawn(|| {
        let body = unsafe { fetch_latest() };
        let body = match body {
            Some(b) => b,
            None => return,
        };

        let body_str = String::from_utf8_lossy(&body);
        let tag = match parse_tag(&body_str) {
            Some(t) => t.to_owned(),
            None => return,
        };

        let latest_ver = match parse_ver(&tag) {
            Some(v) => v,
            None => return,
        };

        let current_ver = match parse_ver(env!("CARGO_PKG_VERSION")) {
            Some(v) => v,
            None => return,
        };

        if latest_ver <= current_ver {
            return;
        }

        let msg = Box::new(format!(
            "Version {} is available. Visit github.com/shadow-dragon-2002/Wraith/releases",
            tag
        ));
        let raw = Box::into_raw(msg) as isize;
        let hwnd = APP_HWND.load(Relaxed) as *mut core::ffi::c_void;
        unsafe {
            PostMessageW(hwnd, crate::WM_UPDATE_RESULT, 0, raw);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{parse_tag, parse_ver};

    #[test]
    fn parse_tag_extracts_version() {
        let json = r#"{"tag_name": "v1.2.3", "name": "Release 1.2.3"}"#;
        assert_eq!(parse_tag(json), Some("v1.2.3"));
    }

    #[test]
    fn parse_tag_returns_none_on_missing() {
        assert_eq!(parse_tag(r#"{"name": "no tag here"}"#), None);
    }

    #[test]
    fn parse_ver_strips_v_prefix() {
        assert_eq!(parse_ver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_ver("1.2.3"), Some((1, 2, 3)));
    }

    #[test]
    fn parse_ver_numeric_comparison_correct() {
        // "1.10.0" > "1.9.0" must hold — string compare would fail this
        let a = parse_ver("1.10.0").unwrap();
        let b = parse_ver("1.9.0").unwrap();
        assert!(a > b);
    }

    #[test]
    fn parse_ver_returns_none_on_invalid() {
        assert_eq!(parse_ver("not-a-version"), None);
        assert_eq!(parse_ver("1.2"), None);
    }

    #[test]
    fn parse_tag_handles_whitespace_and_compact_json() {
        // Compact JSON (no spaces after colon)
        let compact = r#"{"tag_name":"v2.0.1","prerelease":false}"#;
        assert_eq!(parse_tag(compact), Some("v2.0.1"));

        // Spaces around colon
        let spaced = r#"{ "tag_name" : "v3.1.0" }"#;
        assert_eq!(parse_tag(spaced), Some("v3.1.0"));
    }
}
