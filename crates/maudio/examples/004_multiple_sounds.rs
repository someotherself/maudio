use std::path::PathBuf;

use maudio::{engine::Engine, sound::sound_builder::SoundBuilder, MaResult};

// This example must be ran with the `vorbis` feature

// Demonstrates creating and playing multiple sounds.
// Each sound is independent and can be controlled individually.
// The engine automatically mixes all active sounds together.

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let path1 = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let path2 = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/48000-stereo.ogg"
    ));
    // Create two independent sounds.
    let mut sound1 = SoundBuilder::new(&engine).file_path(&path1).build()?;

    let mut sound2 = SoundBuilder::new(&engine).file_path(&path2).build()?;

    println!("Playing first sound...");
    sound1.play_sound()?;

    // Let it play briefly before starting the second.
    std::thread::sleep(std::time::Duration::from_secs(2));

    println!("Playing second sound...");
    sound2.play_sound()?;

    // Let both play together.
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Stop first sound
    sound1.stop_sound()?;

    std::thread::sleep(std::time::Duration::from_secs(5));

    Ok(())
}
