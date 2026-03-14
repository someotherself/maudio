use std::{thread, time::Duration};

use maudio::{
    audio::{sample_rate::SampleRate, wave_shape::WaveFormType},
    data_source::sources::waveform::{WaveFormBuilder, WaveFormOps},
    engine::{
        node_graph::{
            nodes::{source::source_node::SourceNodeBuilder, NodeOps},
            NodeGraphOps,
        },
        Engine, EngineOps,
    },
    MaResult,
};

// This is a more basic example of how multiple sources can be added to an engine or node graph
// In this example, they are all connected to the endpoint.
// In more practical scenarios, you may want to connect them to a mixer node (SplitterNode)
// so the are mixed there.
// This allows other effect or filter nodes to be added between the mixer and the endpoint.

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let mut wave_src = WaveFormBuilder::new(2, SampleRate::Sr44100, WaveFormType::Sine, 0.8, 200.0)
        // The engine is f32 natively
        .build_f32()?;

    let wave_src_2 = WaveFormBuilder::new(2, SampleRate::Sr44100, WaveFormType::Sine, 0.8, 800.0)
        // The engine is f32 natively
        .build_f32()?;

    let graph = engine.as_node_graph().unwrap();
    let mut src_node = SourceNodeBuilder::new(&graph, &wave_src_2).build()?;

    // Connect a second source node to the endpoint. They will be mixed there
    let mut end_node = graph.endpoint().unwrap();
    src_node.attach_output_bus(0, &mut end_node, 0)?;

    // Start the sound
    // The source node added earlier will start feeding sound into the engine immediately.
    // This is not the same behaviour as a Sound node.
    // Sound::play_sound() only controls this `Sound` instance.
    // It does not control the engine's node graph.
    //
    // The node we attached earlier (src_node -> endpoint) is already part of the engine graph,
    // so it will be pulled/mixed by the engine as long as the engine is running.
    // Starting/stopping this `Sound` only adds/removes this additional source from the mix.
    //
    // If you created the engine with `no_auto_start`, call `engine.start()` first.
    let mut sound = engine.new_sound_from_source(&wave_src)?;
    sound.play_sound()?;
    println!("Stopping in 5 seconds...");
    thread::sleep(Duration::from_secs(1));
    wave_src.set_amplitude(0.9)?;
    thread::sleep(Duration::from_secs(1));
    wave_src.set_frequency(500.0)?;
    thread::sleep(Duration::from_secs(3));
    sound.stop_sound()?;
    Ok(())
}
