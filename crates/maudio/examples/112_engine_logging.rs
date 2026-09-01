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

    // When miniaudio calls our register_log, it passes the
    // level and the message of that event. We can then use
    // them to print our own logs.
    //
    // Dropping the listener will remove our logging hook.
    // For a permanent log, `Log::print_level`:
    // log.print_level(LogLevel::Warning)?;

    let _listener = log.register_log(|level, msg| {
        if matches!(level, LogLevel::Info | LogLevel::Warning | LogLevel::Error) {
            eprintln!("[{level}] {msg}");
        }
    })?;

    let _engine = EngineBuilder::new().logger(&log).build()?;

    // For this purpose, we don't need to play any sound

    Ok(())
}
