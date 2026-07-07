use std::sync::{Arc, Mutex};

use maudio::{
    audio::sample_rate::SampleRate,
    device::{
        device_builder::{DeviceBuilder, DeviceBuilderOps},
        DeviceOps,
    },
    MaResult,
};

// Records audio from the default capture device, then plays it back.
//
// This example uses maudio's low-level device API. Recording is a low-level
// operation because it works directly with the raw audio buffers provided by
// the device callback.
//
// In playback mode, the callback receives an output buffer and the application
// fills it with audio samples. In capture mode, the callback receives an input
// buffer containing samples captured from the microphone, and the application
// copies or processes those samples.
//
// This example runs in two phases:
//
// 1. Start a capture device and record three seconds of microphone input into
//    a buffer.
// 2. Drop the capture device, wait briefly, then start a playback device and
//    copy the recorded samples into the output buffer.
//
// The callback is allowed to receive different amounts of audio each time it
// runs, so the example tracks explicit read and write positions and clamps each
// copy to the remaining buffer length.

fn main() -> MaResult<()> {
    const SECONDS: usize = 3;
    const CHANNELS: usize = 2;
    let sr = SampleRate::Sr44100;

    // samples = seconds * sample_rate * channels
    let expected_samples = SECONDS * CHANNELS * u32::from(sr) as usize;

    // The audio callback owns its closure, so any data shared with the callback
    // needs to be moved into it. We use Arc<Mutex<_>> here so the capture
    // callback can write into the buffer, and the playback callback can read
    // from it later.
    //
    // In this example the callbacks do not run at the same time, but the Mutex
    // is still useful because the callback API requires shared ownership.
    let buffer = Arc::new(Mutex::new(vec![0.0; expected_samples]));
    let capture_buffer = buffer.clone();
    let mut write_pos = 0usize;

    println!("Capturing");

    let mut capture = DeviceBuilder::capture()
        .f32()
        .sample_rate(sr)
        .capture_channels(CHANNELS as u32)
        .with_callback(move |_, input| {
            let Ok(mut buffer) = capture_buffer.lock() else {
                return;
            };

            // Once the recording buffer is full, ignore any extra input.
            if write_pos >= buffer.len() {
                return;
            }

            // The device decides how many frames it gives us per callback.
            // Near the end of the recording, the callback may provide more
            // samples than we still have room for, so clamp the copy length.
            let remaining = buffer.len() - write_pos;
            let samples_to_copy = remaining.min(input.len());

            buffer[write_pos..write_pos + samples_to_copy]
                .copy_from_slice(&input[..samples_to_copy]);

            write_pos += samples_to_copy;
        })?;

    capture.device_start()?;

    // Let the capture device run for roughly the length of the recording.
    //
    // This is good enough for a simple example, but it is not sample-accurate.
    // The callback still clamps writes to the buffer length above, so a slightly
    // late callback cannot write past the end.
    std::thread::sleep(std::time::Duration::from_secs(SECONDS as u64));

    drop(capture);

    println!("Preparing for playback");

    // This pause is not required. It just makes the two phases easier to hear
    // and understand when running the example.
    std::thread::sleep(std::time::Duration::from_secs(1));

    let playback_buffer = buffer.clone();
    let mut read_pos = 0usize;

    let mut playback = DeviceBuilder::playback()
        .f32()
        .sample_rate(sr)
        .playback_channels(CHANNELS as u32)
        .with_callback(move |_, output| {
            // Always initialize the output buffer. If we have no more recorded
            // audio to play, the output remains silent.
            output.fill(0.0);

            let Ok(buffer) = playback_buffer.lock() else {
                return;
            };

            if read_pos >= buffer.len() {
                return;
            }

            // As with capture, the device decides how many samples it wants per
            // callback. Clamp the copy length so the final callback only copies
            // the samples that are still available.
            let remaining = buffer.len() - read_pos;
            let samples_to_copy = remaining.min(output.len());

            output[..samples_to_copy]
                .copy_from_slice(&buffer[read_pos..read_pos + samples_to_copy]);

            read_pos += samples_to_copy;
        })?;

    playback.set_master_volume(0.5)?;
    playback.device_start()?;
    std::thread::sleep(std::time::Duration::from_secs(SECONDS as u64));

    drop(playback);

    Ok(())
}
