use std::path::PathBuf;

use maudio::{engine::Engine, MaResult};

// Audio libraries commonly use callbacks to notify the application when
// something happens during playback, such as when a sound finishes,
// a device changes state, or more audio data is needed.
//
// `maudio` exposes several callback-based features, but many common cases
// can be handled with small notification helpers instead of requiring the
// user to manage callback state directly.
//
// These callbacks usually run on the audio thread. Only a limited set of
// operations are safe to perform there. In particular, starting or stopping
// sounds, modifying the engine, or performing heavier application logic
// should generally NOT be done inside the callback itself.
//
// Instead, the callback should signal the application that an event occurred,
// and the application can react from its main thread or event loop.
//
// This example shows the simplest form: an `EndNotifier`.
// Internally, a playback callback sets a flag when the sound finishes.
// The application can then poll that flag from its main loop and react
// to the event at a convenient time (for example by starting the next sound).

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));
    let mut sound = engine.new_sound_from_file(&path)?;

    // Register a simple end-of-playback notification.
    //
    // Internally this installs the necessary callback state and returns an
    // `EndNotifier`, which can be polled from another thread.
    let notif = sound.set_end_callback()?;

    sound.set_volume(0.1);
    sound.play_sound()?;

    loop {
        // In a real program, the main loop might update a UI, process input,
        // or advance game/application state.

        // `call_if_notified()` consumes the notification and runs the closure
        // exactly once for each playback-end event.

        notif.call_if_notified(|| {
            println!("Sound ended — queue next track");
        });

        // Break condition for example purposes
        if !sound.is_playing() {
            break;
        }
    }
    Ok(())
}
