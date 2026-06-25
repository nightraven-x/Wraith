# Wraith — Specification Index

Each part has a companion research file in `docs/research/` that must be completed
before implementing that part. Research files list specific MSDN behaviors and
version-dependent details to verify — nothing is assumed.

## Parts

| # | Module | Spec | Research | Step |
|---|--------|------|----------|------|
| 01 | Hook Architecture | [spec](01-hooks.md) | [research](../research/01-hooks.md) | Step 4 |
| 02 | Message Window + Pump | [spec](02-message-pump.md) | [research](../research/02-message-pump.md) | Step 1 |
| 03 | Configuration (INI) | [spec](03-config.md) | [research](../research/03-config.md) | Step 2 |
| 04 | System Tray | [spec](04-tray.md) | [research](../research/04-tray.md) | Step 3 |
| 05 | Lock / Unlock State | [spec](05-lock-unlock.md) | [research](../research/05-lock-unlock.md) | Step 5 |
| 06 | Panic Unlock | [spec](06-panic.md) | [research](../research/06-panic.md) | Step 6 |
| 07 | Single-Instance Mutex | [spec](07-single-instance.md) | [research](../research/07-single-instance.md) | Step 1 |
| 08 | Autostart (Registry) | [spec](08-autostart.md) | [research](../research/08-autostart.md) | Step 7 |
| 09 | Update Checker | [spec](09-updater.md) | [research](../research/09-updater.md) | Step 8 |
| 10 | Resource Embedding | [spec](10-resources.md) | [research](../research/10-resources.md) | Step 9 |
| 11 | NSIS Installer | [spec](11-installer.md) | [research](../research/11-installer.md) | Step 9 |
| 12 | CI / GitHub Actions | [spec](12-ci.md) | [research](../research/12-ci.md) | All |

## Build Order

Implement in spec order: 07 → 02 → 03 → 04 → 01 → 05 → 06 → 08 → 09 → 10 → 11 → 12.
Each step must be independently testable before proceeding to the next.

## Global Constraints

Constraints that apply to every spec part — do not repeat in individual files,
but treat as always active:

- `nCode < 0` in any hook proc: call `CallNextHookEx` and return immediately.
- No `SendMessageW` from hook callbacks or background threads. `PostMessageW` only.
- No `Mutex` in hook callbacks. Atomics only.
- No blocking I/O, no allocations in hook callbacks.
- `panic = "abort"` in release profile. Never change.
- `windows-sys` only. Never `windows` crate.
- Wide strings via `s.encode_utf16().chain(once(0)).collect::<Vec<u16>>()`.
- `WM_COMMAND` dispatch: always `LOWORD(wp)`, never raw `wp`.
- `lock()`/`unlock()`/`toggle()` always take `hwnd: HWND` as first parameter.
- `SetThreadExecutionState` called from main thread only.
