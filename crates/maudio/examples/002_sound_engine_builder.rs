use std::{io::Write, path::PathBuf};

use maudio::{audio::sample_rate::SampleRate, engine::engine_builder::EngineBuilder, MaResult};

// `maudio` uses a crate-wide result type called `MaResult<T>`.
//
// This is just a type alias for `Result<T, MaudioError>` and is used by all
// constructors and operations in the crate.
//
// `MaudioError` itself is a struct with 2 members:
// - `ma_result`: The underlying error code returned by miniaudio.
//                This value always exists and corresponds to the
//                `MA_RESULT` enum from the C library.
//
// - `native`   : An optional wrapper-level error defined by `maudio`.
//                These errors are produced by additional validation or
//                safety checks performed by the maudio.
//
// If `native` is `None`, the error originated directly from miniaudio.
// If `native` is `Some(...)`, the error was produced by the maudio
//
// Returning `MaResult<()>` from `main` makes examples easier to write because
// fallible operations can use `?` directly.

fn main() -> MaResult<()> {
    // `EngineBuilder` provides a customizable way to configure an engine
    // before it is created.
    // Almost all the types available in maudio can be created using a Builder.
    let engine = EngineBuilder::new()
        .no_auto_start(true)
        .set_sample_rate(SampleRate::Sr48000)
        .build()?;

    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    // By default, the Engine is started automatically, however
    // with `no_auto_start`, we need to start it manually.
    engine.start()?;

    let mut sound = engine.new_sound_from_file(&path)?;

    sound.play_sound()?;

    wait_and_play();
    Ok(())
}

fn wait_and_play() {
    print!("Press Enter to quit...");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
}
