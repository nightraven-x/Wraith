# Changelog

All notable changes to Wraith are documented here.

## [1.1.0] - 2026-07-07

### Added

- Native Win32 settings dialog (tray menu → Settings...) for changing the
  lock combo, unlock combo, panic key, and lock-on-start — no more editing
  `wraith.ini` by hand and restarting. Changes take effect immediately and
  persist back to the ini.
- Reusable hotkey-recorder control: click into a field and press the combo
  you want, instead of typing raw virtual-key numbers. Used for the lock
  combo, unlock combo, and now the panic key too.
- Dark mode + Windows 11 rounded corners for the settings dialog (ComCtl32
  v6, Segoe UI, `DwmSetWindowAttribute`), matching the system's light/dark
  preference.
- `wraith.ini` now falls back to `%LOCALAPPDATA%\Wraith\wraith.ini` when no
  portable ini sits next to the exe — fixes settings not surviving a
  restart on an installed (Program Files) copy, which a standard user
  account can't write to.
- `RunOnce` failsafe for the `DisableTaskMgr` anti-circumvention policy: if
  Wraith is killed, crashes, or the machine loses power while locked, the
  policy is cleaned up automatically at the next interactive logon instead
  of staying stuck indefinitely.

### Fixed

- Settings dialog could fire the real lock/unlock action while recording
  the currently-active combo, and provided no way to reach its own Cancel
  button if Wraith was already locked when it opened. Hooks now pass all
  input through unconditionally while the dialog is open.
- Exiting Wraith while locked left `DisableTaskMgr` stuck system-wide with
  no running process left to clear it — `WM_DESTROY` now clears it
  unconditionally, not just when unlocking normally.
- Panic key accepted virtual-key code 0 (Win32's reserved "no key" value),
  which could silently make panic-unlock unreachable; found in a security
  audit and now rejected by validation (1-255 only).

## [1.0.1] - 2026-07-07

### Fixed

- Unlock hotkey (and panic-key hold-to-unlock) never fired while locked. The
  combo check read modifier state via `GetAsyncKeyState`, but our own keyboard
  hook returns `1` (never calling `CallNextHookEx`) for modifier keydowns
  while locked — which stops Windows from updating the state
  `GetAsyncKeyState` reads. So Ctrl/Alt/Shift always read as "not held" during
  the unlock check, even while physically pressed, and the combo could never
  match. Fixed by tracking modifier and panic-key hold state ourselves from
  the raw hook events instead of trusting `GetAsyncKeyState`.

## [1.0.0] - 2025-01-01

Initial public release.

### Features

- Block physical keyboard and mouse input while passing synthetic (injected) input through unaffected
- `WH_KEYBOARD_LL` + `WH_MOUSE_LL` low-level hooks — no kernel driver required
- Configurable lock/unlock hotkeys and panic unlock (hold key for 3 seconds)
- System tray icon with lock state indicator and right-click menu
- DisableTaskMgr policy applied on lock to prevent Ctrl+Alt+Del escape
- Hook watchdog: reinstalls hooks every 5 seconds to survive Parsec/RDP virtual driver teardown
- Modifier key-up passthrough prevents stuck Ctrl/Shift/Alt after lock combo fires
- Start-with-Windows autostart toggle in tray menu
- Background update checker via GitHub Releases API
- Single-instance enforcement via named mutex
- Portable `.exe` + `.ini` or NSIS installer
