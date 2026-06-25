# Spec 10 — Resource Embedding

> Research prerequisite: [../research/10-resources.md](../research/10-resources.md)
> Implements: `src/resource.rc`, `wraith.manifest`, `build.rs` — Step 9

---

## Purpose

Embed icons, version information, and a UAC manifest into the `.exe` using a Windows
resource file. Compiled via `x86_64-w64-mingw32-windres` and linked in via `build.rs`.

---

## Files

```
src/resource.rc       Windows resource script
wraith.manifest       UAC + DPI awareness manifest
assets/
  wraith_locked.ico   Locked-state tray icon
  wraith_unlocked.ico Unlocked-state tray icon (default)
build.rs              Compiles resource.rc and tells cargo to link the .o
```

---

## resource.rc

```rc
#include <windows.h>

// Icons
1 ICON "assets/wraith_unlocked.ico"
2 ICON "assets/wraith_locked.ico"

// Manifest
1 RT_MANIFEST "wraith.manifest"

// Version info
VS_VERSION_INFO VERSIONINFO
FILEVERSION     1,0,0,0
PRODUCTVERSION  1,0,0,0
FILEFLAGSMASK   VS_FFI_FILEFLAGSMASK
FILEFLAGS       0
FILEOS          VOS_NT_WINDOWS32
FILETYPE        VFT_APP
FILESUBTYPE     VFT2_UNKNOWN
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"   // en-US, Unicode
        BEGIN
            VALUE "CompanyName",      "shadow-dragon-2002"
            VALUE "FileDescription",  "Wraith Input Blocker"
            VALUE "FileVersion",      "1.0.0.0"
            VALUE "InternalName",     "wraith"
            VALUE "LegalCopyright",   "PolyForm Noncommercial 1.0.0"
            VALUE "OriginalFilename", "wraith.exe"
            VALUE "ProductName",      "Wraith"
            VALUE "ProductVersion",   "1.0.0.0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
```

---

## wraith.manifest

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity
      version="1.0.0.0"
      processorArchitecture="amd64"
      name="shadow-dragon-2002.Wraith"
      type="win32"/>
  <description>Wraith Input Blocker</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" onError="abort"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">
        PerMonitorV2
      </dpiAwareness>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">
        True/PM
      </dpiAware>
    </windowsSettings>
  </application>
</assembly>
```

**R1.** `requestedExecutionLevel = "asInvoker"` — no UAC elevation prompt. Wraith
runs at the user's normal privilege level. `WH_KEYBOARD_LL` / `WH_MOUSE_LL` do not
require elevation.

**R2.** DPI: `PerMonitorV2` + `True/PM` for correct icon scaling on high-DPI displays.

---

## build.rs

**R3.**
```rust
fn main() {
    println!("cargo:rerun-if-changed=src/resource.rc");
    println!("cargo:rerun-if-changed=wraith.manifest");
    println!("cargo:rerun-if-changed=assets/wraith_unlocked.ico");
    println!("cargo:rerun-if-changed=assets/wraith_locked.ico");

    let out = std::env::var("OUT_DIR").unwrap();
    let rc  = "src/resource.rc";
    let obj = format!("{}/resource.o", out);

    let status = std::process::Command::new("x86_64-w64-mingw32-windres")
        .args([rc, "-o", &obj])
        .status()
        .expect("windres not found — install gcc-mingw-w64-x86-64");

    assert!(status.success(), "windres failed");

    println!("cargo:rustc-link-arg={}", obj);
    println!("cargo:rustc-link-lib=winhttp");
}
```

**R4.** `cargo:rustc-link-lib=winhttp` ensures the WinHTTP library links correctly
on GNU targets (sometimes not auto-resolved).

**R5.** `cargo:rerun-if-changed` directives prevent unnecessary rebuilds.

---

## Icon Resource IDs

| ID | File | Use |
|----|------|-----|
| 1  | `wraith_unlocked.ico` | Default / unlocked tray state |
| 2  | `wraith_locked.ico`   | Locked tray state |

In `tray.rs`, load via:
```rust
LoadImageW(
    GetModuleHandleW(NULL),
    MAKEINTRESOURCEW(1),   // or 2 for locked
    IMAGE_ICON,
    16, 16,
    LR_DEFAULTCOLOR
)
```

---

## ICO File Requirements

**R6.** Each `.ico` must contain at minimum: 16x16 and 32x32 sizes.
Windows uses 16x16 for the notification area; 32x32 for Explorer if needed.

---

## Dependencies

- `tray.rs` — loads icons by resource ID
- `.cargo/config.toml` — `rustflags` must NOT also set `--subsystem,windows` via
  `link-arg` if `build.rs` is providing `rustc-link-arg` — verify no double-linking.
  Actually they are separate: subsystem flag vs object file link. Both are fine together.

---

## Edge Cases

- **windres not in PATH on CI:** The CI job installs `gcc-mingw-w64-x86-64` which
  includes `windres`. The `build.rs` panics with a clear message if missing.
- **windres path on Ubuntu vs other distros:** May be `x86_64-w64-mingw32-windres`
  or just `windres`. Use the full prefixed name for reliability.
- **Resource not found at runtime:** `LoadImageW` returns NULL → tray.rs falls back
  to `IDI_APPLICATION` (per spec 04 R5). Graceful degradation.
