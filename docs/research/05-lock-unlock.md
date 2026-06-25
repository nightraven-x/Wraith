# Research 05 — Lock / Unlock State

Verified facts for `src/app.rs` lock/unlock/wnd_proc.

---

## SetThreadExecutionState

```rust
// On lock:
SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
// On unlock:
SetThreadExecutionState(ES_CONTINUOUS);
```

Constants:
- `ES_SYSTEM_REQUIRED  = 0x00000001` — prevents system sleep
- `ES_DISPLAY_REQUIRED = 0x00000002` — prevents display-off
- `ES_CONTINUOUS       = 0x80000000` — makes the state persist until explicitly changed

Semantics:
- Each call REPLACES the previous state (not additive/stacking)
- `ES_CONTINUOUS` alone clears all flags → restores default sleep behavior
- Thread-scoped: the calling thread's execution state is set; this affects system-wide sleep (the system stays awake if any thread has SYSTEM_REQUIRED set)
- Return value: previous execution state flags — no error handling needed in practice
- Must be called from the main thread (via WndProc) — do not call from hook callbacks or background threads

## WM_ENDSESSION — NOT RECEIVED

**HWND_MESSAGE windows do NOT receive WM_ENDSESSION or WM_QUERYENDSESSION.**

See research 02. Do not implement a WM_ENDSESSION handler — it will never fire.
Cleanup on shutdown is handled by OS process termination (hooks auto-uninstall, icon auto-removed).

## GWLP_USERDATA Pattern

```rust
// Store TrayIcon pointer after creation:
let tray = Box::new(TrayIcon::new(hwnd));
let ptr = Box::into_raw(tray) as isize;
SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr);

// Retrieve in WndProc:
let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
if ptr == 0 { return DefWindowProcW(hwnd, msg, wp, lp); }
let tray = &mut *(ptr as *mut TrayIcon);
```

- `GWLP_USERDATA` works on HWND_MESSAGE windows — confirmed
- `GetWindowLongPtrW` returns 0 before `SetWindowLongPtrW` is called — safe to null-check
- `GWLP_USERDATA = -21i32` in windows-sys

## SetTimer / KillTimer

```rust
// Set: timer ID is second param (return value is the timer ID or 0 on failure)
SetTimer(hwnd, TIMER_PANIC, 100, None);

// Kill: safe no-op if timer not active (returns TRUE even if timer didn't exist)
KillTimer(hwnd, TIMER_PANIC);
```

- `WM_TIMER` CAN arrive after `KillTimer` if already in the queue — defensive LOCKED check handles this
- Timer IDs for window timers (non-NULL hwnd): any value > 0 is valid — `TIMER_PANIC = 2001` is fine
- `SetTimer(hwnd, id, 100, None)` resolution: system timer at ~15.6ms default → fires at next 15.6ms boundary after 100ms ≈ actual ~100-116ms

## WM_COMMAND Dispatch

```rust
// LOWORD of WPARAM = menu item command ID
// WPARAM in windows-sys 0.59 is usize
WM_COMMAND => {
    match (wp & 0xFFFF) as usize {
        ID_LOCK   => lock(hwnd),
        ID_UNLOCK => unlock(hwnd),
        // etc.
    }
}
```

- `wp as u16` is equivalent to `(wp & 0xFFFF) as u16` for positive IDs
- Menu item IDs (ID_LOCK=1001 etc.) fit in u16; safe to cast

## WndProc Message Handlers

```rust
WM_TRAY_MSG    => { /* LOWORD(lp) dispatch */ }
WM_COMMAND     => { /* (wp & 0xFFFF) dispatch */ }
WM_TIMER       => { if wp == TIMER_PANIC { /* panic check */ } }
WM_UPDATE_RESULT => { /* Box::from_raw(lp as *mut String) */ }
WM_DESTROY     => { hooks::uninstall(); tray.destroy(); PostQuitMessage(0); }
// WM_QUERYENDSESSION, WM_ENDSESSION: do NOT handle — never received on HWND_MESSAGE
_ => DefWindowProcW(hwnd, msg, wp, lp)
```

## TaskbarCreated Recovery

Not implementable via HWND_MESSAGE — see research 02 and 04.
