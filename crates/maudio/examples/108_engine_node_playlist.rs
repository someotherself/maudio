use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use maudio::{
    audio::sample_rate::SampleRate,
    data_source::sources::decoder::{Decoder, DecoderBuilder, Fs},
    engine::{
        node_graph::{
            nodes::{
                source::source_node::{AttachedSourceNode, AttachedSourceNodeBuilder},
                NodeOps,
            },
            NodeGraphOps, NodeGraphRef,
        },
        Engine,
    },
    MaResult,
};

// This example shows a very simple playlist using source nodes.
//
// `Sound` is the easiest way to play audio with the engine, but miniaudio also
// lets us build the same thing manually with the node graph.
//
// In this version, each playlist item is:
//
// path -> Decoder -> AttachedSourceNode -> graph endpoint
//
// This gives more control than `Sound`, but also requires more work:
// - create a decoder for each file,
// - create a source node from the decoder,
// - attach the source node to the graph endpoint,
// - poll the decoder cursor to detect when playback has reached the end.
//
// Unlike the `Sound` example, this example does not use an `EndNotifier`.
// Instead, `current_finished()` compares the decoder cursor with the decoder
// length. When the cursor reaches the end, the playlist drops the current node
// and starts the next queued file.
//
// This is still a polling example. A real application would usually integrate
// this logic into an existing control loop, event loop, or engine-host thread
// instead of sleeping in a dedicated loop.

pub enum Command {
    Next,
    Shutdown,
    Add { path: PathBuf },
}

struct PlayList {
    current: Option<AttachedSourceNode<Decoder<f32, Fs>>>,
    queue: VecDeque<PathBuf>,
}

impl PlayList {
    fn new() -> Self {
        Self {
            current: None,
            queue: VecDeque::new(),
        }
    }

    fn add_sound(&mut self, path: impl AsRef<Path>) {
        self.queue.push_back(path.as_ref().to_path_buf());
    }

    fn play_next(&mut self, graph: &NodeGraphRef) -> MaResult<()> {
        self.current = None;

        let Some(path) = self.queue.pop_front() else {
            return Ok(());
        };

        let decoder =
            DecoderBuilder::new_f32(graph.channels(), SampleRate::Sr44100).from_file(&path)?;
        let mut node = AttachedSourceNodeBuilder::new(graph, decoder).build()?;
        let mut endpoint = graph.endpoint();
        node.attach_output_bus(0, &mut endpoint, 0)?;
        self.current = Some(node);

        Ok(())
    }

    fn current_finished(&self) -> MaResult<bool> {
        let Some(current) = self.current.as_ref() else {
            return Ok(true);
        };

        let decoder = current.as_source_ref();

        let cursor = decoder.cursor_in_pcm_frames()?;
        let length = decoder.length_in_pcm_frames()?;

        Ok(cursor >= length)
    }

    fn run(&mut self, graph: &NodeGraphRef) -> MaResult<()> {
        if self.current.is_none() {
            self.play_next(graph)?;
        }

        while self.current.is_some() {
            if self.current_finished()? {
                self.play_next(graph)?;
            }

            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        Ok(())
    }
}

fn main() -> MaResult<()> {
    if cfg!(not(feature = "vorbis")) {
        println!("Run using: cargo run --features vorbis --example 108_engine_node_playlist");
        return Ok(());
    }
    let path_1 = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));
    let path_2 = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/48000-stereo.ogg"
    ));

    let engine = Engine::new()?;
    let graph = engine.as_node_graph();
    let mut playlist = PlayList::new();

    playlist.add_sound(path_1);
    playlist.add_sound(path_2);

    playlist.run(&graph)?;

    Ok(())
}
