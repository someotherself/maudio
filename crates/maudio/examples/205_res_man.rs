use std::path::PathBuf;

use maudio::{
    audio::sample_rate::SampleRate,
    data_source::DataSourceOps,
    device::device_builder::{DeviceBuilder, DeviceBuilderOps},
    engine::resource::{rm_builder::ResourceManagerBuilder, rm_source_flags::RmSourceFlags, RmOps},
    MaResult,
};

// This example show the resource manager used in combination with the low level API
//
// We are registering audio in the main thread, then in a separate thread we create
// an engine, and loading that sound using another instance of the resource manager.
//
// This example is simple and the thread is created after the audio was fully registered.
// More complex scenarios may require a syncronization mechanism if new sounds are registered later.

fn main() -> MaResult<()> {
    let channels = 2;
    let sample_rate = SampleRate::Sr44100;

    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let rm = ResourceManagerBuilder::new_f32()
        .channels(channels)
        .sample_rate(sample_rate)
        .build()
        .unwrap();

    // keep this alive somehow
    let _guard1 = rm.register_file(&path, RmSourceFlags::NONE).unwrap();

    let rm_worker = rm.clone();

    let join_h = std::thread::spawn(move || -> MaResult<()> {
        // It's possible to move the original guard we created in this thread.
        // We could also move only the final buffer directly.
        // Having access to the resource manager gives us more flexibility.

        // This does not poll the filesystem anymore. It loads the registration we did before.
        // Registering an in memory sound and loading it by name also works, without having
        // to move the in memory sound to this thread.
        let worker_guard = rm_worker.load_path(&path).unwrap();

        let mut buffer = worker_guard
            .build_buffer(RmSourceFlags::NONE, None)?
            .into_ready()
            .unwrap();

        let mut device = DeviceBuilder::playback()
            .f32()
            .playback_channels(channels)
            .sample_rate(sample_rate)
            // a device cannot be started without a callback
            .with_callback(move |_, out| {
                let frames_read = buffer.read_pcm_frames_into(out).unwrap_or(0);

                let samples_read = frames_read * channels as usize;

                if samples_read < out.len() {
                    out[samples_read..].fill(0.0);
                }
            })?;
        device.device_start()?;

        std::thread::sleep(std::time::Duration::from_secs(2));

        Ok(())
    });

    let _ = join_h.join();
    Ok(())
}
