# Research 11 — NSIS Installer

Verified facts for `installer/wraith.nsi`.

---

## NSIS on GitHub Actions

- NSIS (`makensis`) IS pre-installed on `windows-latest` (Windows Server 2022) runners
- `windows-server-2025` image does NOT include NSIS (separate issue/PR); use `windows-latest`
- Command: `makensis` (not `makensis.exe` — both work on Windows)
- Location: in PATH — no PATH manipulation needed

## NSIS Script Fundamentals

```nsis
Name "Wraith"
OutFile "wraith-setup.exe"   ; relative to SCRIPT LOCATION, not working dir
InstallDir "$PROGRAMFILES64\Wraith"
RequestExecutionLevel admin  ; UAC elevation prompt on install

; $PROGRAMFILES64 = C:\Program Files on 64-bit Windows
; (NOT $PROGRAMFILES which = C:\Program Files (x86) on 64-bit)
```

- `OutFile` is relative to the NSIS script file's location
- `makensis installer\wraith.nsi` from repo root → `installer\wraith-setup.exe`
- `RequestExecutionLevel admin` → UAC elevation → $SMPROGRAMS = All Users start menu

## File Section

```nsis
Section "Install"
    SetOutPath "$INSTDIR"
    File "wraith.exe"    ; relative to script location OR working directory where makensis runs
    File "wraith.ini"
    WriteUninstaller "$INSTDIR\uninstall.exe"
    ; WriteUninstaller generates the uninstaller — NO separate script section needed for the file
```

- `File` command: path relative to **working directory where makensis is invoked**, NOT script location
- In CI: `actions/download-artifact@v4` downloads to current directory (repo root by default)
- Running `makensis installer\wraith.nsi` from repo root with artifact in root → `File "wraith.exe"` finds `.\wraith.exe`
- Adjust by using absolute paths or `!cd` in script if paths differ

## Uninstall Section

```nsis
Section "Uninstall"
    Delete "$INSTDIR\wraith.exe"
    Delete "$INSTDIR\wraith.ini"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"    ; Silently fails if non-empty (user added files) — acceptable

    ; Start menu cleanup
    Delete "$SMPROGRAMS\Wraith\Wraith.lnk"
    RMDir "$SMPROGRAMS\Wraith"

    ; Registry cleanup
    DeleteRegKey HKLM "${UNINSTALL_KEY}"

    ; KNOWN BUG: Autostart cleanup
    ; DeleteRegValue HKCU "..." "Wraith"  ; This refers to ADMIN user's HKCU, not logged-in user!
    ; For v1.0: omit autostart cleanup from uninstaller — user must manually disable
SectionEnd
```

**HKCU autostart cleanup bug**: The uninstaller runs elevated (admin). `HKCU` in an elevated process maps to the ADMIN account's registry hive, NOT the logged-in user's hive. The autostart entry (stored in the logged-in user's HKCU) will NOT be cleaned up correctly.
- v1.0 mitigation: Omit HKCU cleanup from uninstaller. Document that user must disable autostart via tray menu BEFORE uninstalling, OR manually delete the Run entry.

## Uninstaller Registration (Add/Remove Programs)

```nsis
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wraith"

WriteRegStr  HKLM "${UNINSTALL_KEY}" "DisplayName"          "Wraith"
WriteRegStr  HKLM "${UNINSTALL_KEY}" "UninstallString"      "$INSTDIR\uninstall.exe"
WriteRegStr  HKLM "${UNINSTALL_KEY}" "InstallLocation"      "$INSTDIR"
WriteRegStr  HKLM "${UNINSTALL_KEY}" "DisplayVersion"       "1.0.0"
WriteRegStr  HKLM "${UNINSTALL_KEY}" "Publisher"            "shadow-dragon-2002"
WriteRegDWORD HKLM "${UNINSTALL_KEY}" "NoModify"            1
WriteRegDWORD HKLM "${UNINSTALL_KEY}" "NoRepair"            1
```

- `DisplayName` + `UninstallString` are the minimum required for Add/Remove Programs
- `NoModify=1` / `NoRepair=1` hides "Change" and "Repair" buttons

## Start Menu Shortcuts

```nsis
CreateDirectory "$SMPROGRAMS\Wraith"
CreateShortcut "$SMPROGRAMS\Wraith\Wraith.lnk" "$INSTDIR\wraith.exe"
; Syntax: CreateShortcut "destination.lnk" "target"
```

- Elevated install → $SMPROGRAMS = All Users start menu (C:\ProgramData\Microsoft\Windows\Start Menu\Programs)

## shell: cmd for makensis

`shell: cmd` is NOT required — default PowerShell shell handles `makensis installer\wraith.nsi` correctly.
Backslash paths work in PowerShell. Use `continue-on-error: true` since NSIS step is optional.
