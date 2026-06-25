# Research 09 — Update Checker

Verified facts for `src/updater.rs`.

---

## WinHTTP Call Sequence

```rust
// 1. Open session
let session = WinHttpOpen(agent, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);

// 2. Connect to host (hostname only, no https:// prefix)
let connect = WinHttpConnect(session, w!("api.github.com"), INTERNET_DEFAULT_HTTPS_PORT, 0);

// 3. Open request
let request = WinHttpOpenRequest(connect, w!("GET"),
    w!("/repos/shadow-dragon-2002/Wraith/releases/latest"),
    null(), WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, WINHTTP_FLAG_SECURE);

// 4. Send
WinHttpSendRequest(request, WINHTTP_NO_ADDITIONAL_HEADERS, 0, WINHTTP_NO_REQUEST_DATA, 0, 0, 0);

// 5. Receive response
WinHttpReceiveResponse(request, null_mut());

// 6. Read loop
let mut body = Vec::<u8>::new();
let mut buf = [0u8; 4096];
let mut bytes_read: u32 = 0;
loop {
    let ok = WinHttpReadData(request, buf.as_mut_ptr() as _, buf.len() as u32, &mut bytes_read);
    if !ok.as_bool() || bytes_read == 0 { break; }  // EOF when bytes_read == 0
    body.extend_from_slice(&buf[..bytes_read as usize]);
}

// 7. Close all handles (order doesn't matter, but reverse creation is conventional)
WinHttpCloseHandle(request);
WinHttpCloseHandle(connect);
WinHttpCloseHandle(session);
```

## Key Constants

- `WINHTTP_FLAG_SECURE = 0x00800000`
- `INTERNET_DEFAULT_HTTPS_PORT = 443`
- `WINHTTP_ACCESS_TYPE_DEFAULT_PROXY` — reads system proxy (IE/WinINet), supports PAC scripts

## WinHttpReadData EOF

- `WinHttpReadData` returning `TRUE` with `bytes_read == 0` → **EOF** — all data received
- `WinHttpReadData` returning `FALSE` → error — bail out
- Loop termination: `!ok || bytes_read == 0`

## GitHub API

- Endpoint: `GET https://api.github.com/repos/shadow-dragon-2002/Wraith/releases/latest`
- Returns HTTP 200 + JSON with `"tag_name"` field (e.g. `"v1.0.0"`)
- HTTP 404 if no releases exist → body won't contain `"tag_name"` → string search returns `None` → silent fail
- No auth token required for public repos
- Rate limit: 60 req/hour per IP — single startup check is within limits

## Version Parsing (no serde/json crate)

```rust
fn parse_tag(body: &str) -> Option<(u32, u32, u32)> {
    let start = body.find("\"tag_name\"")? + "\"tag_name\"".len();
    let after = body[start..].find('"')? + start + 1;
    let end = body[after..].find('"')? + after;
    let tag = body[after..end].trim_start_matches('v');
    let mut parts = tag.splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}
// Compare as (u32, u32, u32) tuple — correct numeric semver comparison
```

## Timeouts

```rust
// Set on hRequest handle before SendRequest
let timeout_ms: u32 = 10_000;
WinHttpSetOption(request, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout_ms as *const _ as _, 4);
WinHttpSetOption(request, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout_ms as *const _ as _, 4);
WinHttpSetOption(request, WINHTTP_OPTION_SEND_TIMEOUT,    &timeout_ms as *const _ as _, 4);
```

## PostMessageW from Background Thread

```rust
// Pack result into Box, pass pointer as LPARAM
let result = Box::new(format!("Update available: {}", latest_version));
PostMessageW(APP_HWND.load(Relaxed) as HWND, WM_UPDATE_RESULT, 0,
    Box::into_raw(result) as LPARAM);

// WndProc handles WM_UPDATE_RESULT:
WM_UPDATE_RESULT => {
    let msg = Box::from_raw(lp as *mut String);
    tray.show_balloon("Wraith Update", &msg);
}
```

- `PostMessageW` is thread-safe — calling thread needs NO message loop
- `HWND` is `*mut c_void` — cast to `usize` for `APP_HWND: AtomicUsize`, cast back in spawned thread

## windows-sys Setup

Feature: `Win32_Networking_WinHttp`

build.rs required for GNU target:
```rust
fn main() { println!("cargo:rustc-link-lib=winhttp"); }
```
`windows-sys` does NOT auto-link `winhttp.lib` for GNU target.

## Corporate Proxy / HTTPS Inspection

- `WINHTTP_ACCESS_TYPE_DEFAULT_PROXY` reads system proxy + PAC scripts — works for standard corp proxies
- HTTPS-intercepting proxies → certificate validation fails → treat as network error (silent fail)
- Acceptable: update checker is non-critical, silent failure is fine
