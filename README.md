# Wraith — Windows Physical Input Blocker for AI Automation

You're running an AI agent on your machine — Claude computer-use, an AutoHotkey script, a PyAutoGUI bot, whatever — and you need to step away. The problem is that anyone who sits down at your desk can just grab the keyboard and mouse and start interfering. You don't want to close the AI session, you just want the physical inputs to go dead until you get back.

That's what Wraith does. Lock it from the system tray and the keyboard and mouse stop responding to physical input. The AI keeps typing, clicking, scrolling — it doesn't notice anything changed. Unlock it when you're back.

**Works with:** Claude computer-use, OpenAI Operator, AutoHotkey, PyAutoGUI, Sikuli, Parsec, RDP, and any tool that sends input via `SendInput()`.

**Use cases:**
- Block physical keyboard and mouse input on Windows while an AI agent runs unattended
- Prevent desk interference during overnight Claude or GPT computer-use sessions
- Lock a shared workstation while automation runs in the foreground
- Any scenario needing Windows input blocking that preserves synthetic/injected input

---

## How it actually works

Windows has a feature most people don't know about: every input event carries a flag that says whether it came from physical hardware or was injected by software. When you call `SendInput()`, press a key via AutoHotkey, or do anything through a remote session like Parsec or RDP, Windows sets `LLKHF_INJECTED` on the keyboard event (bit 4 of the flags field) and `LLMHF_INJECTED` on the mouse event (bit 0). Physical keystrokes and mouse movements don't have that flag set.

Wraith installs two low-level Windows hooks — `WH_KEYBOARD_LL` and `WH_MOUSE_LL` — that intercept every input event system-wide before it reaches any application. For each event, it checks one thing: is the injected flag set? If yes, pass it through. If no and we're locked, return 1 and swallow it.

That's the whole mechanism. No kernel driver, no virtual device, no registry tricks for the main blocking logic. The binary is around 50KB.

**What Wraith protects against:** a person sitting down at the keyboard. **What it doesn't:** software already running on the machine. Any process that calls `SendInput()` sets the injected flag and passes straight through — that's the intended behavior, since that's how your AI agent sends input. On a clean machine running only software you trust, that's the right trade-off.

There's one thing you can't block from user mode: `Ctrl+Alt+Del`. Microsoft hardwired that into the kernel as the Secure Attention Sequence and no application can intercept it. Everything else is fair game.

---

## Remote access while locked

One thing worth knowing: input from Parsec, RDP, and VNC is also tagged as injected by Windows — those tools go through `SendInput` or equivalent. That means you can lock Wraith on the physical machine and still take control remotely from another device without touching the unlock hotkey. The physical desk is locked, the remote session works fine.

---

## Getting started

**Installer (recommended):** Grab `wraith-setup.exe` from [Releases](https://github.com/shadow-dragon-2002/Wraith/releases) and run it. It handles everything.

**Portable:** Download `wraith.exe` and `wraith.ini` and put them in the same folder. Run it — Windows will ask for administrator access, which is required for the hooks to work reliably.

Once running, Wraith sits in the system tray. Right-click for the menu, double-click to toggle lock state.

---

## First run — SmartScreen warning

Windows will show a SmartScreen warning the first time you run the `.exe` or installer: *"Windows protected your PC"*. This happens because the binary is unsigned (code-signing certificates cost money and Wraith is a free personal-use tool).

To proceed: click **More info**, then **Run anyway**. You're running a ~50KB Rust binary with no network access beyond the GitHub release check. Source is [here](https://github.com/shadow-dragon-2002/Wraith) if you want to audit or build it yourself.

---

## Controls

| What | Default |
|------|---------|
| Lock | `Ctrl + Shift + Alt + L` |
| Unlock | `Ctrl + Shift + Alt + U` |
| Panic unlock | Hold `Esc` for 3 seconds |
| Toggle | Double-click the tray icon |
| Menu | Right-click the tray icon |

The panic unlock is there as a last resort — if you forget your hotkey combo or something goes wrong, hold Escape for three seconds and it unlocks regardless.

---

## Configuration

All settings live in `wraith.ini` next to the `.exe`. Edit it in Notepad and restart Wraith to apply changes.

```ini
[Wraith]

; Modifier bitmask: MOD_ALT=1, MOD_CONTROL=2, MOD_SHIFT=4, MOD_WIN=8
; Ctrl+Shift+Alt = 2+4+1 = 7
LockModifiers=7
LockKey=76        ; 'L' — virtual key code

UnlockModifiers=7
UnlockKey=85      ; 'U' — virtual key code

PanicKey=27       ; Escape

LockOnStart=0     ; set to 1 to lock automatically when Wraith starts
```

The key codes are standard Windows virtual key codes: A-Z are 65-90, 0-9 are 48-57, function keys F1-F12 are 112-123. Full list at [learn.microsoft.com](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes).

---

## Building from source

Wraith is written in Rust and cross-compiles from WSL to a Windows `.exe` via MinGW. You don't need a Windows machine to build it.

**One-time setup:**

```bash
rustup target add x86_64-pc-windows-gnu
sudo apt install -y gcc-mingw-w64-x86-64
```

**Build:**

```bash
cargo build --release --target x86_64-pc-windows-gnu
# Output: target/x86_64-pc-windows-gnu/release/wraith.exe
```

CI runs on GitHub Actions (Ubuntu + MinGW). Pushing a `vX.Y.Z` tag creates a release with the `.exe` and installer attached automatically.

If you want to embed custom icons, drop your `.ico` files into `assets/` and update `src/resource.rc` to reference them, then rebuild. The tray icon uses the 16x16 size; include at least 16x16 and 32x32 in the `.ico` file.

---

## Why not BlockInput?

`BlockInput()` is the obvious Windows API for this. The problem is it blocks everything — physical and synthetic alike. Your AI agent would also be locked out. Wraith specifically checks the injected flag so synthetic input passes through and only physical hardware gets blocked. The injected-flag technique uses documented Windows APIs (`LLKHF_INJECTED` / `LLMHF_INJECTED`) but hasn't been packaged as a ready-made lock utility before.

---

## Project layout

```
src/
  main.rs       startup, single-instance mutex, message loop
  app.rs        lock/unlock, WndProc, DisableTaskMgr policy
  hooks.rs      keyboard and mouse hook callbacks, global atomics
  tray.rs       system tray icon, menu, balloon notifications
  config.rs     wraith.ini loading
  autostart.rs  Windows startup registry entry
  updater.rs    background GitHub release check

installer/wraith.nsi   NSIS installer script
.github/workflows/     CI — builds the .exe and creates releases on tag push
```

For deeper reading on the architecture, hook timing constraints, module interfaces, and how to extend things, see [CONTRIBUTING.md](CONTRIBUTING.md).

---

## License

[PolyForm Noncommercial 1.0.0](LICENSE) — free for personal and open-source use, not for commercial products or services.
