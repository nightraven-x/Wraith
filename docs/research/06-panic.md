# Research 06 — Panic Unlock

Verified facts for WM_TIMER panic handler in `src/app.rs`.

---

## GetAsyncKeyState

```rust
// Signature in windows-sys 0.59:
fn GetAsyncKeyState(vkey: i32) -> i16

// Check if key is held:
let held = GetAsyncKeyState(panic_vk as i32) & (0x8000u16 as i16) != 0;
// Equivalent: result < 0 (bit 15 / sign bit = held)
```

- Bit 15 (MSB / sign bit) set → key currently held → negative i16 value
- Bit 0 (LSB) → pressed since last call — unreliable, don't use for hold detection
- MSDN LowLevelKeyboardProc confirms: callback is called BEFORE app can query GetAsyncKeyState → hook blocking does not interfere with hardware state reading

## GetTickCount

```rust
// windows-sys 0.59:
fn GetTickCount() -> u32
```

- Returns u32 milliseconds since system boot
- Wraps at ~49.7 days (2^32 ms)
- Use `wrapping_sub` for correct elapsed time across rollover:
  ```rust
  let elapsed = GetTickCount().wrapping_sub(PANIC_START.load(Relaxed));
  // Correct even if GetTickCount wrapped between start and now
  ```
- `GetTickCount64() -> u64` avoids wraparound but unnecessary for 3-second timer

## Panic Timer Logic

```rust
const PANIC_HOLD_MS: u32 = 3000;

// In WM_TIMER handler (100ms interval):
let held = GetAsyncKeyState(config.panic_vk as i32) & (0x8000u16 as i16) != 0;
if held {
    let start = PANIC_START.load(Relaxed);
    if start == 0 {
        PANIC_START.store(GetTickCount(), Relaxed);
    } else if GetTickCount().wrapping_sub(start) >= PANIC_HOLD_MS {
        app::unlock(hwnd);
    }
} else {
    PANIC_START.store(0, Relaxed);
}
```

- `PANIC_START = 0` is the sentinel for "not started"
- Reset to 0 when key released — prevents false triggers from previous hold sessions
- `GetTickCount()` returning 0 at boot would be a false "not started" — acceptable edge case (0ms after boot)

## SetTimer Accuracy

- Default system timer resolution: ~15.6ms (64 ticks/sec)
- `SetTimer(hwnd, id, 100, None)` fires at next 15.6ms boundary after 100ms
- Actual interval: 100-116ms — acceptable for 3-second hold detection
- `WM_TIMER` is low-priority; coalesces if message pump is busy — the 3-second threshold is generous enough to absorb delays
- `SetThreadExecutionState` (sleep prevention) does not affect timer delivery

## PANIC_START AtomicU32

```rust
pub static PANIC_START: AtomicU32 = AtomicU32::new(0);
```

- Set by WM_TIMER handler on main thread (same thread as WndProc)
- Not accessed from hook callbacks or other threads — but AtomicU32 is correct by design
- `Relaxed` ordering sufficient — no cross-thread synchronization needed
