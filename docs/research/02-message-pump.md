# Research 02 — Message Window + Pump

Verified facts for `src/main.rs` GetMessageW loop and HWND_MESSAGE window.

---

## CRITICAL: WM_ENDSESSION on HWND_MESSAGE Windows

**HWND_MESSAGE windows do NOT receive WM_QUERYENDSESSION or WM_ENDSESSION.**

Source: Raymond Chen (devblogs.microsoft.com/oldnewthing/20171218-00/?p=97595):
> "Many window messages are sent to all top-level windows... These messages don't reach message-only windows."
> Listed explicitly: WM_QUERYENDSESSION, WM_SETTINGCHANGE, etc.

Message-only windows are internally children of the HWND_MESSAGE parent, so they are NOT top-level windows and do not receive broadcast/shutdown messages.

**Impact:** The spec's `WM_ENDSESSION` handler in `wnd_proc` will never fire. Hooks will be silently removed by the OS at process termination anyway (OS cleans up all hooks on process exit). This is acceptable — do not attempt `WM_ENDSESSION` handling via HWND_MESSAGE.

## CRITICAL: TaskbarCreated on HWND_MESSAGE Windows

**HWND_MESSAGE windows do NOT receive the `TaskbarCreated` broadcast.**

Source: Shell taskbar doc + fetch agent 20:
> "When the taskbar is created, it registers a message with the TaskbarCreated string and then broadcasts this message to all top-level windows."

Message-only windows are NOT top-level → TaskbarCreated is never delivered.

**Impact:** The tray recovery mechanism (re-add icon after Explorer restart) cannot use HWND_MESSAGE directly. Either:
1. Accept that icon won't recover after Explorer crash (simplest)
2. Use a real (but hidden) top-level window instead of HWND_MESSAGE
3. Store the registered message ID and handle it anyway (if it somehow arrives — it won't)

**Recommendation:** For v1.0, accept no recovery. Document as known limitation.

## HWND_MESSAGE Window Properties

- Invisible — no position, no visible rendering
- NOT enumerated by `EnumWindows` or `EnumChildWindows`
- Receives `PostMessageW` normally — any posted message arrives and is dispatched
- Receives `WM_DESTROY` when `DestroyWindow` is called on it (not a broadcast)
- Creates via: `CreateWindowExW(0, class_name, null, 0, 0, 0, 0, 0, HWND_MESSAGE, null, hinstance, null)`

## GetMessageW Return Values

```rust
loop {
    let ret = GetMessageW(&mut msg, hwnd, 0, 0);
    if ret == 0 { break; }           // WM_QUIT received — normal exit
    if ret == -1 { break; }          // Error — log and exit
    TranslateMessage(&msg);
    DispatchMessageW(&msg);
}
```
- `0` → WM_QUIT, exit loop
- `-1` → error (`GetLastError()` for details), exit loop
- `> 0` → message retrieved, process normally

## TranslateMessage

Safe to call unconditionally on HWND_MESSAGE window. No keyboard focus → no WM_CHAR messages generated. No side effects for a message-only window.

## Hook Pump Requirement

- LL hooks (`WH_KEYBOARD_LL`, `WH_MOUSE_LL`) are driven by the installing thread's `GetMessageW` loop
- If the loop blocks for >1000ms (Win10 1709+), the hook is silently removed with no notification
- Nothing else can drive the hook pump — `MsgWaitForMultipleObjects` is an alternative but unnecessary complexity
- The main thread must NEVER block — no I/O, no mutex waits, no Sleep()

## WNDCLASSEXW Required Fields

```rust
WNDCLASSEXW {
    cbSize:        size_of::<WNDCLASSEXW>() as u32,  // required
    lpfnWndProc:   Some(wnd_proc),                    // required
    hInstance:     GetModuleHandleW(null()),           // required
    lpszClassName: class_name.as_ptr(),               // required
    // all other fields: 0/null — OK for message-only window
}
```
- `hInstance`: use `GetModuleHandleW(null())` for the current process
- `hIcon`, `hCursor`, `hbrBackground`: 0/null — fine for invisible message-only window

## WM_DESTROY Sequence

1. External code calls `PostMessageW(hwnd, WM_CLOSE, 0, 0)` or `DestroyWindow(hwnd)`
2. WndProc receives `WM_DESTROY`
3. WndProc calls `hooks::uninstall()`, `tray.destroy()`, `PostQuitMessage(0)`
4. Next `GetMessageW` call returns 0 → loop exits

Note: `WM_CLOSE` is NOT automatically turned into `WM_DESTROY` — must call `DestroyWindow` in the WM_CLOSE handler, or post WM_DESTROY directly. For graceful exit from code, call `PostMessageW(hwnd, WM_DESTROY, 0, 0)` directly.
