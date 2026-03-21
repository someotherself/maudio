use std::path::PathBuf;

use maudio::{
    engine::Engine,
    sound::{sound_builder::SoundBuilder, Sound},
    MaResult,
};

// Demonstrates a simple sound pool for reusing sounds.
//
// In a game, short sound effects such as footsteps or gunshots may be
// triggered frequently. Creating a new sound every time can be inefficient,
// so a pool of pre-created sounds can be reused instead.
//
// The pool reuses sound instances, while the underlying audio data is managed
// internally by the engine (the ResourceManager). Multiple sounds created from the same file do not
// require the file to be loaded in memory multiple times.
//
// This example requires the `vorbis` feature to run

fn main() -> MaResult<()> {
    let engine = Engine::new()?;

    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/48000-stereo.ogg"
    ));

    // Create a small pool of reusable sounds.
    let mut pool: Vec<Sound> = (0..4)
        .map(|_| SoundBuilder::new(&engine).file_path(&path).build())
        .collect::<MaResult<_>>()?;

    let mut index = 0;

    // Simulate triggering sounds repeatedly.
    for _ in 0..10 {
        let sound = &mut pool[index];

        // Restart and play the sound.
        sound.seek_to_frame(0)?;
        sound.play_sound()?;

        index = (index + 1) % pool.len();

        std::thread::sleep(std::time::Duration::from_millis(300));
    }

    Ok(())
}
