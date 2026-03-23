use std::path::PathBuf;

use maudio::{
    audio::sample_rate::SampleRate,
    encoder::EncoderBuilder,
    engine::{engine_builder::EngineBuilder, EngineOps},
    sound::sound_builder::SoundBuilder,
    MaResult,
};

fn main() -> MaResult<()> {
    let seconds = 5;
    let sample_rate = SampleRate::Sr48000;
    let channels = 2;
    let chunk_frames = 1024;

    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let dst_path = PathBuf::from("crates/maudio/examples/template_output.wav");
    let _ = std::fs::remove_file(&dst_path);
    std::fs::File::create(&dst_path).unwrap();

    let engine = EngineBuilder::new().no_device(2, sample_rate).build()?;

    let mut encoder = EncoderBuilder::new_f32(channels, sample_rate)
        .wav()
        .build_file(&dst_path)?;

    let mut sound = SoundBuilder::new(&engine).file_path(&path).build()?;

    sound.play_sound()?;

    let total_frames = 48_000 * seconds * channels;
    let mut frames_remaining = total_frames;

    while frames_remaining > 0 {
        let frames_to_read = frames_remaining.min(chunk_frames) as u64;

        let buffer = engine.read_pcm_frames(frames_to_read)?;

        if buffer.is_empty() {
            break;
        }

        let buf = buffer.as_ref();
        let written = encoder.write_pcm_frames(&buf[..frames_to_read as usize])?;

        frames_remaining -= written as u32;
    }

    Ok(())
}
