use std::path::PathBuf;

use maudio::{
    audio::{formats::SampleBuffer, sample_rate::SampleRate},
    data_source::sources::pcm_ring_buffer::PcmRingBuffer,
    engine::{engine_builder::EngineBuilder, EngineOps},
    sound::sound_builder::SoundBuilder,
    MaResult,
};

// In previous examples, creating an engine automatically started playback
// through a device, which internally pulls frames as needed.
//
// The engine (and many other types in maudio) can also be used in a
// "pull" model, where you manually request PCM frames.
//
// Maudio exposes this via two methods:
// - `read_pcm_frames`      - allocates and returns a buffer with frames.
// - `read_pcm_frames_into` - writes frames into a caller-provided buffer.
//
// The engine is different as read_pcm_frames cannot be used
// unless the engine is created without a `Device`. This is because there
// is no automatic consumer (like a playback device) pulling frames.
//
// In this example, we will manually read frames from the engine and forward
// them into a `PcmRingBuffer`. This demonstrates a common pattern where
// audio is produced in one place and consumed later (e.g. another thread,
// analysis system, or file writer).

fn main() -> MaResult<()> {
    // declare some variables
    let channels = 2;
    let chunk_frames = 128;
    let ring_capacity = 1024;

    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let engine = EngineBuilder::new()
        .no_device(channels, SampleRate::Sr48000)
        .build()?;

    let mut sound = SoundBuilder::new(&engine).file_path(&path).build()?;
    sound.play_sound()?;

    // Create the ring buffer in f32 format (format used by the engine)
    // It is a single producer, single consumer ring buffer, implementing Send.
    // In a more practical scenarion, you can send one end to another thread
    let (mut send, mut recv) = PcmRingBuffer::new_f32(ring_capacity, 2)?;

    // SampleBuffer is used to hold interleaved PCM frames.
    // It is initialized with silence for the chosen sample format
    // (e.g. 0.0 for f32, 0 for i16, 128 for u8).
    // Only the frames returned by `read()` will be overwritten.
    let mut consume_buf = SampleBuffer::<f32>::new_zeroed(chunk_frames, channels)?;

    // Because we fully drain the ring buffer at the end of this example,
    // these totals should all match.
    let mut total_from_engine = 0;
    let mut total_into_ring = 0;
    let mut total_from_ring = 0;

    for i in 0..20 {
        // `write_with` gives us a writable PCM slice and expects the closure to
        // return how many frames were actually produced into that slice.
        //
        // This is where `read_pcm_frames_into` is useful: it writes engine output
        // directly into the provided slice and returns the number of frames read.
        let written =
            send.write_with(100, |slice| engine.read_pcm_frames_into(slice).unwrap_or(0))?;

        total_from_engine += written;
        total_into_ring += written;

        // Consumer side: every other iteration, drain some frames back out.
        if i % 2 == 1 {
            let read = recv.read(&mut consume_buf)?;
            total_from_ring += read;

            println!(
                "iteration {i:02}: wrote {written:3} frames into ring, read {read:3} frames out"
            );
        } else {
            println!("iteration {i:02}: wrote {written:3} frames into ring");
        }
    }

    // Drain any frames still left in the ring buffer.
    loop {
        let read = recv.read(&mut consume_buf)?;
        if read == 0 {
            break;
        }

        total_from_ring += read;
        println!("drain: read {read} more frames");
    }

    println!("----------------------------------");
    println!("total frames read from engine: {total_from_engine}");
    println!("total frames written to ring: {total_into_ring}");
    println!("total frames read from ring:  {total_from_ring}");

    Ok(())
}
