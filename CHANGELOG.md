# Changelog

All notable changes to Wraith are documented here.

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
