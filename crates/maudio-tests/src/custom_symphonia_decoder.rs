mod assets;

use std::process::ExitCode;

use maudio::{
    MaResult,
    audio::{channels::Channel, formats::Format, sample_rate::SampleRate},
    data_source::sources::decoder::{DecoderOps, custom_decoder::CustomDecoderBuilder},
};
use maudio_tests::check;

use crate::assets::sympnonia_decoder::SymphoniaBackend;

const MUSIC_FILE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
));

fn main() -> ExitCode {
    let mut failures = 0;

    check!(failures, custom_decoder_basic_init);
    check!(
        failures,
        custom_decoder_from_memory_f32_passthrough_data_format
    );
    check!(failures, custom_decoder_from_memory_f32_change_data_format);
    check!(
        failures,
        custom_decoder_from_memory_f32_get_input_data_format
    );
    check!(failures, custom_decoder_from_memory_f32_change_channel_map);
    check!(
        failures,
        custom_decoder_from_memory_f32_read_seek_cursor_length_available
    );

    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        eprintln!("{failures} check(s) failed");
        ExitCode::FAILURE
    }
}

fn custom_decoder_basic_init() -> MaResult<()> {
    let dec = CustomDecoderBuilder::new_f32()
        .backend::<SymphoniaBackend>()
        .copy_memory(MUSIC_FILE)?;

    drop(dec);

    Ok(())
}

fn custom_decoder_from_memory_f32_passthrough_data_format() -> MaResult<()> {
    let dec = CustomDecoderBuilder::new_f32()
        .backend::<SymphoniaBackend>()
        .copy_memory(MUSIC_FILE)?;

    let res = dec.data_format();
    assert!(res.is_ok());
    let df = res?;
    assert_eq!(df.format, Format::F32);
    assert_eq!(df.sample_rate, SampleRate::Sr44100);
    assert_eq!(df.channels, 2);
    assert_eq!(df.channel_map, [Channel::FrontLeft, Channel::FrontRight]);

    Ok(())
}

fn custom_decoder_from_memory_f32_change_data_format() -> MaResult<()> {
    let dec = CustomDecoderBuilder::new_i16()
        .backend::<SymphoniaBackend>()
        .channels(2)
        .sample_rate(SampleRate::Sr48000)
        .copy_memory(MUSIC_FILE)?;

    let res = dec.data_format();
    assert!(res.is_ok());
    let df = res?;
    assert_eq!(df.format, Format::S16);
    assert_eq!(df.sample_rate, SampleRate::Sr48000);
    assert_eq!(df.channels, 2);
    assert_eq!(df.channel_map, [Channel::FrontLeft, Channel::FrontRight]);

    Ok(())
}

fn custom_decoder_from_memory_f32_get_input_data_format() -> MaResult<()> {
    let dec = CustomDecoderBuilder::new_f32()
        .channels(1)
        .sample_rate(SampleRate::Sr48000)
        .backend::<SymphoniaBackend>()
        .copy_memory(MUSIC_FILE)?;

    let res = dec.backend_data_format();
    assert!(res.is_ok());
    let df = res?;
    assert_eq!(df.format, Format::F32);
    assert_eq!(df.sample_rate, SampleRate::Sr44100);
    assert_eq!(df.channels, 2);
    assert_eq!(df.channel_map, [Channel::FrontLeft, Channel::FrontRight]);

    Ok(())
}

fn custom_decoder_from_memory_f32_change_channel_map() -> MaResult<()> {
    let map = [
        Channel::TopBackLeft,
        Channel::TopBackLeft,
        Channel::TopFrontLeft,
        Channel::TopFrontRight,
    ];

    let dec = CustomDecoderBuilder::new_f32()
        .set_channel_map(map)
        .backend::<SymphoniaBackend>()
        .copy_memory(MUSIC_FILE)?;

    let res = dec.data_format();
    assert!(res.is_ok());
    let df = res?;
    assert_eq!(df.format, Format::F32);
    assert_eq!(df.sample_rate, SampleRate::Sr44100);
    assert_eq!(df.channels, 4);
    assert_eq!(df.channel_map, map);

    Ok(())
}

fn custom_decoder_from_memory_f32_read_seek_cursor_length_available() -> MaResult<()> {
    let frames_total = 1_553_920;

    let mut dec = CustomDecoderBuilder::new_f32()
        .channels(1)
        .backend::<SymphoniaBackend>()
        .copy_memory(MUSIC_FILE)?;

    let len = dec.length_pcm()?;
    assert_eq!(len as usize, frames_total);

    let cursor0 = dec.cursor_pcm()?;
    assert_eq!(cursor0, 0);

    let avail0 = dec.available_frames()?;
    assert_eq!(avail0 as usize, frames_total);

    let df = dec.data_format()?;
    assert_eq!(df.channels, 1);
    assert_eq!(df.sample_rate, SampleRate::Sr44100);
    assert_eq!(df.format, Format::F32);

    let buf = dec.read_pcm_frames(10)?;
    let read = buf.frames();
    assert_eq!(read, 10);
    assert_eq!(buf.len(), 10);

    let cursor1 = dec.cursor_pcm()?;
    assert_eq!(cursor1, 10);

    let avail1 = dec.available_frames()?;
    assert_eq!(avail1 as usize, frames_total - 10);

    dec.seek_to_pcm_frame(0)?;
    assert_eq!(dec.cursor_pcm()?, 0);

    let buf2 = dec.read_pcm_frames(7)?;
    let read2 = buf2.frames();
    assert_eq!(read2, 7);
    assert_eq!(dec.cursor_pcm()?, 7);

    Ok(())
}
