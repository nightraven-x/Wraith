# Spec 01 — Hook Architecture

> Research prerequisite: [../research/01-hooks.md](../research/01-hooks.md)
> Implements: `src/hooks.rs` — Step 4

---

## Purpose

Install and manage two Windows low-level hooks (`WH_KEYBOARD_LL`, `WH_MOUSE_LL`) that
intercept every input event system-wide and decide whether to pass it through or block it.

---

## Public Interface

```rust
// Global atomics — readable from anywhere without locking
pub static LOCKED:      AtomicBool  = AtomicBool::new(false);
pub static KB_HOOK:     AtomicUsize = AtomicUsize::new(0);  // HHOOK as usize, 0 = not installed
pub static MOUSE_HOOK:  AtomicUsize = AtomicUsize::new(0);
pub static APP_HWND:    AtomicUsize = AtomicUsize::new(0);  // set by main.rs before install()
pub static PANIC_START: AtomicU32   = AtomicU32::new(0);    // used by panic timer in app.rs

pub fn install(hwnd: HWND) -> Result<(), &'static str>;
// Err("Failed to install keyboard hook") | Err("Failed to install mouse hook")
// Caller must MessageBoxW + ExitProcess(1) on Err.

pub fn uninstall();
// Calls UnhookWindowsHookEx for both hooks if installed (non-zero).
// Safe to call multiple times (no-op if already uninstalled).

// keyboard_proc and mouse_proc: private extern "system" fn, not exported.
```

---

## Behavioral Requirements

### install()

**R1.** Store `hwnd as usize` in `APP_HWND` with `Relaxed` ordering before calling
`SetWindowsHookExW`.

**R2.** Call `SetWindowsHookExW(WH_KEYBOARD_LL, keyboard_proc, NULL, 0)`.
On null return: return `Err("Failed to install keyboard hook")`.

**R3.** Call `SetWindowsHookExW(WH_MOUSE_LL, mouse_proc, NULL, 0)`.
On null return: uninstall the keyboard hook already installed, then return
`Err("Failed to install mouse hook")`.

**R4.** Store both HHOOK handles as `usize` in `KB_HOOK` / `MOUSE_HOOK` with `Relaxed`.

### uninstall()

**R5.** Load `KB_HOOK` / `MOUSE_HOOK`. If non-zero: call `UnhookWindowsHookEx`,
store 0 back. Do both unconditionally (don't short-circuit on first failure).

### keyboard_proc

Signature: `unsafe extern "system" fn keyboard_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT`

**R6.** If `n_code < 0`: call `CallNextHookEx(0, n_code, w_param, l_param)` and return
its value. No further processing.

**R7.** Cast `l_param` to `*const KBDLLHOOKSTRUCT`. Read `flags` and `vk_code`.

**R8.** If `flags & LLKHF_INJECTED (0x10) != 0`: call `CallNextHookEx` and return.
Synthetic input always passes through, regardless of lock state.

**R9.** Check lock combo: if modifier state matches `Config::get().lock_mods` AND
`vk_code == Config::get().lock_vk` AND `w_param == WM_KEYDOWN || WM_SYSKEYDOWN`:
call `PostMessageW(APP_HWND, WM_COMMAND, MAKEWPARAM(ID_LOCK, 0), 0)` and return 1
(consume the event, do not call `CallNextHookEx`).

**R10.** Check unlock combo: same pattern with `unlock_mods` / `unlock_vk` / `ID_UNLOCK`.
Return 1 to consume.

**R11.** Modifier state detection: use `GetAsyncKeyState` for each modifier bit in
`lock_mods` / `unlock_mods`. `GetAsyncKeyState(vk) & 0x8000u16 as i16 != 0` = held.
Map `MOD_ALT=0x1 → VK_MENU`, `MOD_CONTROL=0x2 → VK_CONTROL`,
`MOD_SHIFT=0x4 → VK_SHIFT`, `MOD_WIN=0x8 → VK_LWIN || VK_RWIN`.

**R12.** If `LOCKED.load(Relaxed) == true`: return 1 (block). Do NOT call `CallNextHookEx`.

**R13.** Default: call `CallNextHookEx(0, n_code, w_param, l_param)` and return its value.

### mouse_proc

Signature: `unsafe extern "system" fn mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT`

**R14.** If `n_code < 0`: call `CallNextHookEx` and return. No further processing.

**R15.** Cast `l_param` to `*const MSLLHOOKSTRUCT`. Read `flags`.

**R16.** If `flags & LLMHF_INJECTED (0x01) != 0`: call `CallNextHookEx` and return.

**R17.** If `LOCKED.load(Relaxed) == true`: return 1 (block ALL mouse events —
moves, clicks, scroll, wheel). Do NOT call `CallNextHookEx`.

**R18.** Default: call `CallNextHookEx` and return.

---

## Ordering of Checks

```
keyboard_proc
  1. nCode < 0?          → CallNextHookEx + return
  2. LLKHF_INJECTED set? → CallNextHookEx + return
  3. Lock combo match?   → PostMessage(ID_LOCK) + return 1
  4. Unlock combo match? → PostMessage(ID_UNLOCK) + return 1
  5. LOCKED == true?     → return 1
  6. default             → CallNextHookEx + return

mouse_proc
  1. nCode < 0?          → CallNextHookEx + return
  2. LLMHF_INJECTED set? → CallNextHookEx + return
  3. LOCKED == true?     → return 1
  4. default             → CallNextHookEx + return
```

---

## Constraints

- Callbacks run on the main thread. No cross-thread data access except reading atomics.
- No allocations, no I/O, no blocking calls inside either proc.
- `PostMessageW` is the only side-effect allowed besides atomic reads and `CallNextHookEx`.
- `GetAsyncKeyState` is safe to call in the hook: it reads hardware state directly,
  works even when the hook is blocking events.
- Atomics use `Relaxed` ordering throughout: all reads/writes are on the main thread,
  and the updater thread only writes `APP_HWND` before hooks are installed.

---

## Dependencies

- `config.rs` — `Config::get()` for combo VKs and mod masks. Called at proc entry.
- `app.rs` — `APP_HWND` for `PostMessageW` target.
- `main.rs` — calls `install()` after `APP_HWND` is stored.

---

## Edge Cases

- **Double lock/unlock posts:** if user holds the combo, `WM_KEYDOWN` fires repeatedly.
  WndProc's `lock()`/`unlock()` must be idempotent (no-op if already in target state).
- **Partial combo:** Ctrl held but not Shift+Alt — does not match. `GetAsyncKeyState`
  checks each modifier individually.
- **Hook removal on timeout:** if the callback exceeds ~200ms Windows silently
  removes the hook. No notification. The message pump must never block.
- **`hmod = NULL`:** for `dwThreadId = 0` (system-wide), `hmod` must be NULL per MSDN.
  Do not pass `GetModuleHandleW(NULL)` here.
