# Research 01 — Hook Architecture

Verified facts for `src/hooks.rs`. Sources: MSDN + windows-rs 0.59.0 source.

---

## nCode Values

- `HC_ACTION = 0` — only nCode value for which the hook should process the event
- `nCode < 0` — MUST call `CallNextHookEx` and return its result without processing
- No other positive nCode values exist for WH_KEYBOARD_LL or WH_MOUSE_LL

## Blocking Events

- Return ANY nonzero value to block (consume) the event — value `1` is conventional
- Do NOT call `CallNextHookEx` when blocking
- MSDN: "it may return a nonzero value to prevent the system from passing the message to the rest of the hook chain or the target window procedure"

## Injected Input Flags

### Keyboard (KBDLLHOOKSTRUCT.flags)

- `LLKHF_INJECTED = 0x00000010` (bit 4) — event injected from ANY process
- `LLKHF_LOWER_IL_INJECTED = 0x00000002` (bit 1) — injected from lower integrity level
- **Bit 4 is ALWAYS set when bit 1 is set** (explicit MSDN note: "Note that bit 4 is also set whenever bit 1 is set")
- Testing bit 4 alone catches ALL injections (SendInput, AHK Send, Chrome DevTools, Parsec, PowerShell)
- Check: `(*kb).flags & 0x10 != 0`

### Mouse (MSLLHOOKSTRUCT.flags)

- `LLMHF_INJECTED = 0x00000001` (bit 0) — event injected from any process
- `LLMHF_LOWER_IL_INJECTED = 0x00000002` (bit 1) — lower-IL injection
- MSDN states: "Testing LLMHF_INJECTED (bit 0) will tell you whether the event was injected. If it was, then testing LLMHF_LOWER_IL_INJECTED (bit 1) will tell you whether the event was injected from a process running at lower integrity level."
- Testing bit 0 alone is sufficient to pass all synthetic mouse input
- Check: `(*ms).flags & 0x01 != 0`

## Struct Field Names (windows-sys 0.59)

### KBDLLHOOKSTRUCT (exactly 5 fields)
```rust
pub vkCode: u32
pub scanCode: u32
pub flags: KBDLLHOOKSTRUCT_FLAGS  // type alias: pub type KBDLLHOOKSTRUCT_FLAGS = u32
pub time: u32
pub dwExtraInfo: usize
```
- `KBDLLHOOKSTRUCT_FLAGS` is a **transparent type alias** (`= u32`), NOT a newtype
- Standard u32 bitwise ops work directly: `flags & 0x10 != 0` compiles without any cast

### MSLLHOOKSTRUCT (exactly 5 fields)
```rust
pub pt: POINT
pub mouseData: u32
pub flags: u32  // plain u32, not a named alias
pub time: u32
pub dwExtraInfo: usize
```

## SetWindowsHookExW Signature (windows-sys 0.59)

```rust
pub unsafe extern "system" fn SetWindowsHookExW(
    idhook: WINDOWS_HOOK_ID,
    lpfn: HOOKPROC,
    hmod: HINSTANCE,    // *mut c_void, NOT Option — pass null_mut() for LL hooks
    dwthreadid: u32,    // 0 for global LL hooks
) -> HHOOK              // bare HHOOK (not Result), returns 0/null on failure
```

- `HOOKPROC = Option<unsafe extern "system" fn(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT>`
- Pass `Some(my_hook_fn)` — no raw pointer cast needed
- `WH_KEYBOARD_LL = 13`, `WH_MOUSE_LL = 14` — both "Global only" (dwThreadId must be 0)
- `hmod = NULL` is CORRECT for LL hooks — they do NOT inject into other processes; context switches back to installer

## Hook Thread Affinity

- Callbacks called ONLY on the thread that called `SetWindowsHookExW`
- That thread MUST run a `GetMessageW` loop
- MSDN: "This hook is called in the context of the thread that installed it. The call is made by sending a message to the thread that installed the hook. Therefore, the thread that installed the hook must have a message loop."
- If the loop blocks: Win7+ → hook silently removed without notification; Win10 1709+ → hard cap of 1000ms regardless of registry setting
- **No way to detect silent hook removal** (MSDN explicit)
- Registry timeout: `HKCU\Control Panel\Desktop\LowLevelHooksTimeout` (NOT HKLM as documented elsewhere)
- Keep callbacks under 200ms to be safe (well within 1000ms cap)

## GetAsyncKeyState

```rust
// Windows API: SHORT GetAsyncKeyState(int vKey)
// windows-sys: fn GetAsyncKeyState(vkey: i32) -> i16
```
- Bit 15 (MSB / sign bit) set → key currently held down → negative return value indicates held
- Check: `GetAsyncKeyState(vk) & (0x8000u16 as i16) != 0`
- Bit 0 (LSB) → pressed since last call — **unreliable under preemptive multitasking**
- MSDN LowLevelKeyboardProc notes: "When this callback function is called in response to a change in the state of a key, the callback function is called before the application can use GetAsyncKeyState" — confirms it reads hardware state independent of hook blocking

## VK Codes and MOD Constants

```rust
VK_CONTROL = 0x11
VK_SHIFT   = 0x10
VK_MENU    = 0x12  // Alt
VK_LWIN    = 0x5B
VK_RWIN    = 0x5C
VK_ESCAPE  = 0x1B

MOD_ALT     = 0x0001
MOD_CONTROL = 0x0002
MOD_SHIFT   = 0x0004
MOD_WIN     = 0x0008
```
- No combined VK for "either Win key" — must check VK_LWIN OR VK_RWIN separately with GetAsyncKeyState

## Modifier Detection in Hook Callback

Detect modifier combo via GetAsyncKeyState (not via wParam modifiers):
```rust
let ctrl  = GetAsyncKeyState(VK_CONTROL as i32) & (0x8000u16 as i16) != 0;
let shift = GetAsyncKeyState(VK_SHIFT as i32)   & (0x8000u16 as i16) != 0;
let alt   = GetAsyncKeyState(VK_MENU as i32)    & (0x8000u16 as i16) != 0;
```

## CallNextHookEx

- First parameter (hhk) is **ignored** for LL hooks on modern Windows — passing 0/null is safe
- `CallNextHookEx(0, n_code, w_param, l_param)` is the correct pattern
