use std::path::PathBuf;

use maudio::{
    engine::Engine,
    sound::{sound_builder::SoundBuilder, sound_group::SoundGroupBuilder},
    MaResult,
};

// Demonstrates using sound groups as mix buses, similar to a game engine.
//
// A typical game, sounds are organized into categories such as music,
// sound effects, dialogue, or UI. Each category is assigned to its own
// sound group so it can be mixed and controlled independently.
//
// For example, lowering the music volume should not affect explosion or
// footstep sounds. Sound groups make this kind of category-based mixing
// straightforward.
//
// This example is for demonstration purposes and only mixes in 2 sounds.
//
// Requires the `vorbis` feature to run

fn main() -> MaResult<()> {
    let engine = Engine::new()?;

    let music_path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let sfx_path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/48000-stereo.ogg"
    ));

    // Create two groups representing separate mix buses.
    let mut music_group = SoundGroupBuilder::new(&engine).build()?;
    let mut sfx_group = SoundGroupBuilder::new(&engine).build()?;

    // Set different base levels for each group.
    music_group.set_volume(0.3);
    sfx_group.set_volume(1.0);

    let mut music = SoundBuilder::new(&engine)
        .sound_group(&music_group)
        .file_path(&music_path)
        .build()?;

    let mut sfx1 = SoundBuilder::new(&engine)
        .sound_group(&sfx_group)
        .file_path(&sfx_path)
        .build()?;

    let mut sfx2 = SoundBuilder::new(&engine)
        .sound_group(&sfx_group)
        .file_path(&sfx_path)
        .build()?;

    // Individual sound volume still applies inside the group mix.
    sfx2.set_volume(0.5);

    println!("Starting music and sound effects...");
    music.play_sound()?;
    sfx1.play_sound()?;

    std::thread::sleep(std::time::Duration::from_secs(2));

    println!("Starting a second sound effect at lower individual volume...");
    sfx2.play_sound()?;

    std::thread::sleep(std::time::Duration::from_secs(2));

    println!("Lowering only the music group...");
    music_group.set_volume(0.1);

    std::thread::sleep(std::time::Duration::from_secs(2));

    println!("Muting sound effects group...");
    sfx_group.set_volume(0.0);

    std::thread::sleep(std::time::Duration::from_secs(2));

    Ok(())
}
