# Spec 02 — Message Window + Pump

> Research prerequisite: [../research/02-message-pump.md](../research/02-message-pump.md)
> Implements: `src/main.rs` (pump loop) — Step 1

---

## Purpose

Create an invisible message-only window (`HWND_MESSAGE`) and run a `GetMessageW` loop
that serves two roles:
1. Drives `WH_KEYBOARD_LL` / `WH_MOUSE_LL` hook callbacks (mandatory — hooks stop
   firing if this loop blocks or exits).
2. Dispatches application messages (`WM_COMMAND`, `WM_TRAY_MSG`, `WM_TIMER`, etc.)
   to `wnd_proc` in `app.rs`.

---

## Public Interface

No exported functions. The pump is the `main` function's final act after init.

---

## Window Registration

**R1.** Call `RegisterClassExW` with:
- `cbSize = size_of::<WNDCLASSEXW>()`
- `lpfnWndProc = app::wnd_proc`
- `hInstance = GetModuleHandleW(NULL)`
- `lpszClassName` = wide `"WraithWnd"`
- All other fields: zero / NULL.

**R2.** On `RegisterClassExW` failure (returns 0): `MessageBoxW` the error, exit.

### Window Creation

**R3.** Call `CreateWindowExW` with:
- `dwExStyle = 0`
- `lpClassName` = wide `"WraithWnd"`
- `lpWindowName` = wide `"Wraith"`
- `dwStyle = 0`
- Position/size: `0, 0, 0, 0`
- `hWndParent = HWND_MESSAGE` — makes it a message-only window
- `hMenu = NULL`, `hInstance = GetModuleHandleW(NULL)`, `lpParam = NULL`

**R4.** On `CreateWindowExW` failure (returns NULL): `MessageBoxW` the error, exit.

---

## Init Sequence (full, in order)

```
1. single-instance mutex check          (spec 07)
2. Config::load()                       (spec 03)
3. RegisterClassExW + CreateWindowExW   (this spec)
4. APP_HWND.store(hwnd as usize, Relaxed)
5. TrayIcon::new(hwnd) → Box → SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr)  (spec 04)
6. hooks::install(hwnd) → on Err: MessageBoxW + ExitProcess(1)              (spec 01)
7. if Config::get().lock_on_start { app::lock(hwnd) }                       (spec 05)
8. updater::spawn(hwnd)                                                      (spec 09)
9. GetMessageW loop                     (this spec)
```

---

## Message Loop

**R5.** Loop:
```rust
let mut msg = MSG::default();
while GetMessageW(&mut msg, NULL, 0, 0) != 0 {
    TranslateMessage(&msg);
    DispatchMessageW(&msg);
}
```

**R6.** `GetMessageW` returning 0 means `WM_QUIT` was posted (via `PostQuitMessage`).
Exit the loop and return from `main`.

**R7.** `GetMessageW` returning -1 means error. On -1: break the loop and exit cleanly.
Do not panic — panics in the main loop with `panic = "abort"` terminate the process.

**R8.** `TranslateMessage` is called on every message — required for correct keyboard
message translation even though no edit controls exist.

**R9.** The loop must never block between calls to `GetMessageW`. No `sleep`,
no synchronous I/O, no `Mutex` waits on the main thread outside of `wnd_proc`.

---

## Shutdown

**R10.** `PostQuitMessage(0)` is called from `wnd_proc` on `WM_DESTROY` and `WM_ENDSESSION`.
This posts `WM_QUIT`, causing `GetMessageW` to return 0 and exit the loop.

**R11.** After the loop exits, `main` returns. No explicit cleanup needed here —
hook uninstall and tray destroy happen in `wnd_proc` before `PostQuitMessage`.

---

## Dependencies

- `app::wnd_proc` — registered as the window procedure
- `hooks::APP_HWND` — stored after window creation, before `install()`
- `hooks::install()` — called after `APP_HWND` is stored

---

## Edge Cases

- **Spurious `GetMessageW` -1:** Can occur on malformed messages. Log (if debug build)
  and exit cleanly rather than loop infinitely or panic.
- **WM_DESTROY before WM_QUIT:** `WM_DESTROY` is dispatched via `DispatchMessageW`,
  calls `PostQuitMessage(0)`, which causes next `GetMessageW` to return 0. Correct flow.
- **Message-only window and WM_QUERYENDSESSION:** Verify in research whether
  `HWND_MESSAGE` windows receive session-end messages. See research file.
