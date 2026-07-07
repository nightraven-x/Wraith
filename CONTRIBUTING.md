# Contributing to Wraith

Everything you need to understand, build, extend, and contribute to Wraith.

---

## Table of Contents

1. [Project overview](#1-project-overview)
2. [Architecture](#2-architecture)
3. [Module reference](#3-module-reference)
4. [Data flow](#4-data-flow)
5. [Build system](#5-build-system)
6. [Testing](#6-testing)
7. [Configuration reference](#7-configuration-reference)
8. [Architecture decisions](#8-architecture-decisions)
9. [Known limitations](#9-known-limitations)
10. [Release process](#10-release-process)
11. [Contributing guidelines](#11-contributing-guidelines)

---

## 1. Project overview

Wraith blocks physical keyboard and mouse input while letting synthetic (software-generated) input pass through unaffected. The intended use case is running an AI agent or automation script on a machine and locking the physical peripherals so no one at the desk can interfere while the AI works.

### The core mechanism

Windows tags every low-level input event with an **injected flag**:

| Flag | Constant | Value | Meaning |
|------|----------|-------|---------|
| Keyboard | `LLKHF_INJECTED` | bit 4 of `KBDLLHOOKSTRUCT.flags` | Event came from `SendInput`, `keybd_event`, etc. |
| Mouse | `LLMHF_INJECTED` | bit 0 of `MSLLHOOKSTRUCT.flags` | Event came from `SendInput`, `mouse_event`, etc. |

Wraith installs `WH_KEYBOARD_LL` and `WH_MOUSE_LL` global hooks. Every event goes through these callbacks before reaching any application. The decision tree is:

```
Event arrives at hook
  ├─ Is the injected flag set?  YES → pass through (AI / remote / script)
  ├─ Is the lock combo pressed? YES → lock or unlock, consume the event
  └─ Is LOCKED == true?         YES → return 1 (block — do not call CallNextHookEx)
                                NO  → pass through
```

No kernel driver required. No `BlockInput()` (which blocks synthetic too). Pure Win32.

### What passes through while locked

- `SendInput()` from any process (AI agents, AutoHotKey, PowerShell)
- Remote Desktop Protocol (RDP) input
- Parsec / VNC remote input
- Chrome extension injection
- Any `keybd_event()` or `mouse_event()` call

### What is blocked while locked

- Physical keyboard keystrokes
- Physical mouse movement, clicks, scroll wheel

### Hard limits (cannot be changed in user mode)

- `Ctrl+Alt+Del` is the Windows Secure Attention Sequence (SAS) — hardwired into the kernel, unreachable from user-mode hooks. Its secure-desktop screen has its own Task Manager, which Wraith closes off by disabling Task Manager (`DisableTaskMgr`) for the duration of the lock — see ADR-0008.
- A process with sufficient privilege (e.g. Task Manager as Administrator, before the lock disables it, or `taskkill` from an elevated shell) can still terminate Wraith — Windows' security model doesn't let a user-mode process protect itself from that.

---

## 2. Architecture

### Thread model

Wraith is single-threaded except for the update checker:

```
Main thread
  ├─ GetMessageW loop (drives WH_KEYBOARD_LL / WH_MOUSE_LL)
  ├─ WndProc (processes all WM_* messages)
  ├─ hook callbacks (keyboard_proc, mouse_proc — called by Windows on this thread)
  └─ WM_TIMER handlers (panic unlock, hook watchdog)

Updater thread (std::thread::spawn)
  └─ WinHTTP fetch → PostMessageW(WM_UPDATE_RESULT) → back to main thread
```

**Critical:** The hook pump and WndProc share the main thread. A hook callback runs synchronously — the `GetMessageW` loop is suspended for its duration. This means:

- Hook callbacks must be O(1). No I/O, no blocking, no mutex waits.
- Communication from hook → app is always via `PostMessageW` (async). Never `SendMessageW` (deadlocks).

### Global state

All shared state lives in `hooks.rs` as lock-free atomics. This is a deliberate architectural choice (see ADR-0003):

```rust
pub static LOCKED:   AtomicBool  = AtomicBool::new(false);  // lock state
pub static APP_HWND: AtomicUsize = AtomicUsize::new(0);      // HWND for PostMessageW
pub static APP_TRAY: AtomicUsize = AtomicUsize::new(0);      // *mut TrayIcon heap pointer
```

Private atomics (hook handles, panic timer) live in the same module but are not `pub`.

### Window

Wraith creates an `HWND_MESSAGE` (message-only) window — invisible, never rendered, sole purpose is to drive the hook pump and receive messages. This is the canonical pattern for background Win32 services.

### TrayIcon ownership

`TrayIcon` is heap-allocated as a `Box<TrayIcon>` immediately after window creation. The raw pointer is stored in `APP_TRAY`. `impl Drop for TrayIcon` calls `Shell_NotifyIconW(NIM_DELETE)` to remove the icon automatically when the Box is dropped.

---

## 3. Module reference

### `main.rs` — Entry point

**Responsibilities:** single-instance enforcement, init sequence, message pump.

**Init sequence (order is load-bearing):**

```
0. argv[1] == "--cleanup-taskmgr"?
     └─ app::startup_cleanup(); ExitProcess(0)
     This is the RunOnce failsafe's own re-invocation of the exe (ADR-0008),
     not a normal launch — checked BEFORE the single-instance mutex so it
     runs whether or not a real Wraith instance is already up.

1. CreateMutexW("Global\WraithSingleInstance")
     └─ ERROR_ALREADY_EXISTS → MessageBox + exit

2. Config::get()            — load wraith.ini into OnceLock

3. app::startup_cleanup()   — crash cleanup: delete DisableTaskMgr if left by prior crash

4. RegisterClassExW + CreateWindowExW(HWND_MESSAGE) → hwnd

5. RegisterWindowMessageW("TaskbarCreated") → TASKBAR_CREATED
     └─ Explorer restart recovery: re-add tray icon when Explorer crashes

6. APP_TRAY.store(Box::into_raw(Box::new(TrayIcon::new(hwnd))))

7. hooks::install(hwnd)
     └─ Err → MessageBox + ExitProcess(1)

8. if lock_on_start: app::lock()

9. updater::spawn()

10. SetTimer(hwnd, TIMER_WATCHDOG, 5000, None)
      └─ Reinstalls hooks every 5s to survive Parsec/RDP virtual driver teardown

11. GetMessageW loop
```

**Public constants:**

| Constant | Value | Purpose |
|----------|-------|---------|
| `WM_TRAY_MSG` | `WM_USER + 1` | Tray icon callback message |
| `WM_UPDATE_RESULT` | `WM_USER + 2` | Updater thread result |
| `ID_LOCK` | `1001` | WM_COMMAND id |
| `ID_UNLOCK` | `1002` | WM_COMMAND id |
| `ID_AUTOSTART` | `1003` | WM_COMMAND id |
| `ID_EXIT` | `1004` | WM_COMMAND id |
| `ID_SETTINGS` | `1005` | WM_COMMAND id — opens the settings dialog |
| `TIMER_PANIC` | `2001` | 100ms panic-unlock poll |
| `TIMER_WATCHDOG` | `2002` | 5s hook reinstall watchdog |
| `CLEANUP_TASKMGR_FLAG` | `"--cleanup-taskmgr"` | internal-only argv flag, see step 0 above |

---

### `hooks.rs` — Low-level input hooks

**Responsibilities:** install/uninstall hooks, keyboard/mouse callbacks, global atomics, panic timer logic, hook watchdog.

**Public API:**

```rust
// Atomics — read anywhere, written only from hooks.rs and app.rs
pub static LOCKED:   AtomicBool
pub static APP_HWND: AtomicUsize   // HWND as usize
pub static APP_TRAY: AtomicUsize   // *mut TrayIcon as usize
pub static SETTINGS_OPEN: AtomicBool  // true while the settings dialog is up

// Install both WH_KEYBOARD_LL and WH_MOUSE_LL. Stores APP_HWND.
// Returns Err with a static string on failure — caller must show MessageBox + exit.
pub fn install(hwnd: HWND) -> Result<(), &'static str>

// Uninstall both hooks. Safe to call when already uninstalled (no-op).
pub fn uninstall()

// Reinstall hooks to recover from silent removal. Called by TIMER_WATCHDOG.
pub fn watchdog()

// Advance panic hold timer. Returns true when panic_vk held >= 3000ms.
// Must be called on every TIMER_PANIC tick.
pub fn panic_key_tick() -> bool

// Reset panic hold timer to zero. Called from unlock().
pub fn panic_reset()
```

**Keyboard hook decision tree (keyboard_proc):**

```
nCode < 0?           → CallNextHookEx (MSDN mandate, no processing)
LLKHF_INJECTED set?  → CallNextHookEx (synthetic, pass through)
SETTINGS_OPEN?       → CallNextHookEx (dialog is up, full passthrough)
WM_KEYDOWN/SYSKEYDOWN?
  lock_vk + mods_held(lock_mods)?   → PostMessageW(ID_LOCK) + return 1
  unlock_vk + mods_held(unlock_mods)? → PostMessageW(ID_UNLOCK) + return 1
LOCKED == true?
  WM_KEYUP/SYSKEYUP + is_modifier_vk(vk)?  → CallNextHookEx (pass modifier key-ups)
  otherwise                                  → return 1 (block)
fallthrough           → CallNextHookEx (pass through, unlocked)
```

**Why the hooks pass everything through while the settings dialog is open:**

Recording the *currently active* lock/unlock combo in the settings dialog would otherwise fire the real lock/unlock action mid-edit — `keyboard_proc` matches it before the keystroke ever reaches the dialog's hotkey-recorder control. Worse, if Wraith was already locked when the dialog opened, its own controls (including Cancel) would be unreachable via physical input. `settings::show()` sets `SETTINGS_OPEN` via an RAII guard for the dialog's lifetime, so it comes back down even on an unexpected unwind; `mouse_proc` has the same check.

**Why modifier key-ups pass through when locked:**

When the lock combo fires (e.g. `Ctrl+Shift+Alt+L`), the modifier key-DOWN events have already passed through (unlocked state). After `LOCKED` becomes true, the subsequent key-UP events for `Ctrl`, `Shift`, `Alt` arrive. If blocked, the OS thinks these modifiers are still held (stuck). Any subsequent synthetic keystroke (e.g. from Parsec) then arrives with OS modifier state `Ctrl+Shift+Alt+X` → garbled output. Passing modifier key-UPs through releases the stuck state. Key-UPs alone cannot type text or trigger combos.

**Hook callback timeout:**

Windows silently removes a hook if its callback does not return within `LowLevelHooksTimeout` (default 300ms, configurable via `HKCU\Control Panel\Desktop\LowLevelHooksTimeout`). Every callback must be O(1). The watchdog timer (`TIMER_WATCHDOG`, 5s interval) reinstalls hooks if they were silently removed (e.g. by Parsec's virtual keyboard driver teardown during disconnect).

---

### `app.rs` — Lock/unlock logic and WndProc

**Responsibilities:** `lock()`, `unlock()`, `toggle()`, `wnd_proc`, DisableTaskMgr policy + RunOnce crash-safety failsafe (private), startup crash cleanup.

**Public API:**

```rust
pub fn lock()    // Set LOCKED=true, block Task Manager, start panic timer, update tray
pub fn unlock()  // Set LOCKED=false, unblock Task Manager, kill panic timer, update tray
pub fn toggle()  // lock() if unlocked, unlock() if locked

pub unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT
```

**lock() side effects (in order):**

1. Early-return if already locked
2. `LOCKED.store(true, Relaxed)`
3. `task_mgr_block()` — set `DisableTaskMgr = 1` in registry and register the
   `RunOnce\WraithTaskMgrCleanup` failsafe entry (private to `app.rs`; no-ops
   if the registry write is denied — see ADR-0008's consequences)
4. `SetTimer(hwnd, TIMER_PANIC, 100ms)` — start panic unlock poll
5. `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED)` — prevent sleep/screensaver
6. `tray().set_locked(true)` — update tray icon + tooltip

**unlock() side effects (in order):**

1. Early-return if already unlocked
2. `LOCKED.store(false, Relaxed)`
3. `task_mgr_unblock()` — delete `DisableTaskMgr` and the `RunOnce` failsafe
   entry from the registry (private to `app.rs`)
4. `KillTimer(hwnd, TIMER_PANIC)` — stop panic poll
5. `hooks::panic_reset()` — clear hold timer
6. `SetThreadExecutionState(ES_CONTINUOUS)` — restore sleep/screensaver
7. `tray().set_locked(false)` — update tray icon + tooltip

**WndProc message table:**

| Message | Condition | Action |
|---------|-----------|--------|
| `WM_TRAY_MSG` | `lp == WM_RBUTTONUP \|\| WM_CONTEXTMENU` | `tray().show_menu(hwnd)` |
| `WM_TRAY_MSG` | `lp == WM_LBUTTONDBLCLK` | `toggle()` |
| `WM_COMMAND` | `LOWORD(wp) == ID_LOCK` | `lock()` |
| `WM_COMMAND` | `LOWORD(wp) == ID_UNLOCK` | `unlock()` |
| `WM_COMMAND` | `LOWORD(wp) == ID_AUTOSTART` | Toggle autostart registry entry |
| `WM_COMMAND` | `LOWORD(wp) == ID_EXIT` | `DestroyWindow(hwnd)` |
| `WM_COMMAND` | `LOWORD(wp) == ID_SETTINGS` | `settings::show(hwnd)` — reachable whether locked or unlocked |
| `WM_TIMER` | `wp == TIMER_PANIC && LOCKED && panic_key_tick()` | `unlock()` |
| `WM_TIMER` | `wp == TIMER_WATCHDOG` | `hooks::watchdog()` |
| `WM_UPDATE_RESULT` | `lp != 0` | Free `Box<String>`, show balloon |
| `WM_DESTROY` | — | `hooks::uninstall()`, `task_mgr_unblock()` **unconditionally** (not gated on `LOCKED` — see ADR-0008), drop TrayIcon Box, `PostQuitMessage(0)` |
| `TASKBAR_CREATED` | — | `tray().re_add()` — recover from Explorer crash |

**Important:** `WM_ENDSESSION` and `WM_QUERYENDSESSION` are NOT delivered to `HWND_MESSAGE` windows. The OS reclaims hooks and tray icon on process exit — no shutdown handler is needed or possible. This is exactly why `WM_DESTROY`'s `task_mgr_unblock()` call can't be conditional on a clean `unlock()` first: a plain "Exit while locked" from the tray menu is a real, deterministic path that never touches `unlock()`.

---

### `tray.rs` — System tray icon

**Responsibilities:** `Shell_NotifyIconW` lifecycle, icon loading, context menu, balloon notifications.

**Public API:**

```rust
pub struct TrayIcon { /* opaque */ }

impl TrayIcon {
    pub fn new(hwnd: HWND) -> Self        // NIM_ADD + NIM_SETVERSION
    pub fn set_locked(&mut self, locked: bool)  // NIM_MODIFY icon + tooltip
    pub fn show_balloon(&self, title: &str, msg: &str)  // NIM_MODIFY with NIF_INFO
    pub fn show_menu(&self, hwnd: HWND)   // CreatePopupMenu + TrackPopupMenu
    pub fn re_add(&self)                  // NIM_ADD again after Explorer restart
}

impl Drop for TrayIcon {
    fn drop(&mut self)  // NIM_DELETE — icon removed automatically when Box drops
}
```

**Icon loading:**

Icons are embedded as Win32 resources via `windres` (see `build.rs`). Resource IDs:
- `1` → `assets/unlocked-white.ico` (shown when unlocked)
- `2` → `assets/locked-white.ico` (shown when locked)

`LoadImageW` loads from the embedded resource. If resource loading fails, falls back to `IDI_APPLICATION` (system default). Both paths are always available — the fallback prevents the tray from being invisible on any build.

**Notification version:**

`NIM_SETVERSION` with `NOTIFYICON_VERSION_4` is called after `NIM_ADD`. This enables `WM_CONTEXTMENU` and `NIN_*` messages on Vista+ (required for correct right-click menu positioning).

---

### `config.rs` — INI configuration

**Responsibilities:** Load `wraith.ini` via `GetPrivateProfileIntW`, cache in `OnceLock`, write changes back for the settings dialog, resolve portable vs. `%LOCALAPPDATA%` location.

**Public API:**

```rust
pub struct Config {
    // AtomicU32/AtomicBool, not plain values — the settings dialog changes
    // these at runtime with no hook reinstall needed. keyboard_proc re-reads
    // Config::get() fresh on every keydown. See ADR-0007.
    pub lock_mods: AtomicU32, pub lock_vk: AtomicU32,
    pub unlock_mods: AtomicU32, pub unlock_vk: AtomicU32,
    pub panic_vk: AtomicU32, pub lock_on_start: AtomicBool,
}

impl Config {
    pub fn load() -> Self;          // reads wraith.ini, falls back to defaults
    pub fn get() -> &'static Self;  // OnceLock accessor, loads once on first call
    pub fn write_back(&self);       // persists all 6 keys via WritePrivateProfileStringW
}
```

**Why atomics, not a `Mutex<Config>` or a swappable `&'static Config`:** ADR-0003 already rules out blocking calls in the hook path — a `Mutex` held by the dialog while the hook tries to read it would violate that. Swapping the whole struct (e.g. via `ArcSwap`) would work but adds a dependency and indirection five `AtomicU32`s solve directly. `decide_action` (`hooks.rs`) still takes `&Config` as a plain, pure, testable parameter — call sites just add `.load(Relaxed)`.

`lock_on_start` is the one field that's atomic for `write_back()`'s sake only — the settings dialog's checkbox needs to persist a change to it, but the in-memory value has no live effect; it's read exactly once, in `main.rs`, at startup. Changing it takes effect on the *next* launch.

**Race note (ADR-0007):** a keydown landing mid-write may read a partially-updated combo (new `lock_vk` with the old `lock_mods`, or vice versa). Self-corrects on the next keystroke — accepted as cheaper than synchronizing the 5 fields as one unit; the failure mode is one misfired combo attempt, not a crash or stuck lock.

**INI path resolution (`ini_path()`, ADR-0009):** re-checked dynamically on every call, not cached.
1. If `wraith.ini` already exists next to the `.exe` (`GetModuleFileNameW` + `GetFileAttributesW`), use it — portable mode, unchanged behavior.
2. Otherwise, resolve to `%LOCALAPPDATA%\Wraith\wraith.ini`, creating the `Wraith` subdirectory if needed (`CreateDirectoryW`, `ERROR_ALREADY_EXISTS` is fine).

This exists because Program-Files installs run `asInvoker` (no elevation) and can't write there — without the fallback, settings changes would work in memory but silently fail to persist. Test code that deletes the portable candidate to exercise the AppData branch must restore it before releasing shared test state, or "portable missing" leaks into whichever test runs next — see `RestorePortableIniOnDrop` and `lock_config_test()`, the single entry point all config/settings tests use to acquire the shared test lock for exactly this reason.

**Missing INI:** Falls back to compiled-in defaults. No error is shown. Users can always recover by editing or deleting `wraith.ini`.

---

### `autostart.rs` — Windows startup entry

**Responsibilities:** Read/write the `HKCU\...\Run` registry key for launch-at-login.

**Public API:**

```rust
pub fn enable()       // Write quoted exe path to Run key
pub fn disable()      // Delete Run key value
pub fn is_enabled() -> bool  // Query Run key for Wraith value
```

**Registry key:** `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value `Wraith`.

**Quoted path:** The exe path is wrapped in double quotes (`"C:\Program Files\Wraith\wraith.exe"`) so paths containing spaces survive the Run key parsing.

---

### `settings.rs` — Native settings dialog

**Responsibilities:** `DIALOGEX` modal dialog (panic key, lock combo, unlock combo, lock-on-start), all-or-nothing validation and commit, dark-mode/rounded-corner theming, suspending the hooks while open.

**Public API:**

```rust
pub fn show(hwnd: HWND);  // DialogBoxParamW — modal, safe from wnd_proc (see below)
```

**Why a modal dialog is safe to call synchronously from `wnd_proc`:** `DialogBoxParamW` pumps its own message loop on the calling thread until `EndDialog`. The hook callbacks live on that same thread's queue — `DialogBoxParamW` keeps dispatching everything on it while the dialog is up, so it does not stop the hook pump. It does, however, mean physical input to the dialog itself would be blocked by `LOCKED == true` like anything else — which is exactly why `hooks::SETTINGS_OPEN` exists (see the hooks.rs section above): `show()` sets it for the dialog's lifetime via an RAII guard, so it's cleared even on an unexpected unwind.

**Validation, all-or-nothing on OK:**
- Panic key must be `1..=255` — `0` is Win32's reserved "no key" value; accepting it would silently make panic-unlock unreachable (found in a security audit — see ADR-0007's amendment history).
- Lock and unlock combos must have at least one modifier — `decide_action` (`hooks.rs`) matches a 0-modifier combo unconditionally (`held_mods & 0 == 0` is vacuously true), which would hijack that key system-wide. The panic key is exempt — it's single-key by design, checked via a separate hold-timer, not `decide_action`.
- Either every field validates and gets stored + `write_back()`'d, or nothing does — an invalid field keeps the dialog open with an inline error (`IDC_VALIDATION_ERROR`) instead of a partial commit.

**Fields commit directly into the `Config` atomics** (`.store(v, Relaxed)`), taking effect on the very next keydown — no hook reinstall needed, since `keyboard_proc` already re-reads `Config::get()` fresh on every event.

---

### `hotkey_recorder.rs` — Live combo capture control

**Responsibilities:** subclass an existing `EDIT` control so it records the next key combo pressed while focused, instead of accepting typed numeric input.

**Public API:**

```rust
pub fn install(hwnd: HWND, initial: (u32, u32));  // subclasses an existing EDIT control
pub fn value(hwnd: HWND) -> (u32, u32);            // reads back the committed (mods, vk)
```

Classic `SetWindowLongPtrW(GWLP_WNDPROC, ...)` subclassing (not ComCtl32's `SetWindowSubclass`) — the previous `WNDPROC` and per-instance state live in a `Box` addressed via `GWLP_USERDATA`; unhandled messages are forwarded with `CallWindowProcW`; the `Box` is freed on `WM_NCDESTROY`, after forwarding to the original proc first.

`GetAsyncKeyState` is reliable here — unlike in `hooks.rs`'s `LOCKED`-blocking case, this control never blocks the event, so Windows keeps hardware modifier state current.

Deliberately dumb and reusable: it does not itself reject a zero-modifier combo. That policy belongs to whichever dialog consumes it — see `settings.rs`'s validation above.

---

### `theme.rs` — Settings dialog dark mode

**Responsibilities:** detect the system light/dark preference, apply dark titlebar + Windows 11 rounded corners + themed child controls to the settings dialog.

**Public API:**

```rust
pub fn system_prefers_dark() -> bool;  // reads AppsUseLightTheme from the registry
pub fn apply(hwnd: HWND, dark: bool, controls: &[HWND]);
```

`system_prefers_dark()` reads `HKCU\...\Themes\Personalize\AppsUseLightTheme`; a missing key or read failure defaults to light, matching Windows' own default before a user ever visits Settings > Personalization > Colors.

`apply()` sets `DWMWA_USE_IMMERSIVE_DARK_MODE` and `DWMWA_WINDOW_CORNER_PREFERENCE` (`DWMWCP_ROUND`) on the dialog itself, and `SetWindowTheme(..., "DarkMode_Explorer", ...)` on each child control passed in. Called once from `settings.rs`'s `WM_INITDIALOG` — purely cosmetic, no effect on hooks, lock/unlock, or the main message loop. DWM attributes that don't exist pre-Windows-10-1809 just fail silently (ignored `HRESULT`) — the dialog still renders fine via ComCtl32 v6, just without the extras.

---

### `updater.rs` — Background update checker

**Responsibilities:** Fetch latest GitHub release tag via WinHTTP, compare with current version, post balloon if newer.

**Flow:**

```
std::thread::spawn
  └─ fetch_latest()     — WinHTTP GET api.github.com/repos/.../releases/latest
  └─ parse_tag(body)    — extract "tag_name" value from JSON (no serde)
  └─ parse_ver(tag)     — parse to (u32, u32, u32) tuple
  └─ compare with env!("CARGO_PKG_VERSION")
  └─ if newer: Box<String> → PostMessageW(WM_UPDATE_RESULT, ptr as LPARAM)
WndProc receives WM_UPDATE_RESULT
  └─ Box::from_raw(lp) → show_balloon → drop
```

**Version comparison:** Parsed as `(major, minor, patch)` tuples and compared numerically. String comparison is intentionally avoided — `"1.10.0" > "1.9.0"` must hold.

**Tag format:** GitHub tags must follow `vX.Y.Z` exactly (e.g. `v1.2.3`). The leading `v` is stripped before parsing.

**Error handling:** Any network error, parse failure, or non-2xx response is silently ignored — the update check is best-effort. No retry.

**WinHTTP on MinGW:** The `build.rs` emits `cargo:rustc-link-lib=winhttp` because `windows-sys` does not auto-link `winhttp.lib` for the GNU target.

---

## 4. Data flow

### Physical keypress while locked

```
Physical key down
  → Windows kernel
  → WH_KEYBOARD_LL chain (our keyboard_proc called first or somewhere in chain)
  → keyboard_proc checks:
      nCode < 0?           → no
      LLKHF_INJECTED?      → no (bit 4 not set)
      lock/unlock combo?   → no
      LOCKED == true?      → yes
      is WM_KEYUP + modifier? → no (it's KEYDOWN)
  → return 1              ← event consumed, no application sees it
```

### Parsec remote keystroke while locked

```
Parsec virtual driver injects keystroke via SendInput()
  → Windows kernel sets LLKHF_INJECTED (bit 4) in flags
  → keyboard_proc checks:
      LLKHF_INJECTED?      → YES (bit 4 set)
  → CallNextHookEx         ← passes through to target application
```

### Lock combo pressed (Ctrl+Shift+Alt+L, physical, unlocked)

```
Ctrl down   → keyboard_proc → LOCKED==false → PASS-PHYS → CallNextHookEx
Shift down  → keyboard_proc → LOCKED==false → PASS-PHYS → CallNextHookEx
Alt down    → keyboard_proc → LOCKED==false → PASS-PHYS → CallNextHookEx
L down      → keyboard_proc → combo check fires
              → PostMessageW(hwnd, WM_COMMAND, ID_LOCK, 0)
              → return 1 (consume L keydown)
              ← GetMessageW loop processes WM_COMMAND
              ← app::lock() runs: LOCKED=true, DisableTaskMgr set, timer started
L up        → keyboard_proc → LOCKED==true → not modifier key-up → return 1 (blocked)
Alt up      → keyboard_proc → LOCKED==true → is modifier key-up → CallNextHookEx (pass)
Shift up    → keyboard_proc → LOCKED==true → is modifier key-up → CallNextHookEx (pass)
Ctrl up     → keyboard_proc → LOCKED==true → is modifier key-up → CallNextHookEx (pass)
```

The modifier key-UPs must pass through to prevent the OS from thinking Ctrl/Shift/Alt are permanently held (which would corrupt all subsequent synthetic keystrokes).

### Panic unlock (hold Esc 3 seconds while locked)

```
Every 100ms (TIMER_PANIC):
  WM_TIMER → hooks::panic_key_tick()
    → GetAsyncKeyState(panic_vk) checks raw hardware state
       (works even though hook is blocking the keystroke)
    → if held: record start time or check elapsed
    → if elapsed >= 3000ms: return true
  → app::unlock()
  → LOCKED=false, DisableTaskMgr removed, timer killed
```

### Update check flow

```
App startup → updater::spawn(hwnd)
  → background thread: WinHTTP → api.github.com
  → parse tag → compare version
  → if newer: PostMessageW(WM_UPDATE_RESULT, box_ptr)
Main thread: GetMessageW → DispatchMessageW → wnd_proc
  → WM_UPDATE_RESULT: tray.show_balloon("Wraith Update", msg)
  → Box::from_raw(lp) freed
```

---

## 5. Build system

### Toolchain requirements

| Tool | Purpose |
|------|---------|
| Rust (stable) | Compiler |
| `x86_64-pc-windows-gnu` target | Cross-compile to Windows |
| `gcc-mingw-w64-x86-64` | GNU linker + assembler for Windows target |
| `x86_64-w64-mingw32-windres` | Compile `.rc` resource files to `.o` |

### One-time WSL setup

```bash
rustup target add x86_64-pc-windows-gnu
sudo apt update && sudo apt install -y gcc-mingw-w64-x86-64
```

Verify:

```bash
x86_64-w64-mingw32-gcc --version
x86_64-w64-mingw32-windres --version
```

### Cargo configuration (`.cargo/config.toml`)

```toml
[build]
target = "x86_64-pc-windows-gnu"

[target.x86_64-pc-windows-gnu]
linker   = "x86_64-w64-mingw32-gcc"
ar       = "x86_64-w64-mingw32-ar"
rustflags = ["-C", "link-arg=-Wl,--subsystem,windows"]
```

The `--subsystem,windows` flag suppresses the console window. Without it, Wraith would spawn a black terminal on launch.

### build.rs

`build.rs` does two things:

1. **Links `winhttp.lib`** — `windows-sys` does not auto-link this for the GNU target:
   ```rust
   println!("cargo:rustc-link-lib=winhttp");
   ```

2. **Compiles Win32 resources** via `windres`:
   ```rust
   // src/resource.rc → target/.../resource.o → linked into binary
   x86_64-w64-mingw32-windres src/resource.rc -o $OUT_DIR/resource.o
   cargo:rustc-link-arg=$OUT_DIR/resource.o
   ```

   `windres` failure is non-fatal (prints a warning, skips embedding). The binary still works — tray falls back to `IDI_APPLICATION`.

### `src/resource.rc`

```rc
1 ICON "../assets/unlocked-white.ico"   // Resource ID 1 = unlocked state
2 ICON "../assets/locked-white.ico"     // Resource ID 2 = locked state
1 RT_MANIFEST "../wraith.manifest"      // UAC + DPI manifest
```

### `wraith.manifest`

Embedded Win32 manifest. Sets:
- `requestedExecutionLevel level="asInvoker"` — Wraith runs at the invoker's privilege level
- DPI awareness: `PerMonitorV2, PerMonitor`
- Supported OS GUIDs: Windows 7 through Windows 11

To require Administrator elevation (stronger privacy protection), change `asInvoker` to `requireAdministrator`. This adds a UAC prompt on each launch.

### Build commands

```bash
# Debug build (faster, larger, no strip)
cargo build --target x86_64-pc-windows-gnu

# Release build (optimised, stripped, ~50KB)
cargo build --release --target x86_64-pc-windows-gnu

# Output
target/x86_64-pc-windows-gnu/release/wraith.exe
```

### Release profile (`Cargo.toml`)

```toml
[profile.release]
opt-level     = "z"    # optimise for size
lto           = true   # link-time optimisation
codegen-units = 1      # single codegen unit for max LTO
panic         = "abort" # REQUIRED — panics in extern "system" with unwind = UB
strip         = true   # strip debug symbols
```

`panic = "abort"` is non-negotiable. The hook callbacks are `extern "system"` functions. If a panic unwinds across an FFI boundary it is undefined behaviour. Abort terminates the process immediately instead.

### Custom icons

Add `.ico` files to `assets/` and reference them in `src/resource.rc`:

```rc
1 ICON "../assets/your-unlocked.ico"
2 ICON "../assets/your-locked.ico"
```

Icon requirements:
- Format: `.ico` with multiple sizes (16x16, 32x32, 48x48 recommended)
- The tray uses 16x16; larger sizes are for the taskbar and file explorer

---

## 6. Testing

### Running tests

```bash
cargo test --target x86_64-pc-windows-gnu
```

### What is tested

Tests are limited to pure-logic modules with no Win32 side effects:

| Test | Module | What it verifies |
|------|--------|-----------------|
| `parse_tag_extracts_version` | `updater.rs` | JSON tag_name extraction |
| `parse_tag_returns_none_on_missing` | `updater.rs` | Graceful missing-field handling |
| `parse_ver_strips_v_prefix` | `updater.rs` | v-prefix normalisation |
| `parse_ver_numeric_comparison_correct` | `updater.rs` | "1.10.0 > 1.9.0" holds |
| `parse_ver_returns_none_on_invalid` | `updater.rs` | Malformed version safety |
| `parse_tag_handles_whitespace_and_compact_json` | `updater.rs` | JSON format variants |

### What cannot be unit tested

The following have no correct test seam and must be verified manually on a running Windows machine:

- **Hook callbacks** (`keyboard_proc`, `mouse_proc`) — require a live Win32 message pump
- **Tray icon lifecycle** — requires `Shell_NotifyIconW` and a real session
- **Lock/unlock side effects** — `SetTimer`, `SetThreadExecutionState`, registry writes
- **Panic unlock timing** — `GetAsyncKeyState` + `GetTickCount` + live keyboard state
- **WinHTTP fetch** — live network required

When adding a new feature, identify the seam at which pure logic can be extracted and tested separately from the Win32 side effect.

---

## 7. Configuration reference

`wraith.ini` lives in the same directory as `wraith.exe`. Missing values fall back to defaults. The file is read once at startup — restart Wraith after editing.

```ini
[Wraith]

; Modifier bitmask: MOD_ALT=1, MOD_CONTROL=2, MOD_SHIFT=4, MOD_WIN=8
; Combine with addition: Ctrl+Shift+Alt = 2+4+1 = 7
LockModifiers=7       ; default: Ctrl+Shift+Alt
LockKey=76            ; default: L (virtual key code 76)

UnlockModifiers=7     ; default: Ctrl+Shift+Alt
UnlockKey=85          ; default: U (virtual key code 85)

PanicKey=27           ; default: Escape (hold for 3 seconds)

LockOnStart=0         ; 0 = start unlocked, 1 = lock immediately on launch
```

### Virtual key code reference

| Key | Code | Key | Code |
|-----|------|-----|------|
| A–Z | 65–90 | 0–9 | 48–57 |
| Escape | 27 | Enter | 13 |
| F1–F12 | 112–123 | Space | 32 |
| Tab | 9 | Backspace | 8 |
| Insert | 45 | Delete | 46 |
| Home | 36 | End | 35 |
| Page Up | 33 | Page Down | 34 |

Full list: https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes

### Modifier bitmask

| Modifier | Bit value |
|----------|-----------|
| `MOD_ALT` | 1 |
| `MOD_CONTROL` | 2 |
| `MOD_SHIFT` | 4 |
| `MOD_WIN` | 8 |

Combine with addition: `Ctrl+Alt = 2+1 = 3`, `Ctrl+Shift+Alt = 2+4+1 = 7`.

---

## 8. Architecture decisions

All ADRs live in `docs/adr/`. Summary:

### ADR-0001: Rust over C++ and Go

Go has garbage-collection pauses that can exceed the `WH_KEYBOARD_LL` callback timeout (~200ms safe limit). This causes the hook to be silently removed mid-session — verified broken in practice. C++ works but lacks memory safety at the FFI boundary. Rust gives zero-cost `extern "system"` callbacks and full memory safety without a GC.

### ADR-0002: `windows-sys` over `windows` crate

`windows-sys` has better GNU target (MinGW) compatibility. The high-level `windows` crate uses proc-macros that add complexity and have known issues with the `x86_64-pc-windows-gnu` toolchain.

### ADR-0003: Global atomics over Mutex

Hook callbacks run on the main thread. Mutexes can block. A blocked callback exceeds the timeout and kills the hook silently. All shared state is `AtomicBool` / `AtomicUsize` with `Ordering::Relaxed` — sufficient for this single-core-equivalent access pattern.

### ADR-0004: `PostMessageW` from hooks, never `SendMessageW`

`SendMessageW` from a hook callback would try to deliver the message to the WndProc synchronously. WndProc runs on the same thread. The hook is blocking that thread. Deadlock. `PostMessageW` queues the message and returns immediately.

### ADR-0005: `panic = "abort"` in release profile

Panics in `extern "system"` functions with `panic = "unwind"` produce undefined behaviour as the stack unwinds across an FFI boundary. `abort` terminates the process immediately. This is always correct behaviour for a hook callback — there is nothing meaningful to recover from mid-callback.

### ADR-0006: No async runtime

`tokio`, `async-std`, etc. bring thread pools, runtime overhead, and dependency weight. The updater's single background task is trivially handled by `std::thread::spawn` + `PostMessageW`. One extra dependency for one HTTP call is not justified.

### ADR-0007: Mutable config atomics for the settings dialog

`Config`'s five runtime-tunable fields (`lock_mods`, `lock_vk`, `unlock_mods`, `unlock_vk`, `panic_vk`) become `AtomicU32`; `lock_on_start` becomes `AtomicBool`. Supersedes the earlier "config is read-only at runtime" design — issues #12/#13 needed the settings dialog to change hotkeys live and persist them via `write_back()`. A `Mutex<Config>` was rejected (ADR-0003 already bans blocking in the hook path); swapping the whole `&'static Config` (e.g. `ArcSwap`) was rejected as unnecessary indirection for a problem five atomics solve directly. Accepted tradeoff: a keydown landing mid-write may read a stale/partial combo for one event — self-corrects on the next keystroke.

### ADR-0008: RunOnce failsafe for a stuck DisableTaskMgr

A security audit found `WM_DESTROY` never called `task_mgr_unblock()` — exiting while locked left `DisableTaskMgr` stuck with no process left to clear it. Fixed by making that call unconditional. The deeper problem — a forced kill, crash, or power loss gives a dying process no chance to clean up at all — is mitigated (not fully fixed, since it's a hard OS limitation) with an `HKCU\...\RunOnce` entry that re-invokes `wraith.exe --cleanup-taskmgr` at the next interactive logon. Removing the DisableTaskMgr feature entirely was considered and rejected: `Ctrl+Alt+Del`'s secure-desktop Task Manager is otherwise a full escape hatch. A companion watchdog process/service was also considered and rejected as conflicting with the single-.exe, no-async-runtime, minimal-dependency constraints — `RunOnce` gets the same crash-safety property from a registry write the codebase already makes routinely.

### ADR-0009: %LOCALAPPDATA% fallback for config location

`Config::load()`/`write_back()` now resolve through `ini_path()`: use the exe-relative `wraith.ini` if one already exists (portable mode, unchanged), otherwise fall back to `%LOCALAPPDATA%\Wraith\wraith.ini`. Fixes a real gap — the installer puts Wraith in Program Files, the manifest requests `asInvoker` (no elevation), and a standard user account can't write there, so settings changes would work in memory but silently fail to survive a restart. An AppData-only design was rejected: it would silently relocate config for existing portable users the moment they upgrade. The resolution is re-checked on every call (not cached), so tests that delete the portable candidate to exercise the AppData branch must restore it before releasing shared test state.

---

## 9. Known limitations

| Limitation | Reason | Workaround |
|------------|--------|------------|
| `Ctrl+Alt+Del` cannot be blocked | Kernel-hardwired SAS | `lock()` sets `DisableTaskMgr` in registry, blocking Task Manager from the Ctrl+Alt+Del menu |
| A forced kill/crash/power-loss can leave `DisableTaskMgr` stuck | Dying process gets no chance to clean up | `RunOnce` failsafe clears it at next interactive logon (ADR-0008) — bounded to "until next logon", not fully eliminated |
| Registry writes to `Policies\System` can be denied (GPO/hardening) | Windows security policy, seen on at least one dev machine | `task_mgr_block()` no-ops in that case; no RunOnce entry is registered either since nothing needs cleaning up |
| Wraith can still be terminated by a sufficiently privileged process before/outside the lock (e.g. an elevated `taskkill`) | Windows security model | Run with `requireAdministrator` manifest; standard user accounts cannot kill admin processes |
| Hook silently removed if callback exceeds `LowLevelHooksTimeout` | Windows enforcement | Callbacks are O(1); 5s watchdog timer reinstalls hooks if removed |
| `WM_ENDSESSION` not received by message-only windows | HWND_MESSAGE limitation | OS reclaims hooks and tray on process exit — no action needed |
| `SendInput` from another MEDIUM-IL process can inject past hooks | By design | Wraith's purpose is to allow this — it is the feature, not a bug |
| `lock_on_start` change requires restart | Read once into `OnceLock` at startup, unlike the other 5 config fields | Toggle it in the settings dialog or `wraith.ini`, then restart Wraith |

---

## 10. Release process

### GitHub Actions CI

The workflow at `.github/workflows/build.yml` has two triggers:

1. **`workflow_dispatch`** — manual trigger from GitHub Actions UI or `gh workflow run`. Builds `wraith.exe` and uploads as an artifact (`wraith-windows-x64`). Does not create a GitHub Release.

2. **`push` on `v*.*.*` tags** — builds, creates a GitHub Release, attaches `wraith.exe` + `wraith.ini`. Also runs the NSIS installer build on `windows-latest` and attaches `wraith-setup.exe`.

### Creating a release

1. Update version in `Cargo.toml`:
   ```toml
   version = "1.1.0"
   ```

2. Commit and push:
   ```bash
   git add Cargo.toml Cargo.lock
   git commit -m "chore: bump version to 1.1.0"
   git push
   ```

3. Tag and push:
   ```bash
   git tag v1.1.0
   git push origin v1.1.0
   ```

   GitHub Actions builds automatically, creates a Release, and attaches the artifacts.

### NSIS installer

The installer script lives at `installer/wraith.nsi`. Built on `windows-latest` in CI via `makensis`. The installer:
- Copies `wraith.exe` and `wraith.ini` to `%PROGRAMFILES64%\Wraith`
- Creates Start Menu shortcuts
- Writes an Add/Remove Programs entry
- Installs an uninstaller that runs `taskkill` before removing files

To build locally (requires NSIS 3.x on Windows):
```cmd
makensis installer\wraith.nsi
```

---

## 11. Contributing guidelines

### Code style

- **Rust edition 2021**
- **No `clippy` warnings** — run `cargo clippy --target x86_64-pc-windows-gnu` before submitting
- **No comments on obvious code** — only add a comment when the WHY is non-obvious (hidden invariant, Win32 quirk, workaround)
- **No `unsafe` outside FFI calls** — `unsafe` blocks should be as small as possible, wrapping only the actual Win32 call
- **No panics in hot paths** — hook callbacks must not panic. Use `if ptr.is_null() { return; }` rather than `.unwrap()`

### Adding a new feature

1. Check `docs/adr/` — ensure your approach doesn't contradict an existing decision
2. If introducing new global state: add it to `hooks.rs` as an `AtomicUsize` or `AtomicBool`
3. If adding Win32 registry operations: consider adding them to `autostart.rs` (Run key) or `app.rs` (lock side effects) rather than a new module unless the concern is genuinely distinct
4. If adding a new WM_* message: define the constant in `main.rs`, handle it in `wnd_proc` in `app.rs`
5. Test any pure logic (parsing, calculations) in a `#[cfg(test)]` module. Document absent seams explicitly

### Hook callback constraints (critical)

Any code called from `keyboard_proc` or `mouse_proc` must follow these rules:

- **No blocking calls** — no I/O, no `Mutex::lock()`, no `std::thread::sleep()`
- **No heap allocation** — `Box::new()` inside a callback is technically safe but risks slow paths under memory pressure
- **No `PostMessageW` loops** — one `PostMessageW` per callback invocation maximum
- **No direct state mutation of TrayIcon or config** — these are not atomic; all state changes go through the message loop

### Submitting changes

1. Fork the repo, create a branch
2. Run `cargo build --release --target x86_64-pc-windows-gnu` — must succeed
3. Run `cargo clippy --target x86_64-pc-windows-gnu` — zero warnings
4. Run `cargo test --target x86_64-pc-windows-gnu` — all pass
5. Test manually on a Windows machine: lock, unlock, panic unlock, tray menu, Parsec passthrough
6. Open a pull request against `main` with a conventional commit message (`fix:`, `feat:`, `refactor:`, `chore:`)

### Conventional commit prefixes

| Prefix | When to use |
|--------|------------|
| `feat:` | New user-visible capability |
| `fix:` | Bug fix |
| `refactor:` | Code change with no behaviour difference |
| `chore:` | Build, CI, deps, docs, tooling |
| `perf:` | Performance improvement |

---

*Wraith — GNU GPL v3.0 — https://github.com/shadow-dragon-2002/Wraith*
