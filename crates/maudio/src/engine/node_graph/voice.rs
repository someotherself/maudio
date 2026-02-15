//! Convenience wrapper for a ready-to-use voice node graph.
//!
//! `VoiceStack` wires together waveform and pulse wave source nodes with a mixer
//! and exposes the underlying nodes for further customization.
use crate::{
    audio::sample_rate::SampleRate,
    data_source::sources::{
        pulsewave::{PulseWave, PulseWaveBuilder},
        waveform::{WaveForm, WaveFormBuilder},
    },
    engine::node_graph::{
        nodes::{
            routing::splitter::{SplitterNode, SplitterNodeBuilder},
            source::source_node::{AttachedSourceNode, AttachedSourceNodeBuilder},
            NodeOps, NodeRef,
        },
        AsNodeGraphPtr, NodeGraphOps,
    },
    sound::Sound,
    ErrorKinds, MaResult, MaudioError,
};

/// ## Start/Stop/set_volume
///
/// There are no helpers to directly set any of these parameters as it may lead to confusing situations.
/// If it was created via an `Engine`, it's best to manage this using the `Engine`.
/// If not, `VoiceStack` provides easy access to the nodes where these actions can be done.
///
/// It's best that these actions are done as late in the chain as possible - at the `endoint` out node.
///
/// Only if finer control over the outcome is needed should you move earlier in the stack.
/// Be aware that the mixer node may have multiple outputs (see `self.mixer.out_bus_count()`).
pub struct VoiceStack<'g> {
    // Primary way
    pub output: NodeRef<'g>, // 'g is borrowed from the node_graph
    pub mix: SplitterNode<'g>,
    pub wave_nodes: Vec<AttachedSourceNode<'g, WaveForm<f32>>>, // engine and node graps are implicitly at f32
    pub pulse_nodes: Vec<AttachedSourceNode<'g, PulseWave<f32>>>,
    pub sounds: Vec<Sound<'g>>,
    state: VoiceState,
}

#[derive(Default)]
struct VoiceState {}

impl VoiceStack<'_> {}

pub struct VoiceBuilder<'g, N: AsNodeGraphPtr> {
    node_graph: &'g N,
    sample_rate: SampleRate,
    wave_builders: Vec<WaveFormBuilder>,
    pulse_builders: Vec<PulseWaveBuilder>,
    splitter_out: u32,
}

impl<'g, N: AsNodeGraphPtr> VoiceBuilder<'g, N> {
    pub fn new(node_graph: &'g N, sample_rate: SampleRate) -> Self {
        Self {
            node_graph,
            sample_rate,
            wave_builders: Vec::new(),
            pulse_builders: Vec::new(),
            splitter_out: 1,
        }
    }

    pub fn splitter_outputs(&mut self, outputs: u32) -> &mut Self {
        self.splitter_out = outputs;
        self
    }

    pub fn sine(&mut self, frequency: f64, amplitude: f64) -> &mut Self {
        let channels = self.node_graph.channels();
        let mut wave = WaveFormBuilder::new_sine(self.sample_rate, frequency);
        wave.channels(channels).amplitude(amplitude);
        self.wave_builders.push(wave);
        self
    }

    pub fn square(&mut self, frequency: f64, amplitude: f64) -> &mut Self {
        let channels = self.node_graph.channels();
        let mut wave = WaveFormBuilder::new_square(self.sample_rate, frequency);
        wave.channels(channels).amplitude(amplitude);
        self.wave_builders.push(wave);
        self
    }

    pub fn sawtooth(&mut self, frequency: f64, amplitude: f64) -> &mut Self {
        let channels = self.node_graph.channels();
        let mut wave = WaveFormBuilder::new_sawtooth(self.sample_rate, frequency);
        wave.channels(channels).amplitude(amplitude);
        self.wave_builders.push(wave);
        self
    }

    pub fn triangle(&mut self, frequency: f64, amplitude: f64) -> &mut Self {
        let channels = self.node_graph.channels();
        let mut wave = WaveFormBuilder::new_triangle(self.sample_rate, frequency);
        wave.channels(channels).amplitude(amplitude);
        self.wave_builders.push(wave);
        self
    }

    pub fn pulse(&mut self, amplitude: f64, frequency: f64, duty_cycle: f64) -> &mut Self {
        let channels = self.node_graph.channels();
        let pulse =
            PulseWaveBuilder::new(channels, self.sample_rate, amplitude, frequency, duty_cycle);
        self.pulse_builders.push(pulse);
        self
    }

    pub fn build(&mut self) -> MaResult<VoiceStack<'g>> {
        let graph_ref = self.node_graph;
        let mut endpoint = graph_ref
            .endpoint()
            .ok_or(MaudioError::new_ma_error(ErrorKinds::InvalidGraphState))?;
        let mut mixer = SplitterNodeBuilder::new(graph_ref, self.node_graph.channels()).build()?;

        // TODO: Check how many outputs it has. How will dsp nodes will be added later?
        // TODO: Add option to not connect it?
        // TODO: Keep track of the connected mixer output busses
        mixer.attach_output_bus(0, &mut endpoint, 0)?;

        let mut waves = Vec::with_capacity(self.wave_builders.len());
        let mut pulses = Vec::with_capacity(self.pulse_builders.len());
        let sounds = Vec::new();
        for wave_builder in &mut self.wave_builders {
            let wave = wave_builder.build_f32()?;
            let mut attach = AttachedSourceNodeBuilder::new(graph_ref, wave).build()?;
            attach.attach_output_bus(0, &mut mixer, 0)?;
            waves.push(attach);
        }
        for pulse_builder in &mut self.pulse_builders {
            let pulse = pulse_builder.build_f32()?;
            let mut attach = AttachedSourceNodeBuilder::new(graph_ref, pulse).build()?;
            attach.attach_output_bus(0, &mut mixer, 0)?;
            pulses.push(attach);
        }
        Ok(VoiceStack {
            output: endpoint,
            mix: mixer,
            wave_nodes: waves,
            pulse_nodes: pulses,
            sounds,
            state: VoiceState::default(),
        })
    }
}

#[cfg(test)]
mod test {
    use crate::{
        audio::sample_rate::SampleRate,
        data_source::sources::waveform::WaveFormBuilder,
        engine::{node_graph::nodes::source::source_node::SourceNodeBuilder, Engine, EngineOps},
    };

    #[test]
    fn voice_test_attached_node_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
        let graph = engine.as_node_graph().unwrap();
        let src = WaveFormBuilder::new_sine(SampleRate::Sr44100, 500.0)
            .build_f32()
            .unwrap();
        let _node = SourceNodeBuilder::new(&graph, &src).build().unwrap();
        // drop(src); // Does not work
        // node.state().unwrap();
    }
}
