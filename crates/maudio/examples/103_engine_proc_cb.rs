use std::path::PathBuf;

use maudio::{engine::engine_builder::EngineBuilder, MaResult};

// # Real-time processing callback
//
// This example shows how the engine's real-time callback works.
//
// Every time the engine produces audio, it calls your function and gives you
// a mutable slice of `f32` samples.
//
// One important thing to understand:
// the buffer already contains audio. The engine has already done all its
// mixing and processing.
//
// So you're not generating sound here — you're getting the final output and
// can tweak it if you want.
//
// Typical things you might do here:
// - apply a simple effect (gain, distortion, etc.)
// - inspect the signal (peak, RMS, VU meter)
// - copy the data somewhere else (recording, ring buffer, etc.)
//
// This runs on the audio thread, so keep it simple:
// - no allocations
// - no locks
// - no blocking work
//
// In this example we just reduce the volume.

fn main() -> MaResult<()> {
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let engine = EngineBuilder::new().with_realtime_callback(|samples, _channels| {
        // Example: simple gain
        for s in samples {
            *s *= 0.5;
        }
    })?;

    let mut sound = engine.new_sound_from_file(&path).unwrap();

    sound.play_sound().unwrap();

    println!("Stopping in 5 seconds...");
    std::thread::sleep(std::time::Duration::from_secs(5));

    sound.stop_sound()?;

    Ok(())
}
