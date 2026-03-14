use std::path::PathBuf;

use maudio::{engine::Engine, sound::sound_builder::SoundBuilder, util::fence::Fence, MaResult};

// Asynchronous loading of sound files is available at various levels.
//
// Typically, the function that creates the sound returns immediately,
// even if the underlying file is still being loaded or decoded in the
// background.
//
// A `Fence` is en example of a synchronization primitive that can be used to wait
// until the resource is fully initialized.
//
// In this example, we create a `Sound` from a file using asynchronous
// loading. The sound is associated with a `Fence`, which can be used to
// block until the loading process has completed.

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    // A fence lets us synchronize with background initialization.
    //
    // Think of it as a "not ready yet" marker. While the resource manager is still
    // loading/decoding the sound on another thread, the fence remains acquired.
    // Once initialization is finished, the fence is released and `wait()` unblocks.
    let fence = Fence::new()?;

    // We create a sound and pass in our Fence
    //
    // Passing in the Fence will implicitly enable ASYNC loading as well
    let mut sound = SoundBuilder::new(&engine)
        .fence(fence.clone())
        .file_path(&path)
        .build()?;

    println!("Sound creation returned immediately.");
    println!("The file may still be loading in the background...");

    // Do other useful work while loading continues.
    // In a real program this might be:
    // - building other sounds
    // - setting up nodes/effects
    // - loading UI/state
    // - preparing a playlist
    for i in 1..=3 {
        println!("Doing other setup work... step {}", i);
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    // Now wait until the sound is fully initialized.
    //
    // This blocks the current thread efficiently using OS synchronization
    // primitives. It does not spin in a loop.
    println!("Waiting for the sound to finish loading...");
    fence.wait()?;

    println!("Sound is ready. Starting playback.");
    sound.play_sound()?;
    Ok(())
}
