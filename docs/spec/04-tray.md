# Spec 04 — System Tray

> Research prerequisite: [../research/04-tray.md](../research/04-tray.md)
> Implements: `src/tray.rs` — Step 3

---

## Purpose

Manage the system-tray notification-area icon: add it on startup, update its appearance
on lock/unlock, show the context menu on right-click, show balloon notifications for
update alerts, and remove the icon cleanly on exit.

---

## Public Interface

```rust
pub struct TrayIcon {
    hwnd:   HWND,
    h_icon_locked:   HICON,
    h_icon_unlocked: HICON,
    locked: bool,
}

impl TrayIcon {
    pub fn new(hwnd: HWND) -> Self;
    // Adds the icon to the tray. Caller boxes the result and stores it via
    // SetWindowLongPtrW(hwnd, GWLP_USERDATA).

    pub fn set_locked(&mut self, locked: bool);
    // Updates icon and tooltip to reflect current state.

    pub fn show_balloon(&self, title: &str, msg: &str);
    // Shows an info balloon. NIIF_NOSOUND to avoid noise.

    pub fn show_menu(&self, hwnd: HWND, locked: bool);
    // Creates and tracks a popup menu. Lock item greyed when locked;
    // Unlock item greyed when unlocked.

    pub fn destroy(&mut self);
    // Removes the icon from the tray (NIM_DELETE). Called from WM_DESTROY.
}
```

---

## Icon IDs and Callback

**R1.** `uID = 1` for the tray icon (arbitrary, must be consistent across calls).

**R2.** `uCallbackMessage = WM_TRAY_MSG (WM_USER + 1)`.

**R3.** `uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP`.

---

## Icon Loading

**R4.** Load two icons: one for locked state, one for unlocked state.
Attempt `LoadImageW(hInstance, resource_id, IMAGE_ICON, 16, 16, LR_DEFAULTCOLOR)`.

**R5.** If `LoadImageW` returns NULL (resource not yet embedded, Step 9 not done):
fall back to `LoadIconW(NULL, IDI_APPLICATION)` for both. Fallback stays permanently
as a safety net.

**R6.** Icon handles are stored in `TrayIcon` and used for `Shell_NotifyIconW`.
Must remain valid for the lifetime of the tray icon.

---

## new()

**R7.** Zero-init a `NOTIFYICONDATAW`. Set `cbSize` to `size_of::<NOTIFYICONDATAW>()`.

**R8.** Call `Shell_NotifyIconW(NIM_ADD, &data)`.

**R9.** Call `Shell_NotifyIconW(NIM_SETVERSION, &data)` with `uVersion = NOTIFYICON_VERSION_4`
in the union field. Verify correct field name in research — this enables `WM_CONTEXTMENU`
and `NIN_*` notifications on Vista+.

**R10.** Initial state: unlocked icon + tooltip `"Wraith — Unlocked"`.

---

## set_locked()

**R11.** Store the new `locked` state in `self.locked`.

**R12.** Update `hIcon` to the locked or unlocked icon handle.

**R13.** Update `szTip` to `"Wraith — Locked"` or `"Wraith — Unlocked"` (wide).

**R14.** Call `Shell_NotifyIconW(NIM_MODIFY, &data)` with `uFlags = NIF_ICON | NIF_TIP`.

---

## show_balloon()

**R15.** Set `uFlags |= NIF_INFO`, `dwInfoFlags = NIIF_INFO | NIIF_NOSOUND`.

**R16.** Copy `title` into `szInfoTitle` and `msg` into `szInfo` (wide, null-terminated,
truncated to fit buffer sizes: `szInfoTitle[64]`, `szInfo[256]`).

**R17.** Call `Shell_NotifyIconW(NIM_MODIFY, &data)`.

---

## show_menu()

**R18.** Call `SetForegroundWindow(hwnd)` before `TrackPopupMenu` — required by
Windows to ensure the menu dismisses correctly when clicking away.

**R19.** `CreatePopupMenu()` → append items:
- `ID_LOCK` / `"Lock"` — `MF_GRAYED` if `locked == true`
- `ID_UNLOCK` / `"Unlock"` — `MF_GRAYED` if `locked == false`
- Separator (`MF_SEPARATOR`)
- `ID_AUTOSTART` / `"Start with Windows"` — checked if autostart enabled
- Separator
- `ID_EXIT` / `"Exit"`

**R20.** Get cursor position via `GetCursorPos`. Call `TrackPopupMenu` with
`TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_LEFTALIGN` at cursor position.

**R21.** `DestroyMenu(hmenu)` after `TrackPopupMenu` returns.

**R22.** Autostart checked state: call `app::is_autostart()` to determine check mark.

---

## WM_TRAY_MSG Routing (in wnd_proc, not in tray.rs)

**R23.** On `WM_TRAY_MSG`:
- `LOWORD(l_param) == WM_RBUTTONUP || WM_CONTEXTMENU` → `tray.show_menu(hwnd, LOCKED.load(Relaxed))`
- `LOWORD(l_param) == WM_LBUTTONDBLCLK` → `app::toggle(hwnd)`
- All other lParam values → ignore

Note: `NIM_SETVERSION` with `NOTIFYICON_VERSION_4` changes lParam encoding — verify
in research whether LOWORD(lParam) is still the correct extraction.

---

## Explorer Restart Recovery

**R24.** Register for `WM_TASKBARCREATED` (returned by `RegisterWindowMessageW(L"TaskbarCreated")`).
On receipt: call `Shell_NotifyIconW(NIM_ADD, &data)` again to re-add the icon.
Explorer restart removes all tray icons; this restores Wraith's icon without requiring
a full restart.

---

## destroy()

**R25.** Call `Shell_NotifyIconW(NIM_DELETE, &data)`. Called from `WM_DESTROY` and
`WM_ENDSESSION` handlers in `wnd_proc`.

---

## Dependencies

- `app.rs` — `wnd_proc` routes `WM_TRAY_MSG` to tray methods.
- `app.rs` — `is_autostart()` called from `show_menu`.
- `hooks.rs` — `LOCKED` read to grey correct menu item.

---

## Edge Cases

- **Duplicate NIM_ADD:** if called twice (e.g. Explorer restart fires before prior icon
  is removed), `NIM_ADD` fails silently. Non-fatal.
- **NULL icon handle after LoadImage + LoadIcon both fail:** `h_icon` = NULL.
  `Shell_NotifyIconW` with NULL icon shows nothing in tray. Acceptable fallback.
- **Tooltip truncation:** `szTip` is `[u16; 128]`. "Wraith — Locked" is well under limit.
- **Balloon ignored by OS:** Windows 10+ may suppress balloons if focus-assist is on.
  Non-fatal — the icon color change still communicates state.
