use std::time::Duration;

use maudio::{
    audio::sample_rate::SampleRate,
    data_source::sources::decoder::{DecoderBuilder, DecoderOps},
    device::{
        device_builder::{DeviceBuilder, DeviceBuilderOps},
        DeviceOps,
    },
    MaResult,
};

const MUSIC_FILE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../maudio-sys/native/miniaudio/data/48000-stereo.ogg"
));

// Create a decoder that outputs `i16` samples, with the format we want the
// playback device to use.
//
// All the types in an audio chain will be forced to be in the same format.
//
// In this example the audio data is embedded in the binary and decoded from
// memory, but the same pattern also works with a file-backed decoder.

fn main() -> MaResult<()> {
    let mut decoder = DecoderBuilder::new_i16(2, SampleRate::Sr44100).from_memory(MUSIC_FILE)?;

    // We get the data format info from the encoder to ensure they are the same as the device
    // This step may be optional in some cases, especially if they are both left as default.
    // But it is a good practice regardless.
    let data_format = decoder.data_format()?;

    // Since this example does not create a custom context or request a specific
    // backend, miniaudio will choose a suitable native backend for the current
    // platform, such as WASAPI on Windows, Core Audio on macOS, or an available
    // Linux backend.
    let mut device = DeviceBuilder::playback()
        .i16()
        .playback_channels(data_format.channels)
        .sample_rate(data_format.sample_rate)
        // a device cannot be started without a callback
        .with_callback(move |_, out| {
            // This callback is called by the audio device whenever it needs more
            // audio. The decoder is moved into this closure, which lets the
            // callback keep reading from the same decoder state each time.
            //
            // `out` is the output buffer for this callback. It contains samples,
            // not frames. For example, with 2 channels, 512 frames means 1024 samples.
            let frames_read = decoder.read_pcm_frames_into(out).unwrap_or(0);

            // `read_pcm_frames_into()` returns the number of complete PCM frames
            // that were written. Convert that to a sample count before checking
            // the interleaved output buffer.
            let samples_read = frames_read * data_format.channels as usize;

            // If the decoder reached the end of the stream, or failed to fill the
            // whole output buffer, silence the unwritten part. Audio callbacks
            // should always fully initialize their output buffer or will cause undefined behaviour.
            if samples_read < out.len() {
                out[samples_read..].fill(0);
            }
        })?;

    device.set_master_volume(0.02)?; // adjust this as needed

    device.device_start()?;

    println!("Stopping in 5 seconds...");
    std::thread::sleep(Duration::from_secs(5));

    device.device_stop()?;

    Ok(())
}
