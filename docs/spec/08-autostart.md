# Spec 08 — Autostart (Registry)

> Research prerequisite: [../research/08-autostart.md](../research/08-autostart.md)
> Implements: `src/app.rs` (set_autostart, is_autostart) — Step 7

---

## Purpose

Allow Wraith to launch automatically on Windows login by writing its exe path to
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. No admin rights required —
`HKCU` is always writable by the current user.

---

## Public Interface

```rust
pub fn set_autostart(enable: bool);
// Enable: writes exe path to registry Run key.
// Disable: deletes the value from registry Run key.

pub fn is_autostart() -> bool;
// Returns true if the "Wraith" value exists in the Run key.
```

---

## Registry Key

```
HKCU\Software\Microsoft\Windows\CurrentVersion\Run
Value name: "Wraith"
Value type: REG_SZ
Value data: full quoted path to wraith.exe, e.g. "C:\Program Files\Wraith\wraith.exe"
```

---

## Behavioral Requirements

### set_autostart(true) — Enable

**R1.** Get the exe path via `GetModuleFileNameW(NULL, buf, MAX_PATH)`.

**R2.** Quote the path: wrap in double quotes → `"\"C:\\path\\wraith.exe\""`.
Required if the path contains spaces (e.g. Program Files). Always quote for safety.

**R3.** Open the key:
```
RegOpenKeyExW(HKEY_CURRENT_USER,
    L"Software\\Microsoft\\Windows\\CurrentVersion\\Run",
    0, KEY_SET_VALUE, &mut hkey)
```
On failure: silently return (non-fatal, feature simply doesn't work).

**R4.** Write the value:
```
RegSetValueExW(hkey, L"Wraith", 0, REG_SZ,
    path_wide.as_ptr() as *const u8,
    (path_wide.len() * 2) as u32)   // byte count including null terminator
```

**R5.** `RegCloseKey(hkey)`.

### set_autostart(false) — Disable

**R6.** Open key with `KEY_SET_VALUE`.

**R7.** `RegDeleteValueW(hkey, L"Wraith")`. On `ERROR_FILE_NOT_FOUND`: ignore (already
absent). Other errors: silently ignore.

**R8.** `RegCloseKey(hkey)`.

### is_autostart()

**R9.** Open key with `KEY_QUERY_VALUE`.

**R10.** Call `RegQueryValueExW(hkey, L"Wraith", NULL, NULL, NULL, NULL)`.
Returns `ERROR_SUCCESS` if value exists, `ERROR_FILE_NOT_FOUND` if absent.

**R11.** `RegCloseKey(hkey)`. Return `true` if `ERROR_SUCCESS`.

---

## Tray Menu Integration

**R12.** `show_menu()` calls `is_autostart()` to determine whether to check the
`ID_AUTOSTART` menu item.

**R13.** On `WM_COMMAND / ID_AUTOSTART`: call `set_autostart(!is_autostart())`.
This toggles the state each time the menu item is selected.

---

## Dependencies

- `app.rs` — `wnd_proc` dispatches `ID_AUTOSTART`
- `tray.rs` — `show_menu()` queries `is_autostart()`

---

## Edge Cases

- **Path with spaces:** Always quote the path (R2). Unquoted paths with spaces in
  the Run key are silently treated as a path to the first word — a silent failure.
- **Moved exe after autostart enabled:** The registry value points to the old path.
  Wraith won't launch at next login. Acceptable — user must re-enable autostart.
- **RegSetValueExW byte count:** `REG_SZ` requires byte count, not character count.
  For UTF-16: `len * 2`. Include the null terminator in the count. Verify in research.
- **Multiple user sessions:** `HKCU` is per-user. Each user's autostart is independent.
