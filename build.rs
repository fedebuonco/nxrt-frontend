// libnxrt_manage (nxrt/manage/nxrt_manage.h), compiled from the nxrt submodule - or the checkout NXRT_DIR names.
use std::path::PathBuf;

fn main() {
    let nxrt = std::env::var_os("NXRT_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("nxrt"));
    let src = [
        nxrt.join("manage/nxrt_manage.c"),
        nxrt.join("runtime/nxpkg.c"),
        nxrt.join("runtime/lz4_block.c"),
    ];
    for f in &src {
        assert!(f.exists(), "{} missing: git submodule update --init (or set NXRT_DIR)", f.display());
        println!("cargo:rerun-if-changed={}", f.display());
    }
    println!("cargo:rerun-if-changed={}", nxrt.join("manage/nxrt_manage.h").display());
    println!("cargo:rerun-if-env-changed=NXRT_DIR");
    cc::Build::new()
        .files(&src)
        .include(nxrt.join("manage"))
        .include(nxrt.join("runtime"))
        .flag_if_supported("-std=c11")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-mmacosx-version-min=11.0")  // (as the Rust code links: no newer-than-target warnings)
        .opt_level(2)
        .compile("nxrt_manage");
}
