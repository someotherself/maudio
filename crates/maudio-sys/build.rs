use std::env;
use std::path::PathBuf;

#[allow(unused)]
const MINIAUDIO_VERSION: &str = "0.11.23";

// Generates new bindings and writes them
// Used either by the "generate-bindigns" flag
// or when pregenerated bindings don't exist for this target
#[cfg(feature = "generate-bindings")]
fn write_bindings(out_bindings: &std::path::Path) {
    let mut builder = bindgen::Builder::default()
        .header("native/miniaudio/miniaudio.h")
        .clang_arg("-Inative")
        .clang_arg("-Inative/miniaudio")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .use_core()
        .ctypes_prefix("core::ffi");

    let target = std::env::var("TARGET").unwrap();

    if target == "wasm32-unknown-emscripten" {
        let emsdk = std::env::var("EMSDK").expect("EMSDK must be set");
        let sysroot = std::path::Path::new(&emsdk).join("upstream/emscripten/cache/sysroot");

        builder = builder
            .clang_arg(format!("--sysroot={}", sysroot.display()))
            .clang_arg("-iwithsysroot/include/fakesdl")
            .clang_arg("-iwithsysroot/include/compat")
            .clang_arg("-fvisibility=default")
            .blocklist_type("max_align_t");
    } else if !target.contains("apple-ios") {
        builder = builder.clang_arg("-std=c99"); // stb_vorbis is c99
    }

    if !cfg!(feature = "vorbis") {
        builder = builder.clang_arg("-DMA_NO_VORBIS=1");
    }

    if !cfg!(feature = "engine") {
        builder = builder.clang_arg("-DMA_NO_ENGINE=1");
    }

    let bindings = builder.generate().expect("Unable to generate bindings");
    bindings
        .write_to_file(out_bindings)
        .expect("Couldn't write bindings.rs");
}

// Checks the current target and if pregenerated bindings already exist
#[cfg(not(feature = "generate-bindings"))]
fn write_bindings(out_bindings: &std::path::Path) {
    let target = env::var("TARGET").expect("Cargo did not provide the TARGET environment variable");

    let ver_folder =
        PathBuf::from("src/pregen_bindings").join(format!("miniaudio-{MINIAUDIO_VERSION}"));
    if !ver_folder.is_dir() {
        panic!(
            "no compatible pregenerated bindings were found for version `{}`",
            ver_folder.display()
        )
    }
    let pregenerated = ver_folder.join(&target).with_extension("rs");

    println!("cargo:rerun-if-changed={}", pregenerated.display());

    if pregenerated.is_file() {
        eprintln!(
            "Using pre-generated bindings for target `{target}` from `{}`",
            pregenerated.display()
        );

        std::fs::copy(&pregenerated, out_bindings)
            .expect("Failed to copy pre-generated bindings to OUT_DIR");
    } else {
        panic!("No pre-generated bindings found for target `{target}`; generating bindings. Use the generate-bindings feature");
    }
}

#[cfg(feature = "supplybin")]
fn link_user_supplied_miniaudio() {
    const LIB_ROOT: &str = "MAUDIO_EXTERNAL_LIB_DIR";

    println!("cargo:rerun-if-env-changed={LIB_ROOT}");

    let root = PathBuf::from(env::var_os(LIB_ROOT).expect(
        "`supplybin` requires MAUDIO_EXTERNAL_LIB_DIR to point \
         to the root directory containing the precompiled libraries",
    ));

    let target = env::var("TARGET").expect("Cargo did not provide the TARGET environment variable");

    let ver_folder = root.join(format!("miniaudio-{MINIAUDIO_VERSION}"));
    if !ver_folder.is_dir() {
        panic!(
            "no compatible libraries were found for version `{}`",
            ver_folder.display()
        )
    }

    let lib_dir = ver_folder.join(&target);

    let filename = if target.contains("windows-msvc") {
        "miniaudio.lib"
    } else {
        "libminiaudio.a"
    };

    let library = lib_dir.join(filename);

    if !library.is_file() {
        panic!(
            "could not find the precompiled miniaudio library for target \
            `{target}` at `{}`",
            library.display()
        );
    }

    if !lib_dir.is_dir() {
        panic!(
            "no precompiled miniaudio library was supplied for target `{target}`; \
             expected directory `{}`",
            lib_dir.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=miniaudio");
}

#[cfg(not(feature = "supplybin"))]
fn backend_features(builder: &mut cc::Build) {
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
    #[cfg(feature = "supplybin")]
    link_user_supplied_miniaudio();

    #[cfg(not(feature = "supplybin"))]
    {
        println!("cargo:rerun-if-changed=native/miniaudio.c");
        println!("cargo:rerun-if-changed=native/miniaudio/miniaudio.h");
        println!("cargo:rerun-if-changed=native/miniaudio/extras/stb_vorbis.c");

        let mut cc_builder = cc::Build::new();

        if cfg!(feature = "vorbis") {
            // stb_vorbis.c is added by miniaudio.c
            cc_builder.define("MAUDIO_ENABLE_VORBIS", "1");
        } else {
            cc_builder.define("MA_NO_VORBIS", "1");
        }

        if !cfg!(feature = "engine") {
            cc_builder.define("MA_NO_ENGINE", "1");
        }

        // backend features
        backend_features(&mut cc_builder);

        let target =
            env::var("TARGET").expect("Cargo did not provide the TARGET environment variable");

        if target == "wasm32-unknown-emscripten" {
            if cfg!(feature = "worklet") {
                cc_builder.define("MA_ENABLE_AUDIO_WORKLETS", "1");

                println!("cargo:rustc-link-arg=-sAUDIO_WORKLET=1");
                println!("cargo:rustc-link-arg=-sWASM_WORKERS=1");
                println!("cargo:rustc-link-arg=-sASYNCIFY");
            }

            let emsdk = env::var("EMSDK").expect("EMSDK must be set");
            let sysroot = std::path::Path::new(&emsdk).join("upstream/emscripten/cache/sysroot");

            cc_builder
                .flag("-fwasm-exceptions")
                .flag("-fvisibility=default")
                .flag(format!("--sysroot={}", sysroot.display()));
        } else if !target.contains("apple-ios") {
            cc_builder.std("c99"); // stb_vorbis is c99
        }

        cc_builder
            .file("native/miniaudio_version_check.c")
            .file("native/miniaudio.c")
            .include("native")
            .flag_if_supported("-Wno-maybe-uninitialized")
            .flag_if_supported("-Wno-unused-parameter")
            .flag_if_supported("-Wno-unused-function")
            .compile("miniaudio");
    }

    let out_path = PathBuf::from(
        env::var("OUT_DIR").expect("Cargo did not provide the OUT_DIR environment variable"),
    );
    let out_bindings = out_path.join("bindings.rs");

    write_bindings(&out_bindings);
}
