# Wraith

**Block physical keyboard and mouse input while keeping AI tools and automation running.**

Wraith sits in your system tray. Lock it before you step away — your mouse and keyboard go dead to anyone at the desk, but Claude in Chrome, computer-use agents, AutoHotkey scripts, and any other software-driven input carry on without interruption.

---

## How it works

Windows tags every input event with an **injected flag** (`LLKHF_INJECTED` for keyboard, `LLMHF_INJECTED` for mouse) when the event originates from software rather than physical hardware. Wraith installs low-level hooks (`WH_KEYBOARD_LL` / `WH_MOUSE_LL`) and checks this flag on every event:

- **Flag set** → synthetic input (AI tool, script, `SendInput`, etc.) → **pass through**
- **Flag not set** → physical hardware → **block when locked**

No kernel driver required. Pure Win32 API, minimal footprint.

> ⚠️ **Hard limitation:** `Ctrl+Alt+Del` is hardwired into the Windows kernel (Secure Attention Sequence) and **cannot be blocked by any user-mode software**, including Wraith. Everything else is blocked.

> ✅ **RDP / remote access works while locked:** Input from Remote Desktop, Parsec, VNC, and similar tools is tagged as injected by Windows — Wraith passes it through. You can lock the physical desk and still check in remotely via Parsec or RDP without touching the unlock combo.

---

## Features

- 🔒 Blocks physical keyboard and mouse
- 🤖 AI tools and automation pass through unaffected
- 🖥️ Remote access (Parsec, RDP, VNC) works while locked — check in from anywhere
- 💤 Prevents sleep, screensaver, and display-off while running
- 🖥️ System tray icon with lock/unlock state indicator
- ⌨️ Fully configurable hotkeys via `wraith.ini`
- 🆘 Panic unlock: hold `Esc` for 3 seconds
- 🚀 Optional auto-start with Windows
- 🔄 Built-in update checker (GitHub releases)
- 🪶 ~50 KB binary, zero dependencies, no runtime

---

## Installation

### Option A — Installer (recommended)
Download `wraith-setup.exe` from [Releases](https://github.com/shadow-dragon-2002/Wraith/releases) and run it.

### Option B — Portable
Download `wraith.exe` + `wraith.ini`, place them in the same folder, run as Administrator.

> Wraith requires **Administrator** privileges for reliable hook installation. The embedded manifest requests this automatically.

---

## Usage

| Action | Default |
|--------|---------|
| Lock   | `Ctrl + Shift + Alt + L` |
| Unlock | `Ctrl + Shift + Alt + U` |
| Panic unlock | Hold `Esc` for 3 seconds |
| Toggle via tray | Double-click the tray icon |
| Menu | Right-click the tray icon |

---

## Configuration

Edit `wraith.ini` (same directory as the `.exe`):

```ini
[Wraith]

; Modifier bitmask: MOD_ALT=1, MOD_CONTROL=2, MOD_SHIFT=4, MOD_WIN=8
; Ctrl+Shift+Alt = 2+4+1 = 7
LockModifiers=7
LockKey=76        ; Virtual key code for 'L'

UnlockModifiers=7
UnlockKey=85      ; Virtual key code for 'U'

PanicKey=27       ; Escape

LockOnStart=0     ; Set to 1 to lock immediately on launch
```

Virtual key codes: https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes

Common values: `A-Z` = 65–90, `0-9` = 48–57, `Esc` = 27, `F1-F12` = 112–123

---

## Building from source

Built in Rust. Cross-compiles from WSL (Linux → Windows `.exe`) via MinGW.

### One-time WSL setup
```bash
rustup target add x86_64-pc-windows-gnu
sudo apt install -y gcc-mingw-w64-x86-64
```

### Build
```bash
cargo build --release --target x86_64-pc-windows-gnu
```
Output: `target/x86_64-pc-windows-gnu/release/wraith.exe`

### Custom icons
Add `wraith_unlocked.ico` and `wraith_locked.ico` to the project root and reference them in `src/resource.rc`, then rebuild.

---

## Project structure

```
wraith/
├── src/
│   ├── main.rs        ← entry point, single-instance mutex, message loop
│   ├── app.rs         ← lock/unlock logic, WndProc, state coordination
│   ├── hooks.rs       ← WH_KEYBOARD_LL / WH_MOUSE_LL callbacks, global atomics
│   ├── tray.rs        ← system tray icon, menu, balloon notifications
│   ├── config.rs      ← INI load/save, Config struct
│   └── updater.rs     ← background update check via GitHub API
├── installer/
│   └── wraith.nsi     ← NSIS installer script
├── .github/
│   └── workflows/
│       └── build.yml  ← GitHub Actions CI/CD (ubuntu + MinGW cross-compile)
├── Cargo.toml
├── wraith.ini         ← default config (shipped with .exe)
└── LICENSE            ← MIT
```

---

## Why not just use BlockInput?

`BlockInput()` (Windows API) blocks **all** input including synthetic — so it would also block AI tools and automation. Wraith checks the `LLKHF_INJECTED` / `LLMHF_INJECTED` flags at the hook level, which lets synthetic input through while stopping physical input. No other open-source tool on GitHub implements this distinction as a ready-made utility.

---

## Compared to similar tools

| Tool | Physical only | Mouse | Tray | Config | Synthetic passthrough |
|------|:---:|:---:|:---:|:---:|:---:|
| **Wraith** | ✅ | ✅ | ✅ | ✅ | ✅ |
| Padlock | ❌ | ✅ | ✅ | ✅ | ❌ |
| ahk-keyboard-locker | ❌ | opt | ✅ | ✅ | ❌ |
| AutoHotInterception | per-device | per-device | ❌ | ✅ | n/a |

---

## License

MIT — see [LICENSE](LICENSE)
