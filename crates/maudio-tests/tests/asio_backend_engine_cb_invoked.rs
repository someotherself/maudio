use maudio::{
    MaResult, backend::Backend, data_source::sources::decoder::DecoderBuilder,
    device::device_id::DeviceId, engine::engine_builder::EngineBuilder,
};

use crate::assets::AsioBackend;

#[path = "assets/backend_asio.rs"]
mod assets;

const MUSIC_FILE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
));

#[test]
fn custom_backend_engine_callback_invoked() -> MaResult<()> {
    let (callback_tx, callback_rx) = std::sync::mpsc::channel();
    let device_id = DeviceId::custom_from_name(assets::TEST_DEVICE_NAME)?;
    let engine = EngineBuilder::new()
        .custom_backend::<AsioBackend>([Backend::Custom])
        .device_id(&device_id)
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
