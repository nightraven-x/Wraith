# Spec 06 — Panic Unlock

> Research prerequisite: [../research/06-panic.md](../research/06-panic.md)
> Implements: WM_TIMER handler in `src/app.rs` — Step 6

---

## Purpose

Emergency unlock: hold the panic key (default: Esc) for 3 continuous seconds while
Wraith is locked. Works even though the hook is blocking the key, because
`GetAsyncKeyState` reads raw hardware state regardless of hook suppression.

---

## Mechanism

A 100ms repeating timer (`TIMER_PANIC`) is started on `lock()` and killed on `unlock()`.
Each timer tick checks whether the panic key is held. If held continuously for >= 3000ms,
`unlock()` is called.

---

## Global State

```rust
// In hooks.rs:
pub static PANIC_START: AtomicU32 = AtomicU32::new(0);
// 0 = panic key not held (or not started timing yet)
// nonzero = GetTickCount() value when hold started
```

---

## Behavioral Requirements

### Timer Setup

**R1.** In `lock(hwnd)`: call `SetTimer(hwnd, TIMER_PANIC, 100, NULL)`.
Returns a timer ID — if 0, `SetTimer` failed. On failure: log in debug builds,
continue (panic unlock won't work but lock still functions).

**R2.** In `unlock(hwnd)`: call `KillTimer(hwnd, TIMER_PANIC)`.
Call `PANIC_START.store(0, Relaxed)`.

### WM_TIMER / TIMER_PANIC Handler

**R3.** Only runs when `LOCKED.load(Relaxed) == true`. If somehow fired while
unlocked (race between KillTimer and the timer firing), check and return early.

**R4.** Read `GetAsyncKeyState(Config::get().panic_vk as i32)`.
The key is "held" if bit 15 of the return value is set:
```rust
let held = GetAsyncKeyState(panic_vk as i32) & (0x8000u16 as i16) != 0;
```

**R5.** If `held == false`:
- `PANIC_START.store(0, Relaxed)` (reset).
- Return.

**R6.** If `held == true` and `PANIC_START.load(Relaxed) == 0`:
- `PANIC_START.store(GetTickCount(), Relaxed)`.
- Return (start timing from this tick).

**R7.** If `held == true` and `PANIC_START.load(Relaxed) != 0`:
- `let elapsed = GetTickCount().wrapping_sub(PANIC_START.load(Relaxed))`.
- If `elapsed >= 3000`: call `unlock(hwnd)`.

### Precision Note

**R8.** At 100ms timer resolution, the actual unlock may fire up to 100ms late
(3000–3100ms hold). This is acceptable.

**R9.** `GetTickCount()` returns `u32` milliseconds since boot. Wraps at ~49.7 days.
`wrapping_sub` handles rollover correctly.

---

## Why GetAsyncKeyState

The hook returns 1 (blocking) for the panic key — so `WM_KEYDOWN` for that key
never reaches the message queue. `GetAsyncKeyState` bypasses the message queue
entirely and reads the hardware keyboard state directly. It reports the key as
held even when the hook is suppressing it.

---

## Dependencies

- `hooks.rs` — `LOCKED`, `PANIC_START`
- `config.rs` — `Config::get().panic_vk`
- `app.rs` — `unlock(hwnd)` called on timeout
- `app.rs` — `lock(hwnd)` / `unlock(hwnd)` start/stop the timer

---

## Edge Cases

- **Very fast lock/unlock cycle:** Timer may fire after `KillTimer` if the timer
  message was already queued. The `LOCKED` check in R3 handles this.
- **Panic key == lock or unlock key:** Undefined behavior — user's misconfiguration.
  No validation at load time (per spec 03). Consequence: holding the key may fire
  both panic unlock and the unlock combo. Acceptable.
- **GetTickCount rollover during hold:** `wrapping_sub` handles it correctly as long
  as the hold duration is < 49.7 days. Safe.
- **System sleep while locked:** `SetThreadExecutionState` prevents sleep while locked
  (per spec 05), so `GetTickCount` continuity is preserved during a panic hold.
