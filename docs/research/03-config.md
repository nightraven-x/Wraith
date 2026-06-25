# Research 03 — Configuration (INI)

Verified facts for `src/config.rs`.

---

## GetPrivateProfileIntW

```rust
// Returns nDefault when:
// - File does not exist at lpFileName
// - Section does not exist
// - Key does not exist
// - Value is present but not a parseable integer (returns 0, NOT nDefault for non-integer)
// No GetLastError needed — always returns a value
GetPrivateProfileIntW(section, key, default, path)
```

- Missing file → returns `nDefault` silently
- Missing key → returns `nDefault` silently
- Non-integer value → returns `0` (NOT nDefault — MSDN: "If the key cannot be found, the nDefault parameter is returned")
- Case-insensitive for both section and key names (Win32 INI API is case-insensitive)
- Works on read-only files — it only reads
- Works with any file extension (not just `.ini`)
- Can return negative values (returned as `i32`)

## GetModuleFileNameW

```rust
let mut buf = vec![0u16; MAX_PATH as usize];
let len = GetModuleFileNameW(null_mut(), buf.as_mut_ptr(), MAX_PATH);
// len = number of chars written (NOT including null terminator) on success
// len = 0 on failure
// MAX_PATH = 260 (sufficient for most paths; may truncate for very long paths)
```

- `GetModuleHandleW(null())` or `null_mut()` as first param → current process exe
- Returns char count (excl. null) on success, 0 on failure
- To extract directory: find last occurrence of `0x005C` (`\`) in the buffer, truncate there
- `MAX_PATH = 260` — in windows-sys under `Win32_Foundation`

## INI Path Construction

```rust
let mut path = vec![0u16; MAX_PATH as usize];
let len = GetModuleFileNameW(null_mut(), path.as_mut_ptr(), MAX_PATH) as usize;
// Find last backslash
if let Some(pos) = path[..len].iter().rposition(|&c| c == 0x005C) {
    path.truncate(pos + 1);
}
// Append "wraith.ini\0"
for c in "wraith.ini\0".encode_utf16() { path.push(c); }
```

## OnceLock

- `std::sync::OnceLock` stabilized in Rust 1.70 (released 2023-06-01)
- `windows-sys 0.59` MSRV not explicitly verified but is likely 1.60 or lower — no conflict
- `OnceLock::get_or_init` panics on recursive call — safe in Wraith's single-threaded init
- `OnceLock::get()` returns `None` before init → `Config::get()` using `.unwrap()` is safe IF `Config::load()` is called in init before any hook or WndProc code

## Config Struct Defaults

```ini
[Wraith]
LockModifiers=7    ; MOD_CTRL|MOD_SHIFT|MOD_ALT = 2|4|1
LockKey=76         ; 'L'
UnlockModifiers=7
UnlockKey=85       ; 'U'
PanicKey=27        ; VK_ESCAPE
LockOnStart=0
```

- INI is read-only at runtime — no `WritePrivateProfileStringW` needed
- Key combos editable by user via text editor + restart
