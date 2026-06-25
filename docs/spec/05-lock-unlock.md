# Spec 05 — Lock / Unlock State

> Research prerequisite: [../research/05-lock-unlock.md](../research/05-lock-unlock.md)
> Implements: `src/app.rs` (lock, unlock, toggle, wnd_proc) — Step 5

---

## Purpose

Manage the locked/unlocked state of Wraith. `lock()` activates input blocking and
prevents system sleep. `unlock()` restores normal input and clears sleep prevention.
Both are idempotent. All state changes go through the main thread via WndProc.

---

## Public Interface

```rust
pub fn lock(hwnd: HWND);
pub fn unlock(hwnd: HWND);
pub fn toggle(hwnd: HWND);

pub unsafe extern "system" fn wnd_proc(
    hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM
) -> LRESULT;
```

---

## TrayIcon Access Pattern

All functions that need to update the tray retrieve the `TrayIcon` pointer from
window user data:
```rust
let tray = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayIcon);
```
This pointer is set in `main.rs` after `TrayIcon::new(hwnd)` and before any of
these functions can be called.

---

## lock()

**R1.** If `LOCKED.load(Relaxed) == true`: return immediately (idempotent).

**R2.** `LOCKED.store(true, Relaxed)`.

**R3.** `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED)`.
Prevents sleep and display-off while locked.

**R4.** Retrieve `TrayIcon` via `GWLP_USERDATA`. Call `tray.set_locked(true)`.

**R5.** `SetTimer(hwnd, TIMER_PANIC, 100, NULL)` — starts the panic hold timer at
100ms intervals. Killed by `unlock()`.

---

## unlock()

**R6.** If `LOCKED.load(Relaxed) == false`: return immediately (idempotent).

**R7.** `LOCKED.store(false, Relaxed)`.

**R8.** `SetThreadExecutionState(ES_CONTINUOUS)` — clears all execution state flags.

**R9.** `KillTimer(hwnd, TIMER_PANIC)`.

**R10.** `PANIC_START.store(0, Relaxed)` — reset panic timer state.

**R11.** Retrieve `TrayIcon`. Call `tray.set_locked(false)`.

---

## toggle()

**R12.** If `LOCKED.load(Relaxed)`: call `unlock(hwnd)`. Else: call `lock(hwnd)`.

---

## wnd_proc

Signature: `pub unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT`

### WM_COMMAND

**R13.** Match on `LOWORD(wp)`:
- `ID_LOCK (1001)` → `lock(hwnd)`
- `ID_UNLOCK (1002)` → `unlock(hwnd)`
- `ID_AUTOSTART (1003)` → `set_autostart(!is_autostart())` then rebuild menu check state
- `ID_EXIT (1004)` → `DestroyWindow(hwnd)`

### WM_TRAY_MSG

**R14.** Match on `LOWORD(lp)`:
- `WM_RBUTTONUP | WM_CONTEXTMENU` → `tray.show_menu(hwnd, LOCKED.load(Relaxed))`
- `WM_LBUTTONDBLCLK` → `toggle(hwnd)`
- otherwise → ignore, return 0

### WM_TIMER

**R15.** Match on `wp` (timer ID):
- `TIMER_PANIC (2001)` → panic unlock check (see spec 06)

### WM_UPDATE_RESULT

**R16.** `lp` is a raw pointer to a heap `String` (from updater):
```rust
let s = Box::from_raw(lp as *mut String);
tray.show_balloon("Wraith Update Available", &s);
```
Free the box by dropping `s` at end of scope.

### WM_ENDSESSION

**R17.** If `wp != 0` (shutdown is proceeding, not cancelled):
- Retrieve TrayIcon. Call `tray.destroy()`.
- `hooks::uninstall()`.
- `PostQuitMessage(0)`.

### WM_DESTROY

**R18.**
- Retrieve TrayIcon pointer. Call `tray.destroy()`. Drop the Box:
  ```rust
  let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayIcon;
  if !ptr.is_null() { drop(Box::from_raw(ptr)); }
  SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
  ```
- `hooks::uninstall()`.
- `PostQuitMessage(0)`.

### TaskbarCreated (dynamic message ID)

**R19.** Store the result of `RegisterWindowMessageW(L"TaskbarCreated")` in a
`static TASKBAR_CREATED: AtomicU32` during init (before the message loop).
In `wnd_proc`, if `msg == TASKBAR_CREATED.load(Relaxed)`:
call `Shell_NotifyIconW(NIM_ADD, ...)` to restore the tray icon.

### Default

**R20.** All unhandled messages: `return DefWindowProcW(hwnd, msg, wp, lp)`.

---

## Constants

```rust
pub const WM_TRAY_MSG:      u32   = WM_USER + 1;
pub const WM_UPDATE_RESULT: u32   = WM_USER + 2;
pub const ID_LOCK:          usize = 1001;
pub const ID_UNLOCK:        usize = 1002;
pub const ID_AUTOSTART:     usize = 1003;
pub const ID_EXIT:          usize = 1004;
pub const TIMER_PANIC:      usize = 2001;
```

---

## Dependencies

- `hooks.rs` — `LOCKED`, `PANIC_START`, `uninstall()`
- `tray.rs` — `TrayIcon` accessed via `GWLP_USERDATA`
- `config.rs` — `Config::get()` for `lock_on_start` in main init
- `updater.rs` — posts `WM_UPDATE_RESULT`

---

## Edge Cases

- **`WM_DESTROY` before `WM_UPDATE_RESULT` is processed:** The Box from the updater
  leaks. Acceptable — OS reclaims at process exit.
- **`toggle()` from tray double-click while locked:** Tray messages bypass the hook.
  This is intentional — it is a secondary unlock path for RDP users.
- **`WM_ENDSESSION` with `wp == 0`:** Shutdown was cancelled. Do nothing, return 0.
- **`SetWindowLongPtrW` to 0 after free:** Prevents double-free if `WM_DESTROY`
  fires twice (which Windows can do in error paths).
