# Research 10 — Resource Embedding

Verified facts for `src/resource.rc`, `build.rs`, and `wraith.manifest`.

---

## windres

Binary name when `gcc-mingw-w64-x86-64` is installed:
```
x86_64-w64-mingw32-windres
```
Included in the `gcc-mingw-w64-x86-64` package alongside `x86_64-w64-mingw32-gcc` and `x86_64-w64-mingw32-ar`. All in PATH after `sudo apt-get install -y gcc-mingw-w64-x86-64`.

**Output format:** `x86_64-w64-mingw32-windres` targets **pe-x86-64 by default** (unlike generic Linux `windres` which defaults to ELF). No `--target` flag needed for the MinGW cross-compiler.

```bash
# Correct invocation from build.rs:
x86_64-w64-mingw32-windres src/resource.rc -o target/resource.o
# -F pe-x86-64 is redundant but explicit (alias: --target=pe-x86-64)
```

Default output without `-o`: prints 'rc' text format to stdout (not ELF or PE — stdout is text). Always specify `-o resource.o`.

## Linking the Object File

```rust
// In build.rs:
println!("cargo:rustc-link-arg=path/to/resource.o");
// Or use OUT_DIR:
let out = env::var("OUT_DIR").unwrap();
println!("cargo:rustc-link-arg={}/resource.o", out);
```

- Additive with existing rustflags (`-Wl,--subsystem,windows`) — no conflict
- If windres fails, `build.rs` should emit a clear panic message
- `cargo:rerun-if-changed=src/resource.rc` — path relative to Cargo.toml root

## RT_MANIFEST Resource

```rc
// resource.rc
1 RT_MANIFEST "wraith.manifest"
```

- Resource ID 1 = application manifest (correct ID)
- Embedded manifest takes precedence over sidecar `.manifest` file
- MSDN: "If possible, you should embed the application manifest as a resource in your application's .exe file"
- Do NOT ship a sidecar `wraith.exe.manifest` alongside the binary

## VERSIONINFO Resource

```rc
VS_VERSION_INFO VERSIONINFO
FILEVERSION     1,0,0,0
PRODUCTVERSION  1,0,0,0
FILEFLAGSMASK   0x3fL
FILEFLAGS       0x0L
FILEOS          VOS__WINDOWS32
FILETYPE        VFT_APP
FILESUBTYPE     VFT_UNKNOWN
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"  // 0409=en-US, 04B0=Unicode(1200)
        BEGIN
            VALUE "CompanyName",      "shadow-dragon-2002"
            VALUE "FileDescription",  "Wraith Input Blocker"
            VALUE "FileVersion",      "1.0.0.0"
            VALUE "InternalName",     "wraith"
            VALUE "OriginalFilename", "wraith.exe"
            VALUE "ProductName",      "Wraith"
            VALUE "ProductVersion",   "1.0.0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
```

- `040904B0`: `0409` = English US, `04B0` = Unicode code page 1200
- `FILEVERSION` takes four u16 ints: `major,minor,patch,build`
- Minimum required for Explorer to show version: `FileDescription` + `FileVersion`

## wraith.manifest — UAC + DPI

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0"
          xmlns:asmv3="urn:schemas-microsoft-com:asm.v3">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="amd64"
                    name="shadow-dragon-2002.Wraith" type="win32"/>
  <description>Wraith Input Blocker</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <asmv3:application>
    <asmv3:windowsSettings>
      <!-- PerMonitorV2 for Win10 1607+; dpiAware for older Windows -->
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">
        PerMonitorV2
      </dpiAwareness>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">
        True/PM
      </dpiAware>
    </asmv3:windowsSettings>
  </asmv3:application>
</assembly>
```

- `asInvoker` = no UAC elevation prompt — correct for Wraith
- `WH_KEYBOARD_LL` / `WH_MOUSE_LL` do NOT require elevation
- DPI awareness: both `dpiAwareness` (new, Win10 1607+) and `dpiAware` (legacy) for full coverage

## Icon Resource ID and MAKEINTRESOURCEW

```rc
1 ICON "assets/wraith_unlocked.ico"
2 ICON "assets/wraith_locked.ico"
```

```rust
// In Rust (windows-sys doesn't export MAKEINTRESOURCEW):
let icon = LoadImageW(hinstance, 1 as *const u16, IMAGE_ICON, 16, 16, LR_DEFAULTCOLOR);
// Equivalent to MAKEINTRESOURCEW(1) = 1 as usize as *const u16
```

ICO file should include: 16x16 (system tray), 32x32 (Alt+Tab, Explorer), optionally 48x48 and 256x256.

## DPI Awareness for Tray Icon

HWND_MESSAGE window has no visible rendering — DPI is irrelevant for the window itself.
System tray: OS scales the icon automatically if only 16x16 is provided on high-DPI displays.
Including 32x32 in the ICO gives OS better material to scale from.
