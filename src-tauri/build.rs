// libnxrt_manage (nxrt/manage/nxrt_manage.h), compiled from the nxrt submodule - or the checkout NXRT_DIR names.
use std::path::PathBuf;

fn main() {
    tauri_build::build();

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nxrt = std::env::var_os("NXRT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("../nxrt"));
    let src = [
        nxrt.join("manage/nxrt_manage.c"),
        nxrt.join("runtime/nxpkg.c"),
        nxrt.join("runtime/lz4_block.c"),
    ];
    for f in &src {
        assert!(
            f.exists(),
            "{} missing: git submodule update --init (or set NXRT_DIR)",
            f.display()
        );
        println!("cargo:rerun-if-changed={}", f.display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        nxrt.join("manage/nxrt_manage.h").display()
    );
    println!("cargo:rerun-if-env-changed=NXRT_DIR");
    cc::Build::new()
        .files(&src)
        .include(nxrt.join("manage"))
        .include(nxrt.join("runtime"))
        .flag_if_supported("-std=c11")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-mmacosx-version-min=11.0")
        .opt_level(2)
        .compile("nxrt_manage");

    // SDL3 (controllers, built from source) uses @available in its Objective-C, which calls
    // ___isPlatformVersionAtLeast from clang's runtime; Rust links with -nodefaultlibs, so the runtime is named here.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let dir = std::process::Command::new("xcrun")
            .args(["clang", "-print-resource-dir"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()).join("lib/darwin"));
        match dir {
            Some(d) if d.join("libclang_rt.osx.a").exists() => {
                println!("cargo:rustc-link-search=native={}", d.display());
                println!("cargo:rustc-link-lib=static=clang_rt.osx");
            }
            _ => println!(
                "cargo:warning=clang's runtime (libclang_rt.osx.a) not found: SDL3 may not link"
            ),
        }
    }
}
