use maudio::{
    MaResult,
    backend::Backend,
    context::ContextBuilder,
    device::{
        device_builder::{DeviceBuilder, DeviceBuilderOps},
        device_id::DeviceId,
    },
};

use crate::assets::AsioBackend;

#[path = "assets/backend_asio.rs"]
mod assets;

#[test]
fn custom_backend_basic_device_init_custom_context() -> MaResult<()> {
    let context = ContextBuilder::new()
        .preferred_backends([Backend::Custom])
        .build_custom()?;

    let device_id = DeviceId::custom_from_name(assets::TEST_DEVICE_NAME)?;

    let device = DeviceBuilder::playback()
        .f32()
        .playback_device_id(&device_id)
        .custom_context::<AsioBackend>(&context)
        .with_callback(|_, out| out.fill(0.0))?;

    drop(device);
    Ok(())
}
