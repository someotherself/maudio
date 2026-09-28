use maudio::{
    MaResult,
    context::{ContextBuilder, ContextOps, EnumerateControl},
};

use crate::assets::AsioBackend;

#[path = "assets/backend_asio.rs"]
mod assets;

#[test]
fn custom_backend_context_enumerate() -> MaResult<()> {
    let context = ContextBuilder::new().build_custom::<AsioBackend>()?;

    context.enumerate_devices(|_, _| EnumerateControl::Stop)?;

    Ok(())
}
