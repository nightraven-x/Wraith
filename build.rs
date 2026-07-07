fn main() {
    println!("cargo:rerun-if-changed=src/resource.rc");
    println!("cargo:rerun-if-changed=wraith.manifest");
    println!("cargo:rerun-if-changed=assets/unlocked-white.ico");
    println!("cargo:rerun-if-changed=assets/locked-white.ico");
    println!("cargo:rustc-link-lib=winhttp");
    println!("cargo:rustc-link-lib=dwmapi");
    println!("cargo:rustc-link-lib=uxtheme");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::env::var("OUT_DIR").unwrap();
        let obj = format!("{out}/resource.o");
        let windres = std::env::var("WINDRES")
            .unwrap_or_else(|_| "x86_64-w64-mingw32-windres".to_string());

        let ok = std::process::Command::new(&windres)
            .args(["src/resource.rc", "-o", &obj])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if ok {
            println!("cargo:rustc-link-arg={obj}");
        } else {
            println!("cargo:warning=windres failed or not found; resource embedding skipped");
        }
    }
}
