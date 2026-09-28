use maudio::{
    MaResult,
    backend::Backend,
    device::{
        device_builder::{DeviceBuilder, DeviceBuilderOps},
        device_id::DeviceId,
    },
};

use crate::assets::AsioBackend;

#[path = "assets/backend_asio.rs"]
mod assets;

#[test]
fn custom_backend_device_callback_invoked() -> MaResult<()> {
    let (callback_tx, callback_rx) = std::sync::mpsc::channel();
    let device_id = DeviceId::custom_from_name(assets::TEST_DEVICE_NAME)?;

    let mut device = DeviceBuilder::playback()
        .f32()
        .playback_device_id(&device_id)
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
