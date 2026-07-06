use maudio::{
    audio::{dsp::filters::lpf_filter::LpfBuilder, sample_rate::SampleRate},
    data_source::sources::decoder::{DecoderBuilder, DecoderOps},
    device::{
        device_builder::{DeviceBuilder, DeviceBuilderOps},
        DeviceOps,
    },
    MaResult,
};

// This example plays decoded audio through a delay effect before sending it to
// the playback device.
//
// Unlike the basic playback example, the decoder does not write directly into
// the device output buffer. The data flow is:
//
// decoder -> temporary input buffer -> delay -> device output buffer
//
// The temporary buffer is needed because the delay processor reads from one
// buffer and writes the processed result into another.

const MUSIC_FILE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
));

fn main() -> MaResult<()> {
    // Decode into f32 because most DSP processing works naturally with floating
    // point samples.
    let mut decoder = DecoderBuilder::new_f32(2, SampleRate::Sr48000).from_memory(MUSIC_FILE)?;
    let df = decoder.data_format()?;

    // Create a low-pass filter (or any dsp primitive)
    let mut lpf = LpfBuilder::new(df.channels, df.sample_rate, 600.0, 1).build_f32()?;

    // Buffer used between the decoder and the delay processor.
    //
    // The device callback gives us `out`, which is the final output buffer. We
    // need a separate input buffer so the decoder can write unprocessed samples,
    // and the delay can write the processed result into `out`.
    //
    // The buffer length is measured in samples, not frames. With 2 channels and
    // a fixed callback size of 512 frames, the callback will usually receive:
    //
    //     1024 frames * 2 channels = 2048 samples
    //
    // The callback below still resizes this buffer if needed, which keeps the
    // example correct even if the device chooses a different size.
    // But this will likely not be needed, for reasons that we'll see below.
    let mut lpf_buffer: Vec<f32> = vec![0.0; 2048];

    let mut device = DeviceBuilder::playback()
        .f32()
        .playback_channels(df.channels)
        .sample_rate(df.sample_rate)
        // Ask the device to use 512 frames per callback.
        //
        // This makes the example easier to reason about because the callback
        // size is predictable. Without a fixed callback size, the backend may
        // choose different callback sizes, and the temporary buffer must be
        // prepared for that.
        //
        // Without setting 'fixed_callback_size' to `true`, the device would only use the
        // `period_size_in_frames` as a hint, and may request different values.
        .period_size_frames(1024)
        .fixed_callback_size(true)
        .with_callback(move |_a, out| {
            if lpf_buffer.len() < out.len() {
                lpf_buffer.resize(out.len(), 0.0);
            }
            let input = &mut lpf_buffer[..out.len()];

            let frames_read = decoder.read_pcm_frames_into(input).unwrap_or(0);

            // A good practice to silence part of the buffer is we wrote
            // fewer frames than expected
            let samples_read = frames_read * df.channels as usize;
            if samples_read < input.len() {
                input[samples_read..].fill(0.0);
            }

            if lpf.process_pcm_frames(out, input).is_err() {
                out.fill(0.0);
            }
        })
        .unwrap();

    device.device_start()?;

    device.set_master_volume(0.4)?; // adjust this as needed

    println!("Stopping in 5 seconds...");
    std::thread::sleep(std::time::Duration::from_secs(5));

    device.device_stop()?;

    Ok(())
}
