# Research 04 — System Tray

Verified facts for `src/tray.rs`.

---

## Shell_NotifyIconW Lifecycle

```
NIM_ADD → NIM_SETVERSION → (NIM_MODIFY as needed) → NIM_DELETE
```

- `NIM_SETVERSION` MUST be called every time after `NIM_ADD`
- NOT needed after `NIM_MODIFY`
- Setting is NOT persisted across logoff
- Without `NIM_SETVERSION`, `WM_CONTEXTMENU` and LOWORD/HIWORD lParam encoding are not available

## NOTIFYICONDATAW cbSize

```rust
cbSize: size_of::<NOTIFYICONDATAW>() as u32
```
Use `size_of` of the full struct — always correct regardless of OS version.

## NIM_SETVERSION

```rust
// uVersion field is inside the anonymous union in the struct
// In windows-sys 0.59, access via Anonymous.uVersion (or similar union accessor)
// Value: NOTIFYICON_VERSION_4 = 4
```

## WM_TRAY_MSG lParam (after NIM_SETVERSION with VERSION_4)

- `LOWORD(lParam)` = notification event (e.g. `WM_RBUTTONUP`, `WM_CONTEXTMENU`, `WM_LBUTTONDBLCLK`)
- `HIWORD(lParam)` = icon ID (restricted to 16 bits)
- `wParam` = X/Y anchor coordinates via `GET_X_LPARAM`/`GET_Y_LPARAM` for select/mouse events

Routing in WndProc:
```rust
WM_TRAY_MSG => {
    match (lp & 0xFFFF) as u32 {
        WM_RBUTTONUP | WM_CONTEXTMENU => tray.show_menu(hwnd, locked),
        WM_LBUTTONDBLCLK => app::toggle(hwnd),
        _ => {}
    }
}
```

## CRITICAL: TaskbarCreated NOT Received by HWND_MESSAGE

HWND_MESSAGE windows are NOT top-level → they do NOT receive `TaskbarCreated` broadcast.
See research 02. For v1.0: accept this limitation (no icon recovery after Explorer crash).

If recovery is needed in future: use a real hidden top-level window or CreateWindowExW without HWND_MESSAGE parent.

## TrackPopupMenu

**MUST call `SetForegroundWindow(hwnd)` before `TrackPopupMenu`** — without it the menu won't dismiss when user clicks away.

```rust
SetForegroundWindow(hwnd);
TrackPopupMenu(hmenu, TPM_BOTTOMALIGN | TPM_LEFTALIGN, x, y, 0, hwnd, null());
PostMessageW(hwnd, WM_NULL, 0, 0);  // standard fix to ensure menu dismisses
DestroyMenu(hmenu);  // TrackPopupMenu does NOT destroy the menu — caller must
```

- `TPM_BOTTOMALIGN | TPM_LEFTALIGN` = correct for taskbar tray menu positioning
- `TrackPopupMenu` returns item ID or 0 when `TPM_RETURNCMD` is NOT set; menu selection delivered via `WM_COMMAND`
- Without `TPM_RETURNCMD`: selection fires `WM_COMMAND` — use this
- **`DestroyMenu` is the caller's responsibility** — `TrackPopupMenu` does NOT destroy the menu

## Balloon Notification

```rust
// Fields for balloon:
uFlags: NIF_INFO,
szInfo: wide_string_truncated_to_256,  // null-terminated; empty = hide balloon
szInfoTitle: wide_string_truncated_to_64,
dwInfoFlags: NIIF_INFO | NIIF_NOSOUND,
// uTimeout is IGNORED on Vista+ (accessibility settings control duration)
```

Constants:
- `NIIF_INFO = 0x00000001`
- `NIIF_NOSOUND = 0x00000010`

## Icon Loading

```rust
// From embedded resource (ID 1):
LoadImageW(hinstance, MAKEINTRESOURCEW(1), IMAGE_ICON, 16, 16, LR_DEFAULTCOLOR)

// Fallback (shared system handle — must NOT be destroyed):
LoadIconW(null_mut(), IDI_APPLICATION as *const u16)
```

- `MAKEINTRESOURCEW(id)` in Rust: `id as *const u16` (windows-sys does not expose this macro directly)
- Shared system icons (IDI_APPLICATION etc.) must NOT be passed to `DestroyIcon` — they are shared handles owned by the OS
- Icons loaded via `LoadImageW` from resources: can call `DestroyIcon` if needed; when process exits, reclaimed automatically

## windows-sys Constants

```rust
NIM_ADD       = 0
NIM_MODIFY    = 1
NIM_DELETE    = 2
NIM_SETVERSION = 4

NIF_MESSAGE = 0x00000001
NIF_ICON    = 0x00000002
NIF_TIP     = 0x00000004
NIF_INFO    = 0x00000010

NIIF_INFO    = 0x00000001
NIIF_NOSOUND = 0x00000010

NOTIFYICON_VERSION_4 = 4
```

- `Shell_NotifyIconW` is in `Win32_UI_Shell` feature
