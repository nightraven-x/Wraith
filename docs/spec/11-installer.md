# Spec 11 — NSIS Installer

> Research prerequisite: [../research/11-installer.md](../research/11-installer.md)
> Implements: `installer/wraith.nsi` — Step 9

---

## Purpose

Produce a self-contained Windows installer (`wraith-setup.exe`) via NSIS that:
- Installs `wraith.exe` and `wraith.ini` to Program Files
- Creates a Start Menu shortcut
- Registers an uninstaller
- Does NOT require admin by default (see note below)

---

## NSIS Script: `installer/wraith.nsi`

```nsi
!define APPNAME "Wraith"
!define VERSION "1.0.0"
!define PUBLISHER "shadow-dragon-2002"
!define INSTALL_DIR "$PROGRAMFILES64\Wraith"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wraith"

Name "${APPNAME} ${VERSION}"
OutFile "wraith-setup.exe"
InstallDir "${INSTALL_DIR}"
InstallDirRegKey HKLM "${UNINSTALL_KEY}" "InstallLocation"
RequestExecutionLevel admin
ShowInstDetails show
ShowUninstDetails show

Section "Install"
    SetOutPath "$INSTDIR"
    File "wraith.exe"
    File "wraith.ini"

    ; Start Menu shortcut
    CreateDirectory "$SMPROGRAMS\Wraith"
    CreateShortcut "$SMPROGRAMS\Wraith\Wraith.lnk" "$INSTDIR\wraith.exe"
    CreateShortcut "$SMPROGRAMS\Wraith\Uninstall Wraith.lnk" "$INSTDIR\uninstall.exe"

    ; Register uninstaller
    WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayName" "${APPNAME}"
    WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
    WriteRegStr HKLM "${UNINSTALL_KEY}" "Publisher" "${PUBLISHER}"
    WriteRegStr HKLM "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "${UNINSTALL_KEY}" "UninstallString" "$INSTDIR\uninstall.exe"
    WriteRegDWORD HKLM "${UNINSTALL_KEY}" "NoModify" 1
    WriteRegDWORD HKLM "${UNINSTALL_KEY}" "NoRepair" 1

    WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\wraith.exe"
    Delete "$INSTDIR\wraith.ini"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"

    Delete "$SMPROGRAMS\Wraith\Wraith.lnk"
    Delete "$SMPROGRAMS\Wraith\Uninstall Wraith.lnk"
    RMDir "$SMPROGRAMS\Wraith"

    DeleteRegKey HKLM "${UNINSTALL_KEY}"

    ; Remove autostart if set (HKCU, no admin needed)
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Wraith"
SectionEnd
```

---

## Behavioral Requirements

**R1.** `RequestExecutionLevel admin` — installs to Program Files, requires UAC elevation.
This is intentional: the installer runs elevated; Wraith itself runs as `asInvoker`
(no elevation at runtime).

**R2.** Ship `wraith.ini` alongside `wraith.exe`. The INI in Program Files is read-only
for standard users — that is correct behavior (INI is read-only by design).

**R3.** The uninstaller removes the autostart registry value (HKCU) from the uninstall
section. `DeleteRegValue HKCU ...` does not require elevation and is safe to run even
if the value is absent.

**R4.** Output file: `wraith-setup.exe` — placed in the `installer/` directory after
`makensis installer\wraith.nsi` runs.

---

## CI Integration

**R5.** The CI `installer` job runs on `windows-latest` (makensis is pre-installed on
GitHub Actions Windows runners — verify in research). It downloads the `wraith-windows-x64`
artifact (containing `wraith.exe` and `wraith.ini`) and runs `makensis`.

**R6.** The NSIS script references `wraith.exe` and `wraith.ini` relative to the
working directory. CI must `cd` to the artifact download directory before running
`makensis`, or adjust paths in the script.

---

## Dependencies

- `src/resource.rc` / `assets/*.ico` — icons embedded in `wraith.exe` (Step 10)
- `.github/workflows/build.yml` — installer job (spec 12)

---

## Edge Cases

- **makensis not available in CI:** `continue-on-error: true` in CI means the release
  still publishes `wraith.exe` even if the installer step fails.
- **Program Files path on 32-bit Windows:** Not supported. `$PROGRAMFILES64` installs
  to `C:\Program Files`. 32-bit Windows is out of scope.
- **wraith.ini modified after install:** The file in Program Files cannot be modified
  by standard users. Users who need custom config should copy the exe elsewhere or
  run as admin. Document this.
- **Uninstall while Wraith is running:** Wraith holds `wraith.exe` open. The uninstaller
  should handle this — either warn the user or schedule deletion on reboot. Out of
  scope for v1.0 — document as known limitation.
