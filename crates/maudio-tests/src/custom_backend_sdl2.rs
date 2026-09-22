use maudio::{
    MaResult,
    backend::Backend,
    context::{ContextBuilder, ContextOps, EnumerateControl},
    data_source::sources::decoder::DecoderBuilder,
    device::device_builder::{DeviceBuilder, DeviceBuilderOps},
    engine::engine_builder::EngineBuilder,
};
use maudio_tests::assets::backend_sdl2::SdlBackend;

pub mod assets;

const MUSIC_FILE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
));

fn main() -> MaResult<()> {
    custom_backend_basic_context_init()?;
    custom_backed_context_enumerate()?;

    custom_backend_basic_device_init_custom_backend()?;
    custom_backend_basic_device_init_custom_context()?;
    custom_backend_basic_engine_init_custom_backend()?;
    custom_backend_basic_engine_init_custom_context()?;

    custom_backend_basic_device_start_stop()?;
    custom_backend_basic_engine_start_stop()?;

    custom_backend_device_callback_invoked()?;
    custom_backend_engine_callback_invoked()?;

    Ok(())
}

fn custom_backend_basic_context_init() -> MaResult<()> {
    let context = ContextBuilder::new().build_custom::<SdlBackend>()?;

    drop(context);

    Ok(())
}

fn custom_backed_context_enumerate() -> MaResult<()> {
    let context = ContextBuilder::new().build_custom::<SdlBackend>()?;

    context.enumerate_devices(|_, _| EnumerateControl::Stop)?;

    Ok(())
}

fn custom_backend_basic_device_init_custom_backend() -> MaResult<()> {
    let device = DeviceBuilder::playback()
        .f32()
        .custom_backend::<SdlBackend>([Backend::Custom])
        .with_callback(|_, out| out.fill(0.0))?;

    drop(device);
    Ok(())
}

fn custom_backend_basic_device_init_custom_context() -> MaResult<()> {
    let context = ContextBuilder::new()
        .preferred_backends([Backend::Custom])
        .build_custom()?;

    let device = DeviceBuilder::playback()
        .f32()
        .custom_context::<SdlBackend>(&context)
        .with_callback(|_, out| out.fill(0.0))?;

    drop(device);
    Ok(())
}

fn custom_backend_basic_engine_init_custom_backend() -> MaResult<()> {
    let engine = EngineBuilder::new()
        .custom_backend::<SdlBackend>([Backend::Custom])
        .build()?;

    drop(engine);
    Ok(())
}

fn custom_backend_basic_engine_init_custom_context() -> MaResult<()> {
    let mut ctx_builder = ContextBuilder::new();
    let ctx_builder = ctx_builder.preferred_backends([Backend::Custom]);

    let engine = EngineBuilder::new()
        .custom_context::<SdlBackend>(ctx_builder)
        .build()?;

    drop(engine);
    Ok(())
}

fn custom_backend_basic_device_start_stop() -> MaResult<()> {
    let mut device = DeviceBuilder::playback()
        .f32()
        .custom_backend::<SdlBackend>([Backend::Custom])
        .with_callback(|_, out| out.fill(0.0))?;

    device.device_start()?;
    device.device_stop()?;

    Ok(())
}

fn custom_backend_basic_engine_start_stop() -> MaResult<()> {
    let engine = EngineBuilder::new()
        .no_auto_start(true)
        .custom_backend::<SdlBackend>([Backend::Custom])
        .build()?;

    engine.start()?;
    engine.stop()?;

    Ok(())
}

fn custom_backend_device_callback_invoked() -> MaResult<()> {
    let (callback_tx, callback_rx) = std::sync::mpsc::channel();

    let mut device = DeviceBuilder::playback()
        .f32()
        .custom_backend::<SdlBackend>([Backend::Custom])
        .with_callback(move |_, out| {
            out.fill(0.0);
            let _ = callback_tx.send(());
        })?;

    device.device_start()?;

    callback_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("SDL dummy backend did not invoke the audio callback");

    device.device_stop()?;

    Ok(())
}

fn custom_backend_engine_callback_invoked() -> MaResult<()> {
    let (callback_tx, callback_rx) = std::sync::mpsc::channel();
    let engine = EngineBuilder::new()
        .custom_backend::<SdlBackend>([Backend::Custom])
        .with_realtime_callback(move |out, _| {
            out.fill(0.0);
            let _ = callback_tx.send(());
        })?;

    let decoder = DecoderBuilder::new_f32().from_memory(MUSIC_FILE)?;

    // Create a sound using the decoder as its audio source.
    let sound = engine.new_sound_from_source(&decoder)?;

    callback_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("SDL dummy backend did not invoke the audio callback");

    engine.stop()?;

    Ok(())
}
