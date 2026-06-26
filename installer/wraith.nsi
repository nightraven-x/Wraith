; Wraith NSIS Installer
; Requires: NSIS 3.x  (https://nsis.sourceforge.io/)
; Build: makensis installer\wraith.nsi  (run from repo root)
; Place wraith.exe and wraith.ini in repo root before building.

!include "MUI2.nsh"
!include "x64.nsh"

; -- Metadata ---------------------------------------------------------------
; VERSION is passed via: makensis /DVERSION=1.2.3 installer\wraith.nsi
; Falls back to "dev" if not provided (local builds).
!ifndef VERSION
  !define VERSION "dev"
!endif

Name              "Wraith"
OutFile           "wraith-setup.exe"
InstallDir        "$PROGRAMFILES64\Wraith"
InstallDirRegKey  HKCU "Software\Wraith" "InstallDir"
RequestExecutionLevel admin
SetCompressor     /SOLID lzma
Unicode           True

; -- MUI Pages ---------------------------------------------------------------
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN          "$INSTDIR\wraith.exe"
!define MUI_FINISHPAGE_RUN_TEXT     "Launch Wraith"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; -- Install section ---------------------------------------------------------
Section "Wraith" SecMain
    SectionIn RO   ; required

    SetOutPath "$INSTDIR"
    ; File paths relative to CWD at makensis invocation (repo root)
    File "wraith.exe"
    File /nonfatal "wraith.ini"   ; ship default config; /nonfatal if missing

    ; Start Menu
    CreateDirectory "$SMPROGRAMS\Wraith"
    CreateShortCut  "$SMPROGRAMS\Wraith\Wraith.lnk" "$INSTDIR\wraith.exe"
    CreateShortCut  "$SMPROGRAMS\Wraith\Uninstall Wraith.lnk" "$INSTDIR\Uninstall.exe"

    ; Write uninstaller
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    ; Registry: Add/Remove Programs entry
    WriteRegStr   HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wraith" \
                       "DisplayName"     "Wraith"
    WriteRegStr   HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wraith" \
                       "DisplayVersion"  "${VERSION}"
    WriteRegStr   HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wraith" \
                       "Publisher"       "shadow-dragon-2002"
    WriteRegStr   HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wraith" \
                       "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegStr   HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wraith" \
                       "URLInfoAbout"    "https://github.com/shadow-dragon-2002/Wraith"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wraith" \
                       "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wraith" \
                       "NoRepair"  1

    ; Remember install dir
    WriteRegStr HKCU "Software\Wraith" "InstallDir" "$INSTDIR"
SectionEnd

; -- Uninstall section -------------------------------------------------------
Section "Uninstall"
    ; Kill running instance first
    ExecWait 'taskkill /F /IM wraith.exe' $0

    Delete "$INSTDIR\wraith.exe"
    Delete "$INSTDIR\wraith.ini"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir  "$INSTDIR"

    Delete "$SMPROGRAMS\Wraith\Wraith.lnk"
    Delete "$SMPROGRAMS\Wraith\Uninstall Wraith.lnk"
    RMDir  "$SMPROGRAMS\Wraith"

    ; Remove auto-start entry if it was set
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Wraith"

    ; Clear DisableTaskMgr in case Wraith was locked when uninstalled
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Policies\System" "DisableTaskMgr"

    ; Remove uninstall entry and app registry key
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wraith"
    DeleteRegKey HKCU "Software\Wraith"
SectionEnd
