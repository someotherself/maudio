use maudio::{
    engine::engine_builder::EngineBuilder, util::device_notif::DeviceNotificationType, MaResult,
};

// Demonstrates receiving engine state notifications.
//
// State changes such as starting or stopping the engine are driven by the
// underlying audio device and is reported asynchronously. This means a
// call to `engine.start()` does not guarantee that the notification flag is
// updated immediately.
//
// This notification is actually attached to the `Device` the Engine uses.
// Therefore, creating an engine without a Device will not set the `DeviceStateNotifier`

fn main() -> MaResult<()> {
    let engine = EngineBuilder::new()
        .no_auto_start(true)
        .state_notifier()
        .build()?;
    let notif = engine.get_state_notifier().unwrap();

    println!("Starting engine...");
    engine.start()?;

    // Notifications are delivered asynchronously. Poll briefly until the
    // device reports that it has started.
    for _ in 0..100 {
        if notif.contains(DeviceNotificationType::Started) {
            println!("Engine reported Started.");
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    println!("Stopping engine...");
    engine.stop()?;

    Ok(())
}
