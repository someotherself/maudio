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
fn custom_backend_device_capture_works() -> MaResult<()> {
    let device_id = DeviceId::custom_from_name(assets::TEST_DEVICE_NAME)?;
    let mut device = DeviceBuilder::capture()
        .f32()
        .capture_device_id(&device_id)
        .custom_backend::<AsioBackend>([Backend::Custom])
        .with_callback(|_, _| {})?;

    device.device_start()?;
    device.device_stop()?;

    Ok(())
}
