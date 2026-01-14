use std::env;
use std::path::PathBuf;

#[cfg(feature = "generate-bindings")]
fn write_bindings(out_bindings: &std::path::Path) {
    let bindings = bindgen::Builder::default()
        .header("native/miniaudio/miniaudio.h")
        .clang_arg("-Inative/miniaudio")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .use_core()
        .ctypes_prefix("core::ffi")
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_bindings)
        .expect("Couldn't write bindings.rs");
}

#[cfg(not(feature = "generate-bindings"))]
fn write_bindings(out_bindings: &std::path::Path) {
    eprintln!("Copying bindings");
    #[cfg(unix)]
    std::fs::copy("src/pregen_bindings/unix.rs", out_bindings)
        .expect("Failed to copy pre-generated bindings to OUT_DIR");
    #[cfg(windows)]
    std::fs::copy("src/pregen_bindings/windows.rs", out_bindings)
        .expect("Failed to copy pre-generated bindings to OUT_DIR");
}

fn main() {
    println!("cargo:rerun-if-changed=native/miniaudio/miniaudio.c");
    println!("cargo:rerun-if-changed=native/miniaudio/miniaudio.h");
    #[cfg(windows)]
    println!("cargo:rerun-if-changed=src/src/pregen_bindings/windows.rs");
    #[cfg(unix)]
    println!("cargo:rerun-if-changed=src/src/pregen_bindings/unix.rs");

    cc::Build::new()
        .file("native/miniaudio_version_check.c")
        .file("native/miniaudio/miniaudio.c")
        .include("native")
        .flag_if_supported("-Wno-maybe-uninitialized")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-function")
        .compile("miniaudio");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_bindings = out_path.join("bindings.rs");

    write_bindings(&out_bindings);
}
