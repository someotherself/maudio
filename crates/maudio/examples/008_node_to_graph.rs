use std::{path::PathBuf, thread, time::Duration};

use maudio::{
    audio::sample_rate::SampleRate,
    engine::{
        node_graph::{
            nodes::{filters::lpf::LpfNodeBuilder, NodeOps},
            NodeGraphOps,
        },
        Engine, EngineOps,
    },
    MaResult,
};

// The engine contains an internal audio graph, also called the *node graph*.
// Every sound, effect node, and output path is represented as a set of nodes
// connected together inside that graph.
//
// In simple examples, this routing happens automatically: a `Sound` is created,
// attached to the engine's graph, and played back through the default output and the endpoint.
// No manual graph management is required.
//
// However, more advanced setups sometimes need direct control over the signal
// path. For example, you may want to:
//
// - insert an effect between a sound and the output
// - route multiple sounds through shared processing
// - build custom sub-mixes
// - connect your own custom nodes
//
// This example shows the most basic manual routing pattern:
//
// `Sound -> Low-pass filter -> Graph endpoint`
//
// The sound is first detached from its default output path, then reconnected
// through a custom low-pass filter node before finally reaching the graph's
// endpoint, which represents the final output of the engine.

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    // Access the engine's internal node graph.
    // `as_node_graph` returns an option because the engine can exist without a node graph
    let node_graph = engine.as_node_graph().unwrap();

    // Create a low-pass filter node that will process the sound before it
    // reaches the final output.
    let mut lpf = LpfNodeBuilder::new(&node_graph, 2, SampleRate::Sr48000, 800.0, 1).build()?;

    // The endpoint is the final output node of the graph.
    //
    // Every source node in the node graph can be routed to the endpoint
    // and all the sounds wil be mixed there and will exit the node graph.
    let mut end_node = node_graph.endpoint().unwrap();

    // Create a sound source and access its node handle.
    let mut source = engine.new_sound_from_file(&path)?;
    let mut source_node = source.as_node();

    // Sounds are normally connected automatically when created.
    // Disconnect this one so we can define a custom signal path.
    //
    // This can also be done at init using `SoundFlags::NO_DEFAULT_ATTACHMENT`
    source_node.detach_all_outputs()?;

    // Re-route the signal:
    // Sound -> low-pass filter -> graph endpoint
    source_node.attach_output_bus(0, &mut lpf, 0)?;
    lpf.attach_output_bus(0, &mut end_node, 0)?;

    source.play_sound()?;
    println!("Stopping in 5 seconds...");
    thread::sleep(Duration::from_secs(5));
    source.stop_sound()?;

    Ok(())
}
