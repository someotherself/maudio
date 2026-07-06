use maudio::{
    engine::{
        node_graph::{node_builder::NodeBuilder, node_on_process::EffectCallback},
        Engine, EngineOps,
    },
    MaResult,
};

/// A very simple demonstration splitter node.
///
/// This node copies the audio from its single input bus into every output bus.
///
/// miniaudio already provides a built-in splitter node, so this custom node is
/// only useful as an example of how to build nodes with multiple output buses.
struct Split;

impl EffectCallback for Split {
    fn on_audio(
        &mut self,
        input: &maudio::engine::node_graph::node_on_process::InputBusses,
        output: &mut maudio::engine::node_graph::node_on_process::OutputBusses,
    ) -> MaResult<u32> {
        let Some(input_bus) = input.get_bus(0) else {
            return Ok(0);
        };

        for bus_index in 0..output.len() {
            if let Some(output_bus) = output.get_mut_bus(bus_index) {
                output_bus.copy_from_slice(input_bus);
            }
        }

        Ok(input.frame_count(0).unwrap_or(0))
    }
}

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let node_graph = engine.as_node_graph().unwrap();

    let node_custom = Split {};

    let _split_node = NodeBuilder::effect()
        .add_output_bus(None)
        .build(&node_graph, node_custom)?;

    Ok(())
}
