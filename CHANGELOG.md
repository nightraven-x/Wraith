# Changelog

All notable changes to Wraith are documented here.

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
