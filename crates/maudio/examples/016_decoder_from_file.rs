use std::path::PathBuf;

use maudio::{
    audio::sample_rate::SampleRate, data_source::sources::decoder::DecoderBuilder, engine::Engine,
    MaResult,
};

fn main() -> MaResult<()> {
    let engine = Engine::new()?;

    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let file = std::fs::File::open(&path).unwrap();

    let decoder = DecoderBuilder::new_f32(2, SampleRate::Sr44100).from_reader(file)?;

    let mut sound = engine.new_sound_from_source(&decoder)?;

    sound.play_sound()?;
    println!("Stopping in 5 seconds...");
    std::thread::sleep(std::time::Duration::from_secs(5));
    sound.stop_sound()?;

    Ok(())
}
