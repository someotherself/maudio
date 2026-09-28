use maudio::{MaResult, context::ContextBuilder};

use crate::assets::AsioBackend;

#[path = "assets/backend_asio.rs"]
mod assets;

#[test]
fn custom_backend_basic_context_init() -> MaResult<()> {
    let context = ContextBuilder::new().build_custom::<AsioBackend>()?;

    drop(context);

    Ok(())
}
