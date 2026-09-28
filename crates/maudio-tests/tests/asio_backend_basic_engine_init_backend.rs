use maudio::{
    MaResult, backend::Backend, device::device_id::DeviceId, engine::engine_builder::EngineBuilder,
};

use crate::assets::AsioBackend;

#[path = "assets/backend_asio.rs"]
mod assets;

#[test]
fn custom_backend_basic_engine_init_custom_backend() -> MaResult<()> {
    let device_id = DeviceId::custom_from_name(assets::TEST_DEVICE_NAME)?;
    let engine = EngineBuilder::new()
        .device_id(&device_id)
        .custom_backend::<AsioBackend>([Backend::Custom])
        .build()?;

    drop(engine);
    Ok(())
}
