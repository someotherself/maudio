use maudio::{engine::engine_builder::EngineBuilder, MaResult};

// Demonstrates using the engine process notifier to observe audio progress.
//
// The engine's process callback runs on the audio thread.
// Miniaudio allows us to pass a callback that runs inside the audio thread when the engine processes frames.
//
// However, If you only need a signal that audio has been processed, it is often better to use a notifier
// like this instead of doing work inside the callback itself.
//
// This is especially useful when reacting from normal application code, where
// it is safer to manage sounds or other engine state outside the callback.
//
// The ProcFramesNotif is an AtomicU64 that keeps track of the number of frames process

fn main() -> MaResult<()> {
    let engine = EngineBuilder::new().with_process_notifier()?;
    let proc_notif = engine.get_data_notifier().unwrap();

    for _ in 0..20 {
        proc_notif.take_with(|frames| {
            println!("Engine processed {frames} PCM frames.");
        });

        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    Ok(())
}
