// Sounds do not begin playback automatically after being created.
// Normally playback is controlled explicitly with:
//
// `sound.play_sound()?`
// `sound.stop_sound()?`
//
// However, sounds can also be *scheduled* to start and stop at specific
// points in the audio stream. This allows playback windows or timed fades
// to be defined before playback begins.
//
// Start time configuration:
// `sound.set_start_time_pcm()`
// `sound.set_start_time_millis()`
//
// Stop time configuration:
// `sound.set_stop_time_pcm()`
// `sound.set_stop_time_millis()`
//
// Fade control:
// `sound.set_fade_start_pcm()`
// `sound.set_fade_start_millis()`
// `sound.set_stop_time_with_fade_pcm()`
// `sound.set_stop_time_with_fade_millis()`

use std::{path::PathBuf, thread, time::Duration};

use maudio::{engine::Engine, MaResult};

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));
    let mut sound = engine.new_sound_from_file(&path)?;

    // Sounds can define a playback window which limits the portion of the
    // audio file that will be played.
    //
    // These settings must be configured before the sound starts playing.

    // Start playback from the beginning of the file.
    // (This call is optional because the default start time is 0.)
    sound.set_start_time_millis(0_000); // Can be ommited in this case (set to zero)

    // Stop playback at 4 seconds, with a 1 second fade-out.
    // This allows the sound to end smoothly instead of stopping abruptly.
    sound.set_stop_time_with_fade_millis(4_000, 1000);

    sound.play_sound()?;

    // `is_playing()` becomes true only after the audio backend actually
    // begins playback, which may occur slightly after `play_sound()` returns.
    //
    // If a non-zero start time is used, the sound may remain in a pending
    // state until that position is reached.
    while sound.is_playing() {
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
