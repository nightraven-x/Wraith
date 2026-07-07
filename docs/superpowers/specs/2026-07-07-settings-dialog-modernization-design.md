# Settings dialog modernization

Cosmetic-only overhaul of the native Win32 settings dialog (`src/settings.rs` + `src/resource.rc`) to look like a modern Win10/11 dialog: themed controls, Segoe UI, wider spacing, dark mode, Win11 rounded corners. No behavior, validation, or control-ID changes — existing tests in `settings.rs` drive controls by ID and are unaffected.

## Scope

1. **`wraith.manifest`** — add ComCtl32 v6 side-by-side `<dependency>`. Flips buttons/checkbox/edit fields from classic square rendering to the OS's current visual style. No code.

2. **`src/resource.rc`** — `FONT 8, "MS Shell Dlg"` → `FONT 9, "Segoe UI"`. Dialog resized `200x160` → `~240x210` DLU with consistent margins (12 DLU) and label-to-field spacing (currently 12 DLU gap, cramped). Same control IDs, same order.

3. **`src/theme.rs`** (new) —
   - `pub fn system_prefers_dark() -> bool` — reads `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme` (`Win32_System_Registry`, already a dependency feature). Missing key or read failure → `false` (default to light, matches Windows' own default when the key is absent pre-first-personalization).
   - `pub fn apply(hwnd: HWND, dark: bool)` — called once from `settings.rs`'s `WM_INITDIALOG`:
     - `DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ...)` — dark titlebar. Ignore `HRESULT` failure (pre-1809 Windows lacks the attribute; that's expected, not an error).
     - `DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWC_ROUND)` — Win11 rounded corners. Same ignore-failure rule.
     - `SetWindowTheme(ctrl, "DarkMode_Explorer" | "", None)` on each EDITTEXT, the checkbox, and OK/Cancel — dark flat rendering for standard controls (the same string Explorer/Notepad/Terminal use; documented API, conventional string, stable since Win10 1809).

4. **`src/settings.rs`** — `dlg_proc`:
   - `WM_INITDIALOG` calls `theme::apply(hwnd, theme::system_prefers_dark())` after existing control seeding.
   - New `WM_CTLCOLORDLG` / `WM_CTLCOLORSTATIC` / `WM_CTLCOLOREDIT` arms: when dark, `SetTextColor`/`SetBkColor` + return a solid dark brush (two brushes: dialog/label background, slightly lighter edit-field background — both created lazily once via the same atomic-pointer pattern the codebase already uses for `APP_HWND`/`APP_TRAY`, not a `Mutex`, since these are single-threaded-from-the-dialog values that never need blocking access). Buttons/checkbox need no handler — `SetWindowTheme` alone darkens them.
   - When light (or `system_prefers_dark()` is `false`), these arms fall through to `0` (default handling) — zero behavior change from today.

5. **`build.rs`** — add `cargo:rustc-link-lib=dwmapi` and `cargo:rustc-link-lib=uxtheme`, same pattern as the existing `winhttp` link (mingw-w64 ships both import libs).

6. **`Cargo.toml`** — add `Win32_Graphics_Dwm` windows-sys feature. `Win32_UI_Controls` (`SetWindowTheme`) and `Win32_Graphics_Gdi` (brushes/`SetTextColor`/`SetBkColor`) are already enabled.

## Isolation / no perf-or-function impact

Every change lives inside `WM_INITDIALOG` / `WM_CTLCOLOR*` of the modal settings dialog, which already fully bypasses the low-level hooks while open (`SETTINGS_OPEN`, existing code). Nothing here touches `hooks.rs`, `app.rs`'s `lock()`/`unlock()`, or the `GetMessageW` main loop.

## Graceful degradation

- Windows 7/8/8.1 (in the manifest's `supportedOS` list): `DwmSetWindowAttribute` calls fail harmlessly (ignored `HRESULT`) — dialog renders in whatever the OS's current visual style is, no dark titlebar, square corners. Still an upgrade over today via ComCtl32 v6 + Segoe UI + spacing alone.
- `system_prefers_dark()` registry-read failure → light mode, i.e. today's (post-ComCtl32-v6) appearance.

## Testing

- Existing `settings.rs` test suite (drives the real dialog by control ID via `SendInput`) must keep passing unmodified — proves control IDs, validation, and commit/cancel behavior are untouched.
- `cargo build --release --target x86_64-pc-windows-gnu` must succeed (cross-compiled from WSL; no Windows GUI session available here to visually verify).
- No new automated test for the paint/theme logic itself — it's pure Win32 message-handler wiring with no branch worth a unit test beyond "does it compile and not regress the existing suite" (`system_prefers_dark()`'s single branch is a one-line registry read, YAGNI on a dedicated test per project's own testing bar for trivial functions).
- Visual verification (dark mode, corners, spacing) is manual — user will build and screenshot on real Windows hardware.
