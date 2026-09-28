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
fn custom_backend_basic_device_start_stop() -> MaResult<()> {
    let device_id = DeviceId::custom_from_name(assets::TEST_DEVICE_NAME)?;

    let mut device = DeviceBuilder::playback()
        .f32()
        .custom_backend::<AsioBackend>([Backend::Custom])
        .playback_device_id(&device_id)
        .with_callback(|_, out| out.fill(0.0))?;

    device.device_start()?;
    device.device_stop()?;

    Ok(())
}
