use std::{thread, time::Duration};

use maudio::{
    audio::sample_rate::SampleRate, data_source::sources::decoder::DecoderBuilder, engine::Engine,
    MaResult,
};

// So far, we have created sounds directly from a file path. However,
// this is just a convenience. Internally, sounds can play from any
// object that provides a stream of decoded audio samples.
//
// These objects are referred to as *sound sources*. Examples include
// decoders, audio buffers, wave form generators or other custom data sources.
//
// In maudio, most source types implement the `AsSourcePtr` trait,
// which allows them to be passed to functions such as:
//
// `engine.new_sound_from_source(&source)`
//
// This abstraction makes it possible to play audio from files,
// memory, generated audio, or other custom sources using the
// same API.

const MUSIC_FILE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
));

fn main() -> MaResult<()> {
    let engine = Engine::new()?;

    // Sounds do not need to come directly from files. Instead, they can
    // be created from any audio source that implements the engine's
    // data source interface.
    //
    // One such source is a `Decoder`, which converts encoded audio data
    // (such as MP3, WAV, or FLAC) into raw PCM samples that the engine
    // can play.

    // Create a decoder that reads audio data from memory instead of a file.
    // The decoder is configured with the expected channel count and sample rate.
    //
    // We use f32 format as that is the native format of the Engine
    let decoder = DecoderBuilder::new(2, SampleRate::Sr44100).f32_memory(MUSIC_FILE)?;

    // Create a sound using the decoder as its audio source.
    let mut sound = engine.new_sound_from_source(&decoder)?;

    sound.play_sound()?;
    println!("Stopping in 5 seconds...");
    thread::sleep(Duration::from_secs(5));
    sound.stop_sound()?;
    Ok(())
}
