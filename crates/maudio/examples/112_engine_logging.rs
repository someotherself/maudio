use maudio::{
    engine::engine_builder::EngineBuilder,
    logging::{Log, LogLevel, LogOps},
    MaResult,
};

fn main() -> MaResult<()> {
    // Most of the logging happens when initializing
    // the engine / device / context. So, it is useful to
    // create a log and add it in the builder.
    let log = Log::new()?;

    // We use `print_level` to enable specific log levels.
    // Later on, these levels can also be removed usign `remove_level`
    log.print_level(LogLevel::Info)?;
    log.print_level(LogLevel::Warning)?;
    log.print_level(LogLevel::Error)?;

    let _engine = EngineBuilder::new().logger(&log).build()?;

    // For this purpose, we don't need to play any sound

    Ok(())
}
