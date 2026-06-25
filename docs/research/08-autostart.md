# Research 08 — Autostart (Registry)

Verified facts for `set_autostart` / `is_autostart` in `src/app.rs`.

---

## HKCU Run Key

```
HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run
```

- Values executed at user login (per-user, not system startup)
- Missing path → silently skipped (no error shown to user)
- Key always exists on Windows — no need to create it
- `HKEY_CURRENT_USER = 0x80000001` in windows-sys

## RegSetValueExW for REG_SZ

```rust
// cbData MUST include null terminator
// For wide strings: (len + 1) * 2 bytes
let path_quoted = format!("\"{}\"", exe_path);
let wide: Vec<u16> = path_quoted.encode_utf16().chain(once(0)).collect();
RegSetValueExW(
    key,
    value_name.as_ptr(),
    0,
    REG_SZ,
    wide.as_ptr() as *const u8,
    (wide.len() * 2) as u32,  // includes null terminator
);
```

MSDN: "If the data is of type REG_SZ, REG_EXPAND_SZ, or REG_MULTI_SZ, cbData must include the size of the terminating null character or characters."

## Path Quoting Requirement

Paths with spaces MUST be quoted in Run key values:
- Unquoted: `C:\Program Files\Wraith\wraith.exe` → Windows tries to exec `C:\Program` with args `Files\Wraith\wraith.exe`
- Correct: `"C:\Program Files\Wraith\wraith.exe"`

## Access Rights

```rust
// For set_autostart(true/false):
KEY_SET_VALUE = 0x0002
// For is_autostart():
KEY_QUERY_VALUE = 0x0001
// Minimum required — do not use KEY_ALL_ACCESS
```

## RegDeleteValueW

- Returns `ERROR_FILE_NOT_FOUND (2)` if value doesn't exist — safe to ignore this error
- If value was never set (autostart was never enabled), this returns error → treat as success

## is_autostart() Implementation

```rust
// Use RegQueryValueExW with all NULL output parameters to check existence:
RegQueryValueExW(key, value_name, null_mut(), null_mut(), null_mut(), null_mut())
// Returns ERROR_SUCCESS if value exists
// Alternative: RegGetValueW with RRF_RT_REG_SZ — also available in windows-sys
```

## windows-sys Feature and Constants

Feature: `Win32_System_Registry`

Functions available:
- `RegOpenKeyExW`
- `RegSetValueExW`
- `RegQueryValueExW`
- `RegDeleteValueW`
- `RegCloseKey`
- `RegGetValueW` (alternative for is_autostart)

```rust
REG_SZ = 1
KEY_SET_VALUE   = 0x0002
KEY_QUERY_VALUE = 0x0001
```
