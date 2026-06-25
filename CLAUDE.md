# Wraith — Claude Code Project Brief

> Read this entire file before writing any code.
> Matt Pocock skill content lives in `docs/` — see `docs/SKILLS.md` for workflow.

---

## What Wraith Is

Wraith is a Windows system-tray utility that **blocks physical keyboard and mouse input** while **passing synthetic/injected input from AI tools and automation** through unaffected.

**The use case:** Run Claude in Chrome or a computer-use agent, step away from the desk. Lock Wraith — physical keyboard/mouse go dead to anyone at the desk, but the AI keeps working.

**The key insight:**
- `LLKHF_INJECTED` (bit 4 of `KBDLLHOOKSTRUCT.flags`) — keyboard event from software
- `LLMHF_INJECTED` (bit 0 of `MSLLHOOKSTRUCT.flags`) — mouse event from software

These flags are set by `SendInput()`, `keybd_event()`, Chrome extension injection, AHK `Send`, Parsec/RDP, etc. Wraith installs `WH_KEYBOARD_LL` + `WH_MOUSE_LL`, checks the flag on every event, and only blocks events where the flag is NOT set.

**Hard limits:**
- `Ctrl+Alt+Del` (SAS) is kernel-hardwired — cannot be blocked in user mode
- RDP/Parsec/VNC input is injected → passes through (by design — you can check in remotely while locked)

---

## Why Rust

- **Not Go:** GC pauses can exceed the ~200ms `WH_KEYBOARD_LL` callback timeout, silently uninstalling the hook. Verified broken in practice.
- **Not C++:** Prototype exists and works, but Rust gives memory safety at the hook callback level — a crash here can freeze the entire OS input pipeline.
- **Rust:** No GC, no runtime, `extern "system" fn` pointers, cross-compiles cleanly from WSL via `x86_64-pc-windows-gnu`.

Use `windows-sys` (NOT `windows`) — better GNU target compatibility, no proc-macro complications.

---

## Architecture

### Module Layout

```
src/
├── main.rs      Entry point: mutex check, init sequence, GetMessageW loop
├── app.rs       lock()/unlock(), WndProc, coordinates all modules
├── hooks.rs     install/uninstall, KeyboardProc, MouseProc, global atomics
├── tray.rs      Shell_NotifyIcon lifecycle, menu, balloon notifications
├── config.rs    Config struct, INI load/save, OnceLock
└── updater.rs   Background OS thread, WinHTTP, version parse, PostMessage
```

### Module Public Interfaces

**`config.rs`**
```rust
pub struct Config {
    pub lock_mods: u32, pub lock_vk: u32,
    pub unlock_mods: u32, pub unlock_vk: u32,
    pub panic_vk: u32, pub lock_on_start: bool,
}
impl Config {
    pub fn load() -> Self;          // reads wraith.ini, falls back to defaults
    pub fn get() -> &'static Self;  // OnceLock accessor
}
```

**`hooks.rs`**
```rust
// Global atomics — hook callbacks cannot capture, all state must be global
pub static LOCKED:      AtomicBool  = AtomicBool::new(false);
pub static KB_HOOK:     AtomicUsize = AtomicUsize::new(0);  // HHOOK as usize
pub static MOUSE_HOOK:  AtomicUsize = AtomicUsize::new(0);
pub static APP_HWND:    AtomicUsize = AtomicUsize::new(0);  // HWND as usize
pub static PANIC_START: AtomicU32   = AtomicU32::new(0);    // GetTickCount() snapshot

pub fn install(hwnd: HWND) -> Result<(), &'static str>;
pub fn uninstall();
// keyboard_proc / mouse_proc are private extern "system" fn — registered as callbacks
```

**`tray.rs`**
```rust
pub struct TrayIcon { /* opaque */ }
impl TrayIcon {
    pub fn new(hwnd: HWND) -> Self;
    pub fn set_locked(&mut self, locked: bool);
    pub fn show_balloon(&self, title: &str, msg: &str);
    pub fn show_menu(&self, hwnd: HWND);
    pub fn destroy(&mut self);
}
```

**`app.rs`**
```rust
pub fn lock();
pub fn unlock();
pub fn toggle();
pub fn set_autostart(enable: bool);
pub fn is_autostart() -> bool;
pub unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT;
```

**`updater.rs`**
```rust
pub fn spawn(hwnd: HWND); // OS thread; posts WM_UPDATE_RESULT when done
```

**`main.rs` init sequence**
```rust
// 1. CreateMutexW("Global\\WraithSingleInstance") — exit if already exists
// 2. Config::load() stored in OnceLock
// 3. RegisterClassExW + CreateWindowExW(HWND_MESSAGE) → hwnd
// 4. APP_HWND.store(hwnd as usize, Relaxed)
// 5. TrayIcon::new(hwnd)
// 6. hooks::install(hwnd)
// 7. if Config::get().lock_on_start { app::lock() }
// 8. updater::spawn(hwnd)
// 9. GetMessageW loop (drives hook pump + processes app messages)
```

### Custom WM_ Constants
```rust
pub const WM_TRAY_MSG:      u32   = WM_USER + 1;
pub const WM_UPDATE_RESULT: u32   = WM_USER + 2;
pub const ID_LOCK:          usize = 1001;
pub const ID_UNLOCK:        usize = 1002;
pub const ID_AUTOSTART:     usize = 1003;
pub const ID_EXIT:          usize = 1004;
pub const TIMER_PANIC:      usize = 2001;
```

### Data Flow

```
Physical keypress
    │
    ▼
keyboard_proc (hooks.rs)
    │
    ├─ LLKHF_INJECTED set? ─YES─► CallNextHookEx (pass through)
    │
    ├─ == lock combo? ──────YES─► PostMessageW(WM_COMMAND, ID_LOCK) + consume
    │
    ├─ == unlock combo? ────YES─► PostMessageW(WM_COMMAND, ID_UNLOCK) + consume
    │
    └─ LOCKED == true? ─────YES─► return 1 (block — do NOT call CallNextHookEx)
                           NO──► CallNextHookEx (pass through)

GetMessageW loop → DispatchMessageW → wnd_proc (app.rs)
    ├─ WM_COMMAND / ID_LOCK        → app::lock()
    ├─ WM_COMMAND / ID_UNLOCK      → app::unlock()
    ├─ WM_TRAY_MSG + RMB           → tray.show_menu()
    ├─ WM_TRAY_MSG + double-click  → app::toggle()
    ├─ WM_TIMER / TIMER_PANIC      → GetAsyncKeyState(panic_vk); if ≥3000ms → unlock()
    ├─ WM_UPDATE_RESULT            → tray.show_balloon(); free heap Box
    └─ WM_DESTROY                  → hooks::uninstall(), tray.destroy(), PostQuitMessage(0)
```

---

## Win32 API Reference

### Hooks
```
SetWindowsHookExW(idHook, lpfn, hmod=NULL, dwThreadId=0) -> HHOOK
    WH_KEYBOARD_LL = 13, WH_MOUSE_LL = 14
    lpfn: unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT

CallNextHookEx(hhk, nCode, wParam, lParam) -> LRESULT
UnhookWindowsHookEx(hhk) -> BOOL

KBDLLHOOKSTRUCT { vkCode: u32, scanCode: u32, flags: u32, time: u32, dwExtraInfo: usize }
    flags & 0x10 = LLKHF_INJECTED

MSLLHOOKSTRUCT { pt: POINT, mouseData: u32, flags: u32, time: u32, dwExtraInfo: usize }
    flags & 0x01 = LLMHF_INJECTED

GetAsyncKeyState(vKey: i32) -> i16   // bit 15 set = key held; works even when hook blocks the event
```

### Key Codes
```
VK_CONTROL=0x11, VK_SHIFT=0x10, VK_MENU=0x12(Alt), VK_ESCAPE=0x1B
VK_LWIN=0x5B, VK_RWIN=0x5C
WM_KEYDOWN=0x0100, WM_SYSKEYDOWN=0x0104
MOD_ALT=0x1, MOD_CONTROL=0x2, MOD_SHIFT=0x4, MOD_WIN=0x8
```

### System Tray
```
Shell_NotifyIconW(dwMessage, lpdata) -> BOOL
    NIM_ADD=0, NIM_MODIFY=1, NIM_DELETE=2, NIM_SETVERSION=4
NOTIFYICONDATAW { cbSize, hWnd, uID, uFlags, uCallbackMessage, hIcon,
    szTip:[u16;128], szInfo:[u16;256], szInfoTitle:[u16;64], dwInfoFlags }
uFlags: NIF_MESSAGE=1, NIF_ICON=2, NIF_TIP=4, NIF_INFO=0x10
dwInfoFlags: NIIF_INFO=1, NIIF_NOSOUND=0x10
```

### Message Window + Pump
```
CreateWindowExW(..., HWND_MESSAGE, ...) -> HWND   // invisible, no UI, drives hook pump
GetMessageW / TranslateMessage / DispatchMessageW
PostMessageW(hWnd, Msg, wParam, lParam) -> BOOL   // async, safe from any thread
// NEVER use SendMessageW from hook callbacks or background threads — deadlock risk
```

### Sleep Prevention
```
SetThreadExecutionState(ES_CONTINUOUS|ES_SYSTEM_REQUIRED|ES_DISPLAY_REQUIRED)  // lock
SetThreadExecutionState(ES_CONTINUOUS)                                          // unlock
```

### Config (INI)
```
GetPrivateProfileIntW(lpAppName, lpKeyName, nDefault, lpFileName) -> i32
WritePrivateProfileStringW(lpAppName, lpKeyName, lpString, lpFileName) -> BOOL
INI path: resolve relative to GetModuleFileNameW()
```

### Registry (Auto-start)
```
Key: HKCU\Software\Microsoft\Windows\CurrentVersion\Run
Value: "Wraith" = REG_SZ = full path to wraith.exe
APIs: RegOpenKeyExW / RegSetValueExW / RegDeleteValueW / RegCloseKey
```

### WinHTTP (Update Checker)
```
WinHttpOpen → WinHttpConnect("api.github.com", 443)
→ WinHttpOpenRequest(GET, "/repos/shadow-dragon-2002/Wraith/releases/latest", WINHTTP_FLAG_SECURE=0x00800000)
→ WinHttpSendRequest → WinHttpReceiveResponse → WinHttpReadData loop → WinHttpCloseHandle
Parse: str::find("tag_name") → extract value → strip 'v' → compare to env!("CARGO_PKG_VERSION")
No JSON crate needed.
```

---

## Repo Setup (WSL)

### One-Time Toolchain
```bash
rustup target add x86_64-pc-windows-gnu
sudo apt update && sudo apt install -y gcc-mingw-w64-x86-64
x86_64-w64-mingw32-gcc --version   # verify
```

### `.cargo/config.toml`
```toml
[build]
target = "x86_64-pc-windows-gnu"

[target.x86_64-pc-windows-gnu]
linker   = "x86_64-w64-mingw32-gcc"
ar       = "x86_64-w64-mingw32-ar"
rustflags = ["-C", "link-arg=-Wl,--subsystem,windows"]
```

### `Cargo.toml`
```toml
[package]
name        = "wraith"
version     = "1.0.0"
edition     = "2021"
authors     = ["shadow-dragon-2002"]
description = "Physical input blocker — passes synthetic AI input, blocks hardware"
repository  = "https://github.com/shadow-dragon-2002/Wraith"
license     = "MIT"

[[bin]]
name = "wraith"
path = "src/main.rs"

[dependencies]
windows-sys = { version = "0.59", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Shell",
    "Win32_System_LibraryLoader",
    "Win32_System_Power",
    "Win32_System_Threading",
    "Win32_System_Registry",
    "Win32_Networking_WinHttp",
    "Win32_Security",
] }

[profile.release]
opt-level     = "z"
lto           = true
codegen-units = 1
panic         = "abort"   # REQUIRED — panics in extern "system" with unwind = UB
strip         = true
```

### Build
```bash
cargo build --release --target x86_64-pc-windows-gnu
# Output: target/x86_64-pc-windows-gnu/release/wraith.exe
```

Optional `build.rs` if WinHTTP linker doesn't auto-resolve:
```rust
fn main() { println!("cargo:rustc-link-lib=winhttp"); }
```

---

## Implementation Plan

Build in this order — each step independently testable before moving on:

**Step 1 — Skeleton**
`main.rs`: `CreateMutexW("Global\\WraithSingleInstance")` + exit if `ERROR_ALREADY_EXISTS`, register `WNDCLASSEXW`, create `HWND_MESSAGE` window, run `GetMessageW` loop.
✓ Process starts, stays alive, exits cleanly on `WM_DESTROY`.

**Step 2 — Config**
`config.rs`: `Config` struct, `load()` via `GetPrivateProfileIntW`, path from `GetModuleFileNameW`. `OnceLock<Config>` accessor.
✓ Missing INI → defaults. Custom values load correctly.

**Step 3 — Tray**
`tray.rs`: `Shell_NotifyIconW` add/modify/delete, `WM_TRAY_MSG` routing, `CreatePopupMenu` + `TrackPopupMenu`, balloon helper.
✓ Icon visible, right-click menu works, double-click fires.

**Step 4 — Hooks (core)**
`hooks.rs`: `install()` / `uninstall()`, `keyboard_proc` and `mouse_proc` as `unsafe extern "system" fn`.
- Check `LLKHF_INJECTED` / `LLMHF_INJECTED` FIRST — pass through if set
- Check combos via `GetAsyncKeyState` modifiers + `vkCode`
- `PostMessageW` for state changes — never call `lock()`/`unlock()` directly from hook
- Block: `return 1` without calling `CallNextHookEx`
✓ Physical keyboard blocked. `SendInput` from PowerShell passes through. Combos work.

**Step 5 — Lock/Unlock**
`app.rs`: `lock()` → `LOCKED.store(true)`, `SetThreadExecutionState(ES_CONTINUOUS|ES_SYSTEM_REQUIRED|ES_DISPLAY_REQUIRED)`, update tray.
`unlock()` → `LOCKED.store(false)`, `SetThreadExecutionState(ES_CONTINUOUS)`, update tray.
✓ Full cycle works. Sleep/display suppressed while locked.

**Step 6 — Panic Unlock**
`WM_TIMER / TIMER_PANIC` at 100ms (set on lock, kill on unlock):
`GetAsyncKeyState(config.panic_vk) & 0x8000 != 0` → if `PANIC_START == 0` set it to `GetTickCount()`; if held ≥ 3000ms → `unlock()`. Release → reset `PANIC_START` to 0.
✓ Hold Esc 3s → unlocks. Short hold stays locked.

**Step 7 — Auto-start**
`app.rs`: `set_autostart(enable)` reads/writes `HKCU\...\Run`. Tray menu toggle.
✓ Enable → reboot → Wraith launches. State persists.

**Step 8 — Update Checker**
`updater.rs`: `std::thread::spawn`, WinHTTP GET `api.github.com/repos/shadow-dragon-2002/Wraith/releases/latest`, parse `tag_name`, `Box::new(result)` → `PostMessageW(APP_HWND, WM_UPDATE_RESULT, 0, Box::into_raw(...) as LPARAM)`. WndProc frees the Box and shows balloon.
✓ Downgraded version → balloon. Network error → silent fail.

**Step 9 — Polish**
Resource embedding: `x86_64-w64-mingw32-windres src/resource.rc -o target/resource.o` + link via rustflags. UAC manifest + version info + icons. NSIS installer.

---

## Key Constraints

**Hook callback timeout:** ~200ms. If callback doesn't return, Windows **silently removes the hook** — blocking stops with no error. Rules: no blocking, no I/O, no mutex waits, no function calls that can block. Max: a few comparisons + one `PostMessageW` or `CallNextHookEx`.

**Message pump is mandatory:** `WH_KEYBOARD_LL` / `WH_MOUSE_LL` with `dwThreadId=0` are driven by the installing thread's `GetMessageW` loop. If main thread blocks, hooks stop firing. Nothing else may block the main thread.

**`PostMessageW` not `SendMessageW`:** `SendMessageW` from a hook callback is synchronous — deadlocks because the WndProc runs on the same thread. `PostMessageW` only.

**`GetAsyncKeyState` for panic:** Hook blocks the keystroke (`return 1`), so `GetMessage`-based detection won't see it. `GetAsyncKeyState` reads raw hardware state regardless — use it for the panic hold timer.

**Wide strings:** Win32 takes `*const u16`. Use:
```rust
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
```

**`panic = "abort"`:** Already in release profile. Never change. Panics in `extern "system"` with unwind = undefined behavior.

**Hook thread safety:** `WH_KEYBOARD_LL` callbacks are called on the main thread (same thread that called `SetWindowsHookExW`). No cross-thread access from hooks. Only the updater thread crosses threads — it only calls `PostMessageW` (safe). Atomics are sufficient, no `Mutex` needed anywhere in hook path.

---

## What NOT to Do

- **No `BlockInput()`** — blocks ALL input including synthetic. Defeats the entire purpose.
- **No `RegisterHotKey()`** — conflicts with apps; doesn't work while hook is suppressing input.
- **No direct calls from hook callbacks** — only `PostMessageW`. No `lock()`, no `unlock()`, no anything that can block.
- **No `SendMessageW` from hooks or threads** — deadlock. `PostMessageW` only.
- **No `Mutex` in hook callbacks** — can block. Atomics only.
- **No `windows` crate** (high-level) — use `windows-sys`. Better GNU support.
- **No async runtime** — not needed. `std::thread::spawn` + `PostMessageW` is sufficient.
- **No JSON crate** — parse `tag_name` with `str::find`. No `serde` needed.
- **No Ctrl+Alt+Del blocking** — impossible in user mode. Don't try.

---

## `wraith.ini` (ship alongside .exe)

```ini
; Modifier bitmask: MOD_ALT=1, MOD_CONTROL=2, MOD_SHIFT=4, MOD_WIN=8
; Ctrl+Shift+Alt = 7
[Wraith]
LockModifiers=7
LockKey=76
UnlockModifiers=7
UnlockKey=85
PanicKey=27
LockOnStart=0
```

---

## GitHub Actions CI

```yaml
name: Build & Release
on:
  push:
    tags: ['v*.*.*']
  workflow_dispatch:

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: x86_64-pc-windows-gnu }
      - run: sudo apt-get install -y gcc-mingw-w64-x86-64
      - run: cargo build --release --target x86_64-pc-windows-gnu
      - run: mkdir -p release && cp target/x86_64-pc-windows-gnu/release/wraith.exe release/ && cp wraith.ini release/
      - uses: actions/upload-artifact@v4
        with: { name: wraith-windows-x64, path: release/* }
      - if: startsWith(github.ref, 'refs/tags/v')
        uses: softprops/action-gh-release@v2
        with:
          files: release/wraith.exe\nrelease/wraith.ini
          fail_on_unmatched_files: false
        env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" }

  installer:
    runs-on: windows-latest
    needs: build
    if: startsWith(github.ref, 'refs/tags/v')
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with: { name: wraith-windows-x64 }
      - run: makensis installer\wraith.nsi
        shell: cmd
        continue-on-error: true
      - uses: softprops/action-gh-release@v2
        with: { files: installer/wraith-setup.exe, fail_on_unmatched_files: false }
        env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" }
```

---

## File Structure

```
wraith/
├── src/
│   ├── main.rs        Entry point, single-instance, init, message loop
│   ├── app.rs         Lock/Unlock, WndProc, autostart
│   ├── hooks.rs       Hook install/uninstall, KeyboardProc, MouseProc, atomics
│   ├── tray.rs        Shell_NotifyIcon, menu, balloons
│   ├── config.rs      Config struct, INI load/save
│   └── updater.rs     Background thread, WinHTTP, version compare
├── docs/
│   ├── SKILLS.md      Matt Pocock skill workflow guide
│   ├── PRD.md         Product requirements (for /to-prd)
│   ├── DOMAIN.md      Domain model (for /domain-modeling)
│   ├── ISSUES.md      Feature breakdown (for /to-issues)
│   ├── ADR.md         Architecture decisions (for /improve-codebase-architecture)
│   └── TESTS.md       TDD test specs (for /tdd)
├── .cargo/
│   └── config.toml
├── .github/
│   └── workflows/
│       └── build.yml
├── installer/
│   └── wraith.nsi
├── src/resource.rc    Version info + UAC manifest (Step 9)
├── wraith.manifest    UAC + DPI (referenced by resource.rc)
├── Cargo.toml
├── CLAUDE.md          ← this file
├── wraith.ini
├── LICENSE
└── README.md
```

---

## Agent skills

### Issue tracker

Issues live in GitHub Issues on `shadow-dragon-2002/Wraith`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default Matt Pocock vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context repo. Domain glossary: `CONTEXT.md` at repo root. Architecture decisions: `docs/adr/`. See `docs/agents/domain.md`.

---

*Built by shadow-dragon-2002. MIT license. https://github.com/shadow-dragon-2002/Wraith*
