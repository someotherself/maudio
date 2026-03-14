use std::{path::PathBuf, thread, time::Duration};

use maudio::{
    engine::Engine,
    sound::{sound_builder::SoundBuilder, sound_flags::SoundFlags},
    MaResult,
};

// Thread-safety in maudio
//
// Unless specifically stated otherwise, you should assume that a type is not thread-safe
//
// - Do not share objects like `Engine`, `Sound`, or `SoundGroup` between
//   threads.
// - Do not wrap these objects in `Mutex`, `RwLock`, or other locking
//   primitives.
// - Avoid performing blocking operations in audio-related code.
//
// The underlying audio engine runs a real-time audio thread. Introducing
// locks or cross-thread contention can cause dropouts or undefined behavior.
//
// If you need to control a type from another thread, prefer using channels,
// or use one of the thread safe API's offered by maudio (explained later)

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
