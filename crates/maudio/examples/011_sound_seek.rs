use std::path::PathBuf;

use maudio::{engine::Engine, sound::sound_builder::SoundBuilder, MaResult};

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let mut sound = SoundBuilder::new(&engine).file_path(&path).build()?;

    // Start playing from the beginning.
    sound.play_sound()?;

    // Skip ahead to 5 seconds into the sound.
    sound.seek_to_second(5.0)?;
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Seek to an exact PCM frame position.
    // At 44_100 Hz, this is about 1 second from the start.
    sound.seek_to_frame(44_100)?;
    std::thread::sleep(std::time::Duration::from_secs(2));

    Ok(())
}
