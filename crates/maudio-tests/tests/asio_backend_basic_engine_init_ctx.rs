use maudio::{
    MaResult, backend::Backend, context::ContextBuilder, device::device_id::DeviceId,
    engine::engine_builder::EngineBuilder,
};

use crate::assets::AsioBackend;

#[path = "assets/backend_asio.rs"]
mod assets;

#[test]
fn custom_backend_basic_engine_init_custom_context() -> MaResult<()> {
    let mut ctx_builder = ContextBuilder::new();
    let ctx_builder = ctx_builder.preferred_backends([Backend::Custom]);

    let device_id = DeviceId::custom_from_name(assets::TEST_DEVICE_NAME)?;
    let engine = EngineBuilder::new()
        .device_id(&device_id)
        .custom_context::<AsioBackend>(ctx_builder)
        .build()?;

    drop(engine);
    Ok(())
}
