use std::{process::ExitCode, sync::OnceLock};

use maudio::{
    MaResult,
    backend::Backend,
    context::{ContextBuilder, ContextOps, EnumerateControl},
    data_source::sources::decoder::DecoderBuilder,
    device::{
        device_builder::{DeviceBuilder, DeviceBuilderOps},
        device_id::DeviceId,
        device_type::DeviceType,
    },
    engine::engine_builder::EngineBuilder,
};
use maudio_tests::check;

use crate::assets::backend_asio::AsioBackend;

pub mod assets;

const MUSIC_FILE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
));

static TEST_PLAYBACK_DEVICE_ID: OnceLock<DeviceId> = OnceLock::new();
static TEST_CAPTURE_DEVICE_ID: OnceLock<DeviceId> = OnceLock::new();

fn main() -> ExitCode {
    let mut failures = 0;

    match init_test_device_ids() {
        Ok(false) | Err(_) => {
            eprintln!("Cannot initialize test device ids");
            return ExitCode::FAILURE;
        }
        _ => {}
    }

    check!(failures, custom_backend_basic_context_init);
    check!(failures, custom_backend_context_enumerate);
    check!(failures, custom_backend_basic_device_init_custom_backend);
    check!(failures, custom_backend_basic_device_init_custom_context);
    check!(failures, custom_backend_basic_engine_init_custom_backend);
    check!(failures, custom_backend_basic_engine_init_custom_context);
    check!(failures, custom_backend_basic_device_start_stop);
    check!(failures, custom_backend_basic_engine_start_stop);
    check!(failures, custom_backend_device_capture_works);
    check!(failures, custom_backend_device_callback_invoked);
    check!(failures, custom_backend_engine_callback_invoked);

    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        eprintln!("{failures} check(s) failed");
        ExitCode::FAILURE
    }
}

fn init_test_device_ids() -> MaResult<bool> {
    let ctx = ContextBuilder::new()
        .preferred_backends([Backend::Custom])
        .build_custom::<AsioBackend>()?;

    let mut play_init = false;
    let mut capt_init = false;

    ctx.enumerate_devices(|ty, info| {
        if matches!(ty, DeviceType::Playback)
            && TEST_PLAYBACK_DEVICE_ID.get().is_none()
            && TEST_PLAYBACK_DEVICE_ID.set(info.id().clone()).is_ok()
        {
            play_init = true;
        }

        if matches!(ty, DeviceType::Capture)
            && TEST_CAPTURE_DEVICE_ID.get().is_none()
            && TEST_CAPTURE_DEVICE_ID.set(info.id().clone()).is_ok()
        {
            capt_init = true;
        }
        EnumerateControl::Continue
    })?;
    Ok(play_init && capt_init)
}

fn custom_backend_basic_context_init() -> MaResult<()> {
    let context = ContextBuilder::new().build_custom::<AsioBackend>()?;

    drop(context);

    Ok(())
}

fn custom_backend_context_enumerate() -> MaResult<()> {
    let context = ContextBuilder::new().build_custom::<AsioBackend>()?;

    context.enumerate_devices(|_, _| EnumerateControl::Stop)?;

    Ok(())
}

fn custom_backend_basic_device_init_custom_backend() -> MaResult<()> {
    let device_id = TEST_PLAYBACK_DEVICE_ID.get().unwrap();
    let device = DeviceBuilder::playback()
        .f32()
        .playback_device_id(device_id)
        .custom_backend::<AsioBackend>([Backend::Custom])
        .with_callback(|_, out| out.fill(0.0))?;

    drop(device);
    Ok(())
}

fn custom_backend_basic_device_init_custom_context() -> MaResult<()> {
    let context = ContextBuilder::new()
        .preferred_backends([Backend::Custom])
        .build_custom()?;

    let device_id = TEST_PLAYBACK_DEVICE_ID.get().unwrap();

    let device = DeviceBuilder::playback()
        .f32()
        .playback_device_id(device_id)
        .custom_context::<AsioBackend>(&context)
        .with_callback(|_, out| out.fill(0.0))?;

    drop(device);
    Ok(())
}

fn custom_backend_basic_engine_init_custom_backend() -> MaResult<()> {
    let device_id = TEST_PLAYBACK_DEVICE_ID.get().unwrap();
    let engine = EngineBuilder::new()
        .device_id(device_id)
        .custom_backend::<AsioBackend>([Backend::Custom])
        .build()?;

    drop(engine);
    Ok(())
}

fn custom_backend_basic_engine_init_custom_context() -> MaResult<()> {
    let mut ctx_builder = ContextBuilder::new();
    let ctx_builder = ctx_builder.preferred_backends([Backend::Custom]);

    let device_id = TEST_PLAYBACK_DEVICE_ID.get().unwrap();
    let engine = EngineBuilder::new()
        .device_id(device_id)
        .custom_context::<AsioBackend>(ctx_builder)
        .build()?;

    drop(engine);
    Ok(())
}

fn custom_backend_basic_device_start_stop() -> MaResult<()> {
    let device_id = TEST_PLAYBACK_DEVICE_ID.get().unwrap();
    let mut device = DeviceBuilder::playback()
        .f32()
        .custom_backend::<AsioBackend>([Backend::Custom])
        .playback_device_id(device_id)
        .with_callback(|_, out| out.fill(0.0))?;

    device.device_start()?;
    device.device_stop()?;

    Ok(())
}

fn custom_backend_basic_engine_start_stop() -> MaResult<()> {
    let device_id = TEST_PLAYBACK_DEVICE_ID.get().unwrap();
    let engine = EngineBuilder::new()
        .no_auto_start(true)
        .device_id(device_id)
        .custom_backend::<AsioBackend>([Backend::Custom])
        .build()?;

    engine.start()?;
    engine.stop()?;

    Ok(())
}

fn custom_backend_device_capture_works() -> MaResult<()> {
    let device_id = TEST_CAPTURE_DEVICE_ID.get().unwrap();
    let mut device = DeviceBuilder::capture()
        .f32()
        .capture_device_id(device_id)
        .custom_backend::<AsioBackend>([Backend::Custom])
        .with_callback(|_, _| {})?;

    device.device_start()?;
    device.device_stop()?;

    Ok(())
}

fn custom_backend_device_callback_invoked() -> MaResult<()> {
    let (callback_tx, callback_rx) = std::sync::mpsc::channel();
    let device_id = TEST_PLAYBACK_DEVICE_ID.get().unwrap();

    let mut device = DeviceBuilder::playback()
        .f32()
        .playback_device_id(device_id)
        .custom_backend::<AsioBackend>([Backend::Custom])
        .with_callback(move |_, out| {
            out.fill(0.0);
            let _ = callback_tx.send(());
        })?;

    device.device_start()?;

    callback_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("ASIO backend did not invoke the audio callback");

    device.device_stop()?;

    Ok(())
}

fn custom_backend_engine_callback_invoked() -> MaResult<()> {
    let (callback_tx, callback_rx) = std::sync::mpsc::channel();
    let device_id = TEST_PLAYBACK_DEVICE_ID.get().unwrap();
    let engine = EngineBuilder::new()
        .custom_backend::<AsioBackend>([Backend::Custom])
        .device_id(device_id)
        .with_realtime_callback(move |out, _| {
            out.fill(0.0);
            let _ = callback_tx.send(());
        })?;

    let decoder = DecoderBuilder::new_f32().from_memory(MUSIC_FILE)?;

    // Create a sound using the decoder as its audio source.
    let _sound = engine.new_sound_from_source(&decoder)?;

    callback_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("ASIO backend did not invoke the audio callback");

    engine.stop()?;

    Ok(())
}
