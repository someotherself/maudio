use std::path::PathBuf;

use maudio::{
    audio::{
        math::vec3::Vec3,
        spatial::{attenuation::AttenuationModel, positioning::Positioning},
    },
    engine::Engine,
    sound::sound_builder::SoundBuilder,
    MaResult,
};

// Demonstrates basic sound spatialization.
// The sound is placed in 3D space and moved around the listener while playing.
// This affects how the sound is heard based on position and distance.

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let mut sound = SoundBuilder::new(&engine).file_path(&path).build()?;

    // Enable spatial positioning and place the sound to the left.
    sound.set_positioning(Positioning::Absolute);
    sound.set_attenuation(AttenuationModel::Inverse);
    sound.set_position(Vec3::new(-5.0, 0.0, 0.0));

    println!("Starting sound on the left...");
    sound.play_sound()?;
    std::thread::sleep(std::time::Duration::from_secs(2));

    println!("Moving sound in front...");
    sound.set_position(Vec3::new(0.0, 0.0, -2.0));
    std::thread::sleep(std::time::Duration::from_secs(2));

    println!("Moving sound to the right...");
    sound.set_position(Vec3::new(5.0, 0.0, 0.0));
    std::thread::sleep(std::time::Duration::from_secs(2));

    Ok(())
}
