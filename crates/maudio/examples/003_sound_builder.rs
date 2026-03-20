use std::{path::PathBuf, thread, time::Duration};

use maudio::{
    engine::Engine,
    sound::{sound_builder::SoundBuilder, sound_flags::SoundFlags},
    MaResult,
};

// Thread-safety in maudio
//
// Unless documented otherwise, assume a maudio type is intended to be used from
// one controlling thread at a time.
//
// Putting a type behind `Mutex`/`RwLock` can serialize Rust-side access, but it
// does not by itself make the underlying audio object safe for arbitrary
// cross-thread use, nor does it make it safe for real-time audio callbacks.
//
// In particular, avoid taking locks from audio callbacks or other real-time
// paths. Blocking the audio thread can cause glitches or dropouts.
//
// For cross-thread control, prefer dedicated control threads, channels, atomics, fences,
// or dedicated thread-safe APIs when available.

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    // `SoundBuilder` provides a customizable way to configure a `Sound`
    // before it is created. This is useful when additional options are
    // needed, such as looping behavior, sound groups, or loading flags.
    let mut sound = SoundBuilder::new(&engine)
        .file_path(&path)
        // Enable looping so the sound will automatically restart when it
        // reaches the end of the file.
        .looping(true)
        // Configure sound loading flags. `STREAM` tells the engine to stream
        // audio from disk instead of loading the entire file into memory.
        // This is recommended for large audio files such as music tracks.
        //
        // This is equivalent to using `.streaming(true)`.
        .flags(SoundFlags::STREAM)
        .build()?;

    // Sounds can be configured further after creation and before they start playing
    sound.set_volume(0.25);

    sound.play_sound()?;

    println!("Stopping in 5 seconds...");
    thread::sleep(Duration::from_secs(5));

    sound.stop_sound()?;
    Ok(())
}
