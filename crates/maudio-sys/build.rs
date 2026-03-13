use std::env;
use std::path::PathBuf;

use cc::Build;

#[cfg(feature = "generate-bindings")]
fn write_bindings(out_bindings: &std::path::Path) {
    let mut builder = bindgen::Builder::default()
        .header("native/miniaudio/miniaudio.h")
        .clang_arg("-Inative")
        .clang_arg("-Inative/miniaudio")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .use_core()
        .ctypes_prefix("core::ffi");

    if !cfg!(feature = "vorbis") {
        builder = builder.clang_arg("-DMA_NO_VORBIS=1");
    }

    let bindings = builder.generate().expect("Unable to generate bindings");
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

fn backend_features(builder: &mut Build) {
    if cfg!(feature = "no-wasapi") {
        builder.define("MA_NO_WASAPI", "1");
    }
    if cfg!(feature = "no-dsound") {
        builder.define("MA_NO_DSOUND", "1");
    }
    if cfg!(feature = "no-winmm") {
        builder.define("MA_NO_WINMM", "1");
    }
    if cfg!(feature = "no-alsa") {
        builder.define("MA_NO_ALSA", "1");
    }
    if cfg!(feature = "no-pulseaudio") {
        builder.define("MA_NO_PULSEAUDIO", "1");
    }
    if cfg!(feature = "no-jack") {
        builder.define("MA_NO_JACK", "1");
    }
    if cfg!(feature = "no-coreaudio") {
        builder.define("MA_NO_COREAUDIO", "1");
    }
    if cfg!(feature = "no-sndio") {
        builder.define("MA_NO_SNDIO", "1");
    }
    if cfg!(feature = "no-audio4") {
        builder.define("MA_NO_AUDIO4", "1");
    }
    if cfg!(feature = "no-oss") {
        builder.define("MA_NO_OSS", "1");
    }
    if cfg!(feature = "no-aaudio") {
        builder.define("MA_NO_AAUDIO", "1");
    }
    if cfg!(feature = "no-opensl") {
        builder.define("MA_NO_OPENSL", "1");
    }
    if cfg!(feature = "no-webaudio") {
        builder.define("MA_NO_WEBAUDIO", "1");
    }
}

fn main() {
    if cfg!(feature = "generate-bindings") {
        let minor = rustc_minor().unwrap_or(0);
        if minor < 70 {
            panic!("feature `generate-bindings` requires rustc >= 1.70");
        }
    }

    println!("cargo:rerun-if-changed=native/miniaudio.c");
    println!("cargo:rerun-if-changed=native/miniaudio/miniaudio.h");
    println!("cargo:rerun-if-changed=native/miniaudio/extras/stb_vorbis.c");
    #[cfg(windows)]
    println!("cargo:rerun-if-changed=src/src/pregen_bindings/windows.rs");
    #[cfg(unix)]
    println!("cargo:rerun-if-changed=src/src/pregen_bindings/unix.rs");

    let mut cc_builder = cc::Build::new();

    if cfg!(feature = "vorbis") {
        // stb_vorbis.c is added by miniaudio.c
        cc_builder.define("MAUDIO_ENABLE_VORBIS", "1");
    } else {
        cc_builder.define("MA_NO_VORBIS", "1");
    }

    // backend features
    backend_features(&mut cc_builder);

    cc_builder
        .file("native/miniaudio_version_check.c")
        .file("native/miniaudio.c")
        .include("native")
        .flag_if_supported("-Wno-maybe-uninitialized")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-function")
        .compile("miniaudio");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_bindings = out_path.join("bindings.rs");

    write_bindings(&out_bindings);
}

// Checks the rustc version when building with generate-bindings feature.
fn rustc_minor() -> Option<u32> {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let out = std::process::Command::new(rustc).arg("-vV").output().ok()?;
    let s = std::string::String::from_utf8(out.stdout).ok()?;

    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("release: ") {
            let mut it = rest.split('.');
            let _major = it.next()?.parse::<u32>().ok()?;
            let minor = it.next()?.parse::<u32>().ok()?;
            return Some(minor);
        }
    }
    None
}
