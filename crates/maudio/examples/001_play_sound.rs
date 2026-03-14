use std::{io::Write, path::PathBuf};

use maudio::engine::Engine;

fn main() {
    // The high level API is centered around an `Engine`.
    // Internally, (among other things), the engine manages the playback device
    // and the internal audio graph used by all sound sources.
    // `Engine::new` provides an easy way to create an engine with sensible defaults
    let engine = Engine::new().unwrap();

    // For these purposes, we will use a file path as audio source
    // To access this sound file, you will need to use git clone using `--recursive` flag.
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    // A Sound represents a playable audio source.
    // The Engine provides a few methods for quickly creating a Sound.
    // See also:
    // `Engine::new_sound`
    // `Engine::new_sound_from_file`
    // `Engine::new_sound_from_source`
    // `Engine::new_sound_from_file_with_flags`
    // `Engine::clone_sound`
    let mut sound = engine.new_sound_from_file(&path).unwrap();

    // A Sound needs to be started manually.
    sound.play_sound().unwrap();

    // Audio playback happens asynchronously.
    // We must block the current thread, or else the program will exit early
    wait_and_play();
}

fn wait_and_play() {
    print!("Press Enter to quit...");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
}
