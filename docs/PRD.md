# Wraith v1.0 — Product Requirements Document

---

## Problem Statement

Users running AI computer-use agents (Claude, computer-use workflows, AHK automation, RPA) on Windows need the machine to remain active and unattended for extended periods. Windows provides no mechanism that satisfies all three constraints simultaneously:

- **Win+L (lock screen):** terminates the active session, disconnects all AI tools
- **Screensaver / display sleep:** interrupts visual AI workflows that need screen state
- **Leave unprotected:** anyone at the desk can accidentally or intentionally interrupt the AI session by touching keyboard or mouse

`BlockInput()` — the obvious Win32 API — blocks everything including software-generated input, defeating the AI use case. There is no built-in selective filter.

Windows does expose one hook: every input event is tagged with an injected flag (`LLKHF_INJECTED` for keyboard, `LLMHF_INJECTED` for mouse) when it originates from software rather than hardware. This flag is set by `SendInput()`, Chrome DevTools Protocol injection, AHK `Send`, RDP/Parsec input stacks, and any other `keybd_event()`-based mechanism. It is never set for physical hardware events.

---

## Solution

Wraith is a Windows system-tray utility that installs `WH_KEYBOARD_LL` and `WH_MOUSE_LL` low-level hooks. On every input event, it checks the injected flag first. If set, the event passes through unconditionally. If not set and Wraith is in locked state, the event is consumed (the hook returns 1 without calling `CallNextHookEx`). If not set and Wraith is in unlocked state, the event passes through normally.

The user locks via hotkey combo (default Ctrl+Shift+Alt+L), unlocks via hotkey combo (default Ctrl+Shift+Alt+U), or uses a panic unlock (hold Esc for 3 seconds). A system-tray icon shows current state. Config lives in a plain-text INI file.

AI agents, Chrome extensions, RDP sessions, and any other software-originated input continue to work normally regardless of lock state.

---

## User Stories

### Input Blocking

1. As an AI operator, I want physical keyboard input suppressed when locked, so nobody at my desk can type into my running AI session.
2. As an AI operator, I want physical mouse clicks suppressed when locked, so nobody can redirect my AI session to a different application.
3. As an AI operator, I want physical mouse movement suppressed when locked, so accidental mouse nudges cannot change the cursor position the AI is targeting.
4. As an AI operator, I want scroll wheel input suppressed when locked, so accidental scrolling cannot shift the page state the AI is reading.
5. As an AI operator, I want all physical input blocked regardless of which application is focused, so lock state applies system-wide.

### Synthetic Input Passthrough

6. As an AI tool using `SendInput`, I want my keyboard events to pass through when the machine is locked, so I can continue typing into target applications.
7. As an AI tool using `SendInput`, I want my mouse click events to pass through when the machine is locked, so I can continue clicking UI elements.
8. As an AI tool using `SendInput`, I want my mouse movement events to pass through when the machine is locked, so I can move to target coordinates.
9. As a Chrome extension injecting input via CDP, I want my injected keystrokes to pass through when the machine is locked, so AI browser automation continues.
10. As a remote desktop or Parsec user, I want my remote input to pass through when the machine is locked, so I can check in on the running AI session without needing to physically unlock first.
11. As an AHK script, I want my `Send` commands to pass through when the machine is locked, so automation scripts continue running alongside AI workflows.

### Lock / Unlock

12. As an AI operator, I want to lock physical input with a keyboard combo, so I can activate lock without touching the tray icon.
13. As an AI operator, I want the lock hotkey combo to be Ctrl+Shift+Alt+L by default, so I have a sensible out-of-the-box binding.
14. As an AI operator, I want to unlock physical input with a keyboard combo, so I can restore normal input quickly.
15. As an AI operator, I want the unlock hotkey combo to be Ctrl+Shift+Alt+U by default, so I have a sensible out-of-the-box binding.
16. As an AI operator, I want the lock hotkey to work even when an application has keyboard focus, so I can lock from any context.
17. As an AI operator, I want the lock and unlock combos to be consumed (not forwarded), so the target application does not receive them as stray key events.
18. As an AI operator, I want lock combos sent via `SendInput` to still work, so an AI agent can lock/unlock programmatically.

### Panic Unlock

19. As an AI operator, I want a panic unlock that works even when I cannot type the unlock combo, so I am never permanently locked out of my own machine.
20. As an AI operator, I want panic unlock triggered by holding a key for 3 continuous seconds, so brief accidental contact does not trigger it.
21. As an AI operator, I want the panic key to be Escape by default, so it is easy to find under pressure.
22. As an AI operator, I want the panic timer to reset when I release the panic key before 3 seconds, so short holds stay locked.
23. As an AI operator, I want panic unlock to work even though the hook is consuming panic key events, so the hook itself does not prevent the emergency exit.

### System Tray

24. As an AI operator, I want a tray icon in the Windows notification area, so I can see at a glance whether Wraith is locked or unlocked.
25. As an AI operator, I want the tray icon to visually distinguish locked vs unlocked state, so I never have to guess the current state.
26. As an AI operator, I want the tray icon to update immediately on state change, so the icon never shows stale state.
27. As an AI operator, I want right-clicking the tray icon to show a context menu, so I can control Wraith without remembering hotkeys.
28. As an AI operator, I want the context menu to have a Lock option, so I can lock from the tray.
29. As an AI operator, I want the context menu to have an Unlock option, so I can unlock from the tray.
30. As an AI operator, I want the Lock option greyed out when already locked, so the menu reflects current state.
31. As an AI operator, I want the Unlock option greyed out when already unlocked, so the menu reflects current state.
32. As an AI operator, I want the context menu to have an Auto-start toggle, so I can enable or disable auto-start without opening registry editors.
33. As an AI operator, I want the context menu to have an Exit option, so I can quit Wraith cleanly.
34. As an AI operator, I want double-clicking the tray icon to toggle lock state, so RDP users who cannot use the hotkey have an accessible alternative.

### Sleep / Display Prevention

35. As an AI operator, I want the display to stay on while Wraith is running, so the AI agent's visual context is preserved.
36. As an AI operator, I want the system to stay awake while Wraith is running, so background AI processes keep running.
37. As an AI operator, I want normal Windows sleep behavior restored when Wraith exits, so I am not permanently stuck in a no-sleep state.

### Configuration

38. As an AI operator, I want to configure the lock hotkey modifier bitmask in wraith.ini, so I can use custom modifier combos.
39. As an AI operator, I want to configure the lock hotkey virtual key code in wraith.ini, so I can choose the key character.
40. As an AI operator, I want to configure the unlock hotkey separately from the lock hotkey, so they can be different bindings.
41. As an AI operator, I want to configure the panic key virtual key code in wraith.ini, so I can choose a different fallback key.
42. As an AI operator, I want to enable lock-on-start in wraith.ini, so Wraith locks immediately when launched.
43. As an AI operator, I want a missing or invalid wraith.ini to fall back to sensible defaults, so startup never fails due to config issues.
44. As an AI operator, I want config loaded once at startup, so runtime behavior is deterministic for the session lifetime.

### Auto-start

45. As an AI operator, I want to enable Wraith to auto-start with Windows login, so I do not have to manually launch it each session.
46. As an AI operator, I want auto-start stored in the user registry Run key, so it activates for my user only and requires no admin rights.
47. As an AI operator, I want disabling auto-start via the tray menu to immediately remove the registry entry, so the setting takes effect on next boot.
48. As an AI operator, I want the tray menu Auto-start item to reflect the current registry state, so I know whether it is enabled.
49. As an AI operator, I want the auto-start path to be quoted in the registry if it contains spaces, so Windows parses the command correctly.

### Update Notifications

50. As an AI operator, I want Wraith to check GitHub for a newer release on startup, so I learn about updates passively.
51. As an AI operator, I want update checks to run on a background thread, so the hook pump and UI are never blocked.
52. As an AI operator, I want a balloon notification when a newer version is available, so I can decide to upgrade.
53. As an AI operator, I want no notification when running the latest version, so Wraith stays quiet when nothing is needed.
54. As an AI operator, I want network errors during update check to fail silently, so a temporary outage does not cause crashes or noise.
55. As an AI operator, I want version comparison to be numeric rather than lexicographic, so version 1.10.0 is correctly newer than 1.9.0.

### Single Instance

56. As an AI operator, I want only one Wraith instance to run at a time, so duplicate hooks and conflicting state cannot occur.
57. As an AI operator, I want a second launch attempt to exit immediately with a clear message, so I know Wraith is already running.

### Build and Distribution

58. As a developer, I want CI to build a release .exe on every version tag push, so artifacts are produced automatically.
59. As a developer, I want CI to publish wraith.exe and wraith.ini to GitHub Releases on version tags, so users can download without building from source.
60. As a developer, I want an NSIS installer produced on version tags, so users have a one-click install experience.
61. As an administrator, I want the installer to place Wraith in Program Files, so it lives in a standard location.
62. As an administrator, I want the installer to register Wraith in Add/Remove Programs, so it can be uninstalled cleanly.
63. As a developer, I want the .exe to embed its manifest, version info, and icons as PE resources, so it presents correctly in Explorer and Task Manager.
64. As a developer, I want the UAC manifest to request asInvoker execution level, so Wraith does not prompt for elevation.

---

## Implementation Decisions

### Module Boundaries

Six modules with clearly defined public interfaces:

- **main.rs** — single-instance mutex, init sequence, `GetMessageW` loop
- **config.rs** — `Config` struct, INI load, `OnceLock` accessor
- **tray.rs** — `TrayIcon` struct wrapping `Shell_NotifyIconW` lifecycle, menu, balloons
- **hooks.rs** — `install`/`uninstall`, `keyboard_proc`/`mouse_proc` hook callbacks, global atomics
- **app.rs** — `lock`/`unlock`/`toggle`, WndProc, autostart registry reads/writes
- **updater.rs** — background thread, WinHTTP call sequence, version parse, `PostMessageW` result

### Global State for Hook Callbacks

Hook callbacks are `extern "system" fn` and cannot close over any state. All state they access must be global. Five globals cover all needs:

- `LOCKED: AtomicBool` — current lock state
- `KB_HOOK: AtomicUsize` — HHOOK for keyboard hook (stored as usize to avoid `!Send`)
- `MOUSE_HOOK: AtomicUsize` — HHOOK for mouse hook
- `APP_HWND: AtomicUsize` — HWND of the message-only window (for PostMessageW from hook and updater thread)
- `PANIC_START: AtomicU32` — GetTickCount snapshot for panic timer

No `Mutex` anywhere in the hook-callback path. Atomics are sufficient; any blocking primitive would risk exceeding the 1000ms callback timeout and silently losing the hook.

### Injected Flag Check Ordering

In both `keyboard_proc` and `mouse_proc`, the first check after the mandatory `nCode < 0` early-return must be the injected flag:

- Keyboard: `flags & 0x10 != 0` (LLKHF_INJECTED, bit 4) — set for all software-originated input including lower-IL injection (bit 1 always co-sets bit 4)
- Mouse: `flags & 0x01 != 0` (LLMHF_INJECTED, bit 0)

If the injected flag is set, call `CallNextHookEx` unconditionally and return. Do not check lock state. Do not check combos. This ordering ensures AI tool passthrough is evaluated before any blocking logic.

### Blocking Events

To block an event: return any nonzero value from the hook callback. Do NOT call `CallNextHookEx`. The hook callback returns `LRESULT`; returning `1` is sufficient and conventional.

### Combo Detection

Hotkey combos are detected inside `keyboard_proc` using `GetAsyncKeyState` to read modifier state plus `vkCode` from `KBDLLHOOKSTRUCT`. When a combo is detected, post `WM_COMMAND` with the relevant ID to `APP_HWND` and consume the event (return 1, do not call `CallNextHookEx`). Never call `app::lock()` or `app::unlock()` directly from a hook callback.

### Panic Unlock Timer

On lock: `SetTimer(hwnd, TIMER_PANIC, 100, None)`. On unlock: `KillTimer(hwnd, TIMER_PANIC)` and reset `PANIC_START` to 0.

Each `WM_TIMER/TIMER_PANIC` tick:
- If `GetAsyncKeyState(panic_vk) & 0x8000u16 as i16 != 0` and `PANIC_START == 0`: store `GetTickCount()` in `PANIC_START`
- If key held and `GetTickCount().wrapping_sub(PANIC_START.load(Relaxed)) >= 3000`: call `unlock(hwnd)`
- If key released and `PANIC_START != 0`: reset `PANIC_START` to 0

`GetTickCount()` returns `u32` that wraps at ~49.7 days — always use `wrapping_sub`.

`GetAsyncKeyState` reads raw hardware state and works correctly even when the hook is consuming the keystroke.

### HWND_MESSAGE Window

The application creates a message-only window (`CreateWindowExW` with `HWND_MESSAGE` as parent). This window:
- Receives `WM_COMMAND`, `WM_TRAY_MSG`, `WM_UPDATE_RESULT`, `WM_TIMER`, `WM_DESTROY` messages
- Drives the hook pump via `GetMessageW` loop on the main thread
- Does NOT receive `WM_QUERYENDSESSION`, `WM_ENDSESSION`, or `TaskbarCreated` broadcast — these go only to top-level windows

No `WM_ENDSESSION` handler should be implemented. OS process cleanup reclaims all hooks and tray icons on shutdown; no action needed.

### TrayIcon Storage

`TrayIcon` is heap-allocated via `Box::new(TrayIcon::new(hwnd))` immediately after window creation. The raw pointer is stored in window user data via `SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize)`. Retrieved in `wnd_proc` and `lock()`/`unlock()` via `GetWindowLongPtrW`. Freed on `WM_DESTROY` via `Box::from_raw`.

### Tray Icon Version Protocol

After every `Shell_NotifyIconW(NIM_ADD, ...)`, immediately call `Shell_NotifyIconW(NIM_SETVERSION, ...)` with `uVersion = NOTIFYICON_VERSION_4 = 4`. This enables the `WM_TRAY_MSG` lParam encoding where `LOWORD(lParam)` is the notification event and `HIWORD(lParam)` is the icon ID.

### Tray Context Menu

Before calling `TrackPopupMenu`, call `SetForegroundWindow(hwnd)`. After `TrackPopupMenu` returns, call `DestroyMenu(menu)` — `TrackPopupMenu` does not free the menu.

### WM_COMMAND Dispatch

`wParam` is packed: low word = command ID, high word = notification code. Always extract with `(wp & 0xFFFF) as usize` (or `LOWORD(wp)`). Never compare `wp == ID_LOCK` directly.

### SetThreadExecutionState Invariant

`SetThreadExecutionState` is thread-scoped and must be called from the main thread only. `lock()` and `unlock()` are only ever called from `wnd_proc` (which runs on the main thread). Never call them from hook callbacks or the updater thread. Violation causes sleep prevention to silently do nothing.

- On lock: `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED)`
- On unlock: `SetThreadExecutionState(ES_CONTINUOUS)`

### Config Loading

`Config::load()` reads `wraith.ini` via `GetPrivateProfileIntW`. INI path is resolved relative to the executable via `GetModuleFileNameW`. `GetPrivateProfileIntW` returns `nDefault` if the file or key is absent — no crash. Config is stored in `OnceLock<Config>` and accessed via `Config::get()`. INI is not written back at runtime.

### Update Checker

Runs on `std::thread::spawn`. WinHTTP sequence: Open session → Connect to `api.github.com:443` → `GET /repos/shadow-dragon-2002/Wraith/releases/latest` with `WINHTTP_FLAG_SECURE` → read body loop (EOF = `TRUE` return + `bytes_read == 0`). Parses `tag_name` via `str::find` with no JSON crate. Compares as `(u32, u32, u32)` tuples — no semver crate. Posts `Box<String>` pointer via `PostMessageW(APP_HWND, WM_UPDATE_RESULT, 0, ptr as LPARAM)`. WndProc frees via `Box::from_raw`. Network errors are silently swallowed.

`build.rs` must emit `cargo:rustc-link-lib=winhttp` — `windows-sys` does not auto-link `winhttp.lib` for the GNU target.

### Autostart Registry

HKCU Run key: `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, value name `"Wraith"`, type `REG_SZ`. Value must be the quoted executable path (e.g. `"C:\Program Files\Wraith\wraith.exe"`) — unquoted paths with spaces are misparse by Windows. `RegSetValueExW` cbData must include the null terminator: `(wide_len + 1) * 2` bytes. `RegDeleteValueW` returning `ERROR_FILE_NOT_FOUND(2)` on disable is not an error.

### Dependencies and Toolchain

- `windows-sys = "0.59"` (NOT `windows`) — better GNU target compatibility, no proc-macro complications
- Cross-compiled on Ubuntu via `x86_64-pc-windows-gnu` target with `gcc-mingw-w64-x86-64`
- `panic = "abort"` in release profile — panics in `extern "system"` with unwind = undefined behavior
- Resource embedding: `x86_64-w64-mingw32-windres src/resource.rc` → PE object linked via `cargo:rustc-link-arg`

### Single-Instance Mutex

`CreateMutexW(NULL, FALSE, "Global\\WraithSingleInstance")` — call `GetLastError` immediately after. If `ERROR_ALREADY_EXISTS (183)`: show `MessageBoxW("Wraith is already running.")` then exit. The handle is valid even on `ERROR_ALREADY_EXISTS`; the error code is the signal, not a null handle.

---

## Testing Decisions

### What makes a good test

Test observable external behavior at the highest available seam. Do not test internal atomics, intermediate WndProc message routing, or Win32 call sequences — these are implementation details that will change. Test what a caller or user would observe.

### Seam 1 — CI Build (automated, runs on Ubuntu)

`cargo build --release --target x86_64-pc-windows-gnu` must succeed. This validates:
- All modules compile
- `windows-sys` feature flags cover every API used
- `winhttp.lib` links correctly (validates `build.rs` `cargo:rustc-link-lib=winhttp`)
- No Rust type errors, no missing symbols

### Seam 2 — Pure Logic Units (automated, runs anywhere)

The only logic that is fully platform-independent and unit-testable:
- `parse_tag` in `updater.rs`: given a JSON body string, returns `Some((major, minor, patch))` or `None`; test with real-shaped GitHub API response, malformed body, missing `tag_name`, no releases (404-shaped body)
- `Config::load()`: given a temp INI file with known values, returns correct struct; given missing file, returns defaults; given invalid (non-integer) value for a field, returns field-level default
- Version tuple comparison: `(1, 10, 0) > (1, 9, 0)` must hold (numeric, not lexicographic)

These can live as `#[cfg(test)]` modules. No test framework beyond `cargo test` needed.

### Seam 3 — Manual Windows Integration

These require a live Windows session and cannot be automated without a Windows test runner:

| Scenario | Pass condition |
|---|---|
| Launch Wraith | Tray icon appears; no crash |
| Second launch | Message box says "already running"; second process exits |
| Lock via hotkey | Tray icon changes to locked state; physical typing produces no characters |
| Physical key while locked | Key has no effect in any focused window |
| `SendInput` key while locked | Key reaches target window |
| Unlock via hotkey | Tray icon changes to unlocked; typing works again |
| Panic unlock (hold Esc 3s) | Unlocks after 3s |
| Panic key release before 3s | Stays locked; re-hold resets timer |
| Right-click tray | Menu shows Lock/Unlock/Autostart/Exit; correct item greyed |
| Double-click tray | Toggles lock state |
| Enable autostart | Registry key created; survives reboot |
| Disable autostart | Registry key deleted |
| GitHub reachable, newer version | Balloon notification appears |
| GitHub unreachable | No crash, no notification |
| Missing wraith.ini | Wraith starts with defaults |
| Custom hotkeys in wraith.ini | Custom combos work |
| RDP input while locked | Remote keystrokes reach applications |

---

## Out of Scope

- **Tray icon recovery after Explorer crash** — `TaskbarCreated` broadcast is not received by message-only (`HWND_MESSAGE`) windows; the icon cannot be re-registered automatically. Accepted for v1.0; workaround is to restart Wraith.
- **Autostart cleanup in NSIS uninstaller** — elevated uninstaller process cannot access the logged-in user's `HKCU`; the Run key entry would be deleted from the wrong hive. Workaround: disable autostart via tray menu before uninstalling.
- **Ctrl+Alt+Del blocking** — SAS (Secure Attention Sequence) is kernel-hardwired; impossible to intercept in user mode.
- **Per-application input rules** — requires Interception kernel-mode driver; out of scope for user-mode v1.
- **GUI settings panel** — INI file is sufficient; a settings UI adds complexity without proportional benefit.
- **Password-protected unlock** — adds UI complexity and a credential-storage problem; panic unlock is the security model.
- **macOS / Linux support** — Windows-only by design; hooks are Win32-specific.
- **Logging / audit trail** — no requirement; hook callbacks must not do I/O.
- **Remote unlock** — unlock is always local (hotkey, panic, or tray double-click).
- **Per-device filtering** — distinguishing between multiple keyboards/mice requires a kernel-mode driver (e.g. Interception); out of scope.

---

## Further Notes

- `KBDLLHOOKSTRUCT_FLAGS` is a transparent type alias (`pub type KBDLLHOOKSTRUCT_FLAGS = u32`), not a newtype. Direct bitwise operations compile without any cast.
- Hook callback timeout is 1000ms on Win10 1709+ (registry: `HKCU\Control Panel\Desktop\LowLevelHooksTimeout`). Keep callbacks under 200ms for safety margin. Silent unhook on timeout — no detection mechanism.
- `WH_KEYBOARD_LL` callbacks fire on the main thread (same thread that called `SetWindowsHookExW`). Only the updater thread crosses threads; it only uses `PostMessageW`. No `Mutex` is needed anywhere.
- `PostMessageW` from hook callbacks and background threads: safe. `SendMessageW` from those contexts: deadlock. Never use `SendMessageW` except from `wnd_proc` itself.
- UAC manifest must request `asInvoker` — LL hooks do NOT require elevation.
