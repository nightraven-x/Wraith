# Spec 09 — Update Checker

> Research prerequisite: [../research/09-updater.md](../research/09-updater.md)
> Implements: `src/updater.rs` — Step 8

---

## Purpose

On startup, spawn a background OS thread that checks GitHub Releases for a newer
version of Wraith. If a newer version is found, post a message to the main thread
to show a tray balloon. All network activity is off the main thread — the hook pump
is never blocked.

---

## Public Interface

```rust
pub fn spawn(hwnd: HWND);
// Spawns a background std::thread. Returns immediately.
// Posts WM_UPDATE_RESULT to hwnd when complete (or silently does nothing on error).
```

---

## Thread Behavior

**R1.** Spawn via `std::thread::spawn`. The thread captures `hwnd as usize` from
`APP_HWND` (since `HWND` is not `Send` — wrap as `usize`, cast back inside thread).

**R2.** Network errors at any step: close all open handles and return silently.
No error reporting to the user for network failures.

**R3.** If current version >= remote version: return silently. No balloon.

**R4.** If remote version > current: allocate `Box::new(message_string)`,
call `PostMessageW(hwnd, WM_UPDATE_RESULT, 0, Box::into_raw(msg) as LPARAM)`.
The main thread frees the Box in `WM_UPDATE_RESULT` handler.

---

## WinHTTP Sequence

**R5.** `WinHttpOpen(L"Wraith/1.0", WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, NULL, NULL, 0)`
→ `hSession`. On NULL: return.

**R6.** `WinHttpConnect(hSession, L"api.github.com", INTERNET_DEFAULT_HTTPS_PORT, 0)`
→ `hConnect`. On NULL: close session, return.

**R7.** `WinHttpOpenRequest(hConnect, L"GET",
    L"/repos/shadow-dragon-2002/Wraith/releases/latest",
    NULL, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES,
    WINHTTP_FLAG_SECURE)`
→ `hRequest`. On NULL: close connect + session, return.

**R8.** `WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0,
    WINHTTP_NO_REQUEST_DATA, 0, 0, 0)`. On FALSE: close all, return.

**R9.** `WinHttpReceiveResponse(hRequest, NULL)`. On FALSE: close all, return.

**R10.** Read response body in a loop:
```rust
let mut body = Vec::<u8>::new();
let mut buf = [0u8; 4096];
let mut bytes_read: u32 = 0;
loop {
    if WinHttpReadData(hRequest, buf.as_mut_ptr() as _, buf.len() as u32, &mut bytes_read) == 0 {
        // error — close all, return
        break;
    }
    if bytes_read == 0 { break; }  // EOF
    body.extend_from_slice(&buf[..bytes_read as usize]);
}
```

**R11.** Close handles in reverse order: `hRequest` → `hConnect` → `hSession`.
Always close regardless of success/failure.

---

## Version Parsing

**R12.** Convert body bytes to UTF-8 string. On invalid UTF-8: return silently.

**R13.** Find `"tag_name"` in the string. On not found: return silently.

**R14.** Advance past `"tag_name"` and scan for the next `"` character (opening of
the value), then read until the closing `"`. Strip a leading `v` if present.
Tags must follow `vX.Y.Z` format — if the stripped value doesn't parse, return silently.

**R15.** Parse both remote version string and `env!("CARGO_PKG_VERSION")` as
`(u32, u32, u32)` tuples by splitting on `.` and parsing each component.
On parse failure: return silently.

**R16.** Compare tuples numerically: `(major, minor, patch)`.
Only proceed if `remote > current` (lexicographic tuple comparison on integers).

---

## Update Message Format

**R17.** If update available, the heap `String` posted via `WM_UPDATE_RESULT` should
be formatted as:
```
"Version vX.Y.Z is available. Visit github.com/shadow-dragon-2002/Wraith/releases"
```

---

## Timeouts

**R18.** Set connection and receive timeouts via `WinHttpSetOption`:
- `WINHTTP_OPTION_CONNECT_TIMEOUT`: 10000ms
- `WINHTTP_OPTION_RECEIVE_TIMEOUT`: 10000ms
Set these on `hRequest` before `WinHttpSendRequest`. On failure: continue (defaults apply).

---

## Dependencies

- `hooks.rs` — `APP_HWND` for PostMessageW target
- `app.rs` — `WM_UPDATE_RESULT` constant, TrayIcon for balloon display
- `main.rs` — `spawn()` called as step 8 of init

---

## Edge Cases

- **GitHub API rate limiting:** The `/releases/latest` endpoint does not require auth
  for public repos and has a 60 req/hour anonymous limit. Wraith checks once at startup.
  Rate limit response (403) → treated as network error → silent return.
- **Prerelease tags:** `releases/latest` returns the latest non-prerelease by default.
  Correct behavior — Wraith won't nag users to install prerelease builds.
- **No internet connection:** `WinHttpSendRequest` or `WinHttpReceiveResponse` fails.
  Silent return. Expected behavior.
- **Proxy environments:** `WINHTTP_ACCESS_TYPE_DEFAULT_PROXY` auto-detects system proxy
  settings via WinINet. Verify in research.
- **HWND lifetime:** The main thread's window outlives the background thread
  (the thread is short-lived). If somehow `hwnd` is invalid when `PostMessageW` fires,
  the call fails silently. Non-fatal.
