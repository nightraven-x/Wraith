# Spec 03 — Configuration (INI)

> Research prerequisite: [../research/03-config.md](../research/03-config.md)
> Implements: `src/config.rs` — Step 2

---

## Purpose

Load `wraith.ini` once at startup, expose it as a global immutable `Config` via
`OnceLock`. The INI is read-only at runtime — no write-back. Edit the file and
restart to change config.

---

## Public Interface

```rust
pub struct Config {
    pub lock_mods:   u32,   // MOD_* bitmask for lock hotkey modifiers
    pub lock_vk:     u32,   // virtual key code for lock hotkey
    pub unlock_mods: u32,   // MOD_* bitmask for unlock hotkey modifiers
    pub unlock_vk:   u32,   // virtual key code for unlock hotkey
    pub panic_vk:    u32,   // virtual key for panic hold-unlock
    pub lock_on_start: bool,
}

impl Config {
    pub fn load() -> Self;
    // Reads wraith.ini relative to the exe. Missing file or missing key → default.
    // Must be called exactly once, before Config::get().

    pub fn get() -> &'static Self;
    // Returns the OnceLock value. Panics if load() was not called first.
}
```

---

## Defaults

| Field | Default | INI Key | Notes |
|-------|---------|---------|-------|
| `lock_mods` | 7 (Ctrl+Shift+Alt) | `LockModifiers` | MOD_ALT=1, MOD_CONTROL=2, MOD_SHIFT=4 |
| `lock_vk` | 76 (0x4C = 'L') | `LockKey` | |
| `unlock_mods` | 7 | `UnlockModifiers` | |
| `unlock_vk` | 85 (0x55 = 'U') | `UnlockKey` | |
| `panic_vk` | 27 (0x1B = VK_ESCAPE) | `PanicKey` | |
| `lock_on_start` | false (0) | `LockOnStart` | 0=false, nonzero=true |

---

## Behavioral Requirements

### INI Path Resolution

**R1.** Call `GetModuleFileNameW(NULL, buf, MAX_PATH)` to get the full exe path.

**R2.** Strip the filename component to get the directory. Append `\wraith.ini`.

**R3.** Pass the resulting wide string as `lpFileName` to all `GetPrivateProfileIntW`
calls. Use an absolute path — do not rely on the working directory.

### Loading

**R4.** For each field, call `GetPrivateProfileIntW(L"Wraith", key, default, path)`.
The function returns `nDefault` if the file does not exist or the key is absent.
No error handling needed for missing file/key.

**R5.** `lock_on_start`: call `GetPrivateProfileIntW` and treat 0 as false, nonzero as true.

**R6.** Store the result in a `static ONCE: OnceLock<Config>`. `Config::load()` calls
`ONCE.get_or_init(|| { ... })`. `Config::get()` calls `ONCE.get().unwrap()`.

### Validation

**R7.** No validation of VK codes or mod masks at load time. Invalid values result in
a combo that never fires — acceptable failure mode.

---

## INI File Format

```ini
; Modifier bitmask: MOD_ALT=1, MOD_CONTROL=2, MOD_SHIFT=4, MOD_WIN=8
; Ctrl+Shift+Alt = 7
[Wraith]
LockModifiers=7
LockKey=76
UnlockModifiers=7
UnlockKey=85
PanicKey=27
LockOnStart=0
```

The INI ships alongside the exe. If absent, all defaults apply.

---

## Dependencies

- `main.rs` — calls `Config::load()` as step 2 of init.
- `hooks.rs` — calls `Config::get()` on every keypress to read combo VKs.

---

## Edge Cases

- **Program Files install (no write access):** INI is read-only. `WritePrivateProfileStringW`
  is not used anywhere. Read access to Program Files is always allowed.
- **Path longer than MAX_PATH:** Out of scope. Exe paths in practice are well under MAX_PATH.
- **Multiple calls to load():** `OnceLock::get_or_init` makes subsequent calls no-ops.
  Safe but the init sequence must call it exactly once.
- **INI in a different directory:** Not supported. INI must be alongside the exe.
