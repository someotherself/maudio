use std::path::PathBuf;

use maudio::{
    engine::{
        node_graph::{
            node_builder::NodeBuilder,
            node_on_process::{EffectCallback, InputBusses, OutputBusses},
            nodes::NodeOps,
            NodeGraphOps,
        },
        Engine, EngineOps,
    },
    sound::sound_builder::SoundBuilder,
    MaResult,
};

struct Gain {
    gain: f32,
}

impl EffectCallback for Gain {
    fn on_audio(&mut self, input: &InputBusses, output: &mut OutputBusses) -> MaResult<u32> {
        let Some(input_bus) = input.get_bus(0) else {
            return Ok(0);
        };

        let Some(output_bus) = output.get_mut_bus(0) else {
            return Ok(0);
        };

        for (out, input) in output_bus.iter_mut().zip(input_bus.iter()) {
            *out = *input * self.gain;
        }

        Ok(input.frame_count(0).unwrap_or(0))
    }
}

fn main() -> MaResult<()> {
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let engine = Engine::new()?;
    let node_graph = engine.as_node_graph().unwrap();
    let mut endpoint = node_graph.endpoint().unwrap();

    let mut gain_node = NodeBuilder::effect().build(&node_graph, Gain { gain: 0.5 })?;
    gain_node.attach_output_bus(0, &mut endpoint, 0)?;

    let mut sound = SoundBuilder::new(&engine)
        .initial_attachment(&gain_node, 0)
        .file_path(&path)
        .build()?;

    // sound.set_volume(0.5); // adjust volume if needed

    sound.play_sound()?;

    println!("Stopping in 5 seconds...");
    std::thread::sleep(std::time::Duration::from_secs(5));

    sound.stop_sound()?;

    Ok(())
}
