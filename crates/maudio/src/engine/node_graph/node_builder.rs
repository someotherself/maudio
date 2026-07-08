//! Custom node builders.
//!
//! This module provides the builder API for creating custom nodes in a
//! [`NodeGraph`](crate::engine::node_graph).
//!
//! Custom nodes are organized around the shape of their processing callback.
//! Each builder's `build` method accepts a user-defined processor type that
//! implements the matching callback trait:
//!
//! - passthrough nodes use     [`SinkCallback`];
//! - source nodes use          [`SourceCallback`];
//! - processor nodes use       [`EffectCallback`];
//! - resampler nodes use       [`TransformCallback`].
//!
//! The callback trait determines which audio buffers the processor receives
//! during processing. For example, a source processor writes output without
//! receiving input, while a processor node receives input and writes output.
//!
//! Each builder initializes the node with the bus layout and node behavior
//! expected for that callback shape. This mainly means choosing the initial input
//! and output bus counts, and enabling any node behavior required by miniaudio.
//!
//! Choosing one builder does not necessarily close off every other behavior a
//! node can have. The builder selects the callback contract and default
//! initialization, while additional builder methods can enable supported node
//! graph behavior where it makes sense.
//!
//! # Example
//!
//! ```no_run
//! use maudio::{
//!     engine::{
//!         node_graph::{
//!             node_builder::NodeBuilder,
//!             node_on_process::SinkCallback,
//!         },
//!         Engine,
//!         EngineOps,
//!     },
//!     MaResult,
//! };
//!
//! struct MeterNode {
//!     peak: f32,
//! }
//!
//! impl SinkCallback for MeterNode {
//!     fn on_audio(&mut self, input: &[f32]) -> MaResult<()> {
//!         self.peak = input
//!             .iter()
//!             .copied()
//!             .map(f32::abs)
//!             .fold(0.0, f32::max);
//!
//!         Ok(())
//!     }
//! }
//!
//! fn main() -> MaResult<()> {
//!     let engine = Engine::new()?;
//!     let node_graph = engine.as_node_graph().unwrap();
//!
//!     let meter = MeterNode { peak: 0.0 };
//!
//!     let _node = NodeBuilder::sink()
//!         .build(&node_graph, meter)?;
//!
//!     Ok(())
//! }
//! ```
use maudio_sys::ffi as sys;

use crate::{
    engine::node_graph::{
        node_flags::NodeFlags,
        node_on_process::{
            Effect, EffectCallback, InputDemandCallback, Sink, SinkCallback, Source,
            SourceCallback, Transform, TransformCallback, TransformInputDemand,
        },
        nodes::{Node, NodeBusChannelsConfig, NodeState},
        AsNodeGraphPtr,
    },
    MaResult,
};

pub(crate) enum NodeFunction {
    Passthrough,
    Source,
    Process,
    Resampler,
}

/// Builder entry point for custom node types.
///
/// Custom nodes are split into four builder types based on the shape of the
/// processing callback:
///
/// - [`NodeBuilder::sink`] creates a node that only receives input.
/// - [`NodeBuilder::source`] creates a node that only writes output.
/// - [`NodeBuilder::effect`] creates a node that receives input and writes output.
/// - [`NodeBuilder::transformer`] creates a process-style node where the number of
///   input frames consumed may differ from the number of output frames produced.
///
/// These builder types describe how the node is initialized and which callback
/// trait the custom processor must implement. They do not necessarily restrict
/// every behavior the node can have. For example, a source node can still be
/// marked as passthrough when that is useful for node graph behavior.
pub struct NodeBuilder {}

pub struct CustomSinkNodeBuilder {
    config: sys::ma_node_config,
    busses: NodeBusChannelsConfig,
    flags: NodeFlags,
}

impl CustomSinkNodeBuilder {
    pub fn build<'a, C: SinkCallback, N: AsNodeGraphPtr>(
        &mut self,
        node_graph: &'a N,
        custom: C,
    ) -> MaResult<Node<'a, Sink<C>>> {
        let custom = Sink(custom);
        Node::build(
            &mut self.config,
            self.flags,
            custom,
            node_graph,
            NodeFunction::Passthrough,
            &self.busses,
        )
    }

    /// Change the channel count of the input bus
    pub fn input_channel_count(&mut self, channels: u32) -> &mut Self {
        self.busses.change_chanels_in(0, channels);
        self
    }

    pub fn initial_state(&mut self, state: NodeState) -> *mut Self {
        self.config.initialState = state.into();
        self
    }
}

pub struct CustomSourceNodeBuilder {
    config: sys::ma_node_config,
    busses: NodeBusChannelsConfig,
    flags: NodeFlags,
}

impl CustomSourceNodeBuilder {
    pub fn build<'a, C: SourceCallback, N: AsNodeGraphPtr>(
        &mut self,
        node_graph: &'a N,
        custom: C,
    ) -> MaResult<Node<'a, Source<C>>> {
        Node::build(
            &mut self.config,
            self.flags,
            Source(custom),
            node_graph,
            NodeFunction::Source,
            &self.busses,
        )
    }

    pub fn initial_state(&mut self, state: NodeState) -> *mut Self {
        self.config.initialState = state.into();
        self
    }

    /// Change the channel count of the outpus bus
    pub fn output_channel_count(&mut self, channels: u32) -> &mut Self {
        self.busses.change_chanels_out(0, channels);
        self
    }

    // TODO: Causes a null ptr dereference in miniaudio.
    // Do not enable until fixed
    /// Enables passthrough behavior for the source node.
    ///
    /// This allows the node graph to treat the node as passthrough where that
    /// behavior is useful, even though the node still uses the source callback
    /// shape.
    fn _passthrough(&mut self) -> &mut Self {
        self.flags.insert(NodeFlags::PASSTHROUGH);
        self
    }
}

pub struct CustomEffectNodeBuilder {
    config: sys::ma_node_config,
    busses: NodeBusChannelsConfig,
    flags: NodeFlags,
}

impl CustomEffectNodeBuilder {
    pub fn build<'a, C: EffectCallback, N: AsNodeGraphPtr>(
        &mut self,
        node_graph: &'a N,
        custom: C,
    ) -> MaResult<Node<'a, Effect<C>>> {
        Node::build(
            &mut self.config,
            self.flags,
            Effect(custom),
            node_graph,
            NodeFunction::Process,
            &self.busses,
        )
    }

    /// Enables continuous processing for the node.
    ///
    /// Normally, a node with input busses only has its processing callback called
    /// when input data is available from an attached upstream node. If no inputs are
    /// attached, or if the attached inputs do not provide any data, the callback may
    /// not be called.
    ///
    /// Continuous processing changes that behavior so the processing callback can be
    /// called even when no input data is received.
    ///
    /// This is useful for effects that still need to produce output after their
    /// input has stopped, such as delays, echoes, and reverbs with an audible tail.
    /// It can also be useful for nodes that need their callback to run even when no
    /// inputs are currently attached.
    pub fn continuous_processing(&mut self) -> &mut Self {
        self.flags.insert(NodeFlags::CONTINUOUS_PROCESSING);
        self
    }

    /// Allows the processing callback to receive null input when no input is available.
    ///
    /// This also enables [`continuous_processing`](Self::continuous_processing),
    /// because null input only applies when the processing callback can be called
    /// without input data from upstream nodes.
    ///
    /// After this method is called, if the node is processed without any input
    /// frames available, the callback receives no input busses instead of silent
    /// input buffers.
    ///
    /// This is useful when the processor needs to distinguish between missing input
    /// and real input that happens to contain silence.
    ///
    /// If this method is called, a callback that runs without input from upstream
    /// nodes receives an empty [`InputBusses`](crate::engine::node_graph::node_on_process) instead of silence-filled input
    /// buffers. The input will have zero busses, zero frames per bus, and
    /// [`InputBusses::null_input`](crate::engine::node_graph::node_on_process) will return `true`.
    pub fn allow_null_input(&mut self) -> &mut Self {
        self.flags.insert(NodeFlags::CONTINUOUS_PROCESSING);
        self.flags.insert(NodeFlags::ALLOW_NULL_INPUT);
        self
    }

    /// Marks the node's output as silent.
    ///
    /// This tells miniaudio that the node's output should not be mixed into
    /// downstream input busses, even though the processing callback may still run.
    ///
    /// The callback should still report how many frames it processed, this means
    /// returning the number of output frames processed. output frames processed.
    ///
    /// After this method is called, the callback does not need to write meaningful
    /// audio to the output buffers because miniaudio ignores the node's output when
    /// mixing. This is useful for side-effect nodes such as analyzers, meters,
    /// recorders, or branches that write audio to a file.
    pub fn silent_output(&mut self) -> &mut Self {
        self.flags.insert(NodeFlags::SILENT_OUTPUT);
        self
    }

    pub fn initial_state(&mut self, state: NodeState) -> *mut Self {
        self.config.initialState = state.into();
        self
    }

    /// Change the channel count of an input bus as `bus_index`
    pub fn set_in_channel_count(&mut self, bus_index: usize, channels: u32) -> &mut Self {
        self.busses.change_chanels_in(bus_index, channels);
        self
    }

    /// Change the channel count of an output bus at `bus_index`
    pub fn set_out_channel_count(&mut self, bus_index: usize, channels: u32) -> &mut Self {
        self.busses.change_chanels_out(bus_index, channels);
        self
    }

    /// Adds a new output bus with the channel count specified
    ///
    /// If channel count is `None`, the channel count of the NodeGraph will be used instead.
    pub fn add_output_bus(&mut self, channels: Option<u32>) -> &mut Self {
        self.busses.add_output_bus(channels);
        self
    }

    /// Adds a new output bus with the channel count specified
    ///
    /// If channel count is `None`, the channel count of the NodeGraph will be used instead.
    pub fn add_input_bus(&mut self, channels: Option<u32>) -> &mut Self {
        self.busses.add_input_bus(channels);
        self
    }

    /// Takes a slice where slice.len() is the number of output busses with a channel count
    /// specified for each bus.
    ///
    /// Will over-write any existing bus layout.
    ///
    /// Becomes a no-op is the slice has length zero, or if any channel count is zero.
    pub fn set_outputs(&mut self, busses: &[u32]) -> &mut Self {
        self.busses.set_outputs(busses);
        self
    }

    /// Takes a slice where slice.len() is the number of input busses with a channel count
    /// specified for each bus.
    ///
    /// Will over-write any existing bus layout.
    ///
    /// Becomes a no-op is the slice has length zero, or if any channel count is zero.
    pub fn set_inputs(&mut self, busses: &[u32]) -> &mut Self {
        self.busses.set_inputs(busses);
        self
    }
}

pub struct CustomTransformerNodeBuilder {
    config: sys::ma_node_config,
    busses: NodeBusChannelsConfig,
    flags: NodeFlags,
}

impl CustomTransformerNodeBuilder {
    pub fn build<'a, C: TransformCallback, N: AsNodeGraphPtr>(
        &mut self,
        node_graph: &'a N,
        custom: C,
    ) -> MaResult<Node<'a, Transform<C>>> {
        Node::build(
            &mut self.config,
            self.flags,
            Transform(custom),
            node_graph,
            NodeFunction::Resampler,
            &self.busses,
        )
    }

    /// Builds a transform node with an input-demand callback.
    ///
    /// This is intended for nodes where the number of input frames consumed may
    /// differ from the number of output frames produced, such as resamplers,
    /// time-stretchers, pitch shifters, or other rate-changing processors.
    ///
    /// In addition to [`TransformCallback`], the node implements [`InputDemandCallback`].
    ///
    /// This gives miniaudio a hint on how many input frames it should make available for N output frames
    ///
    /// Providing this estimate can reduce latency by avoiding unnecessary upstream
    /// reads. It is still only a scheduling hint: the transform callback remains
    /// responsible for reporting the actual input consumed and output produced.
    pub fn build_req_frames<'a, C, N>(
        &mut self,
        node_graph: &'a N,
        custom: C,
    ) -> MaResult<Node<'a, TransformInputDemand<C>>>
    where
        C: TransformCallback + InputDemandCallback,
        N: AsNodeGraphPtr,
    {
        Node::build_required_frames(
            &mut self.config,
            self.flags,
            TransformInputDemand(custom),
            node_graph,
            NodeFunction::Resampler,
            &self.busses,
        )
    }

    pub fn initial_state(&mut self, state: NodeState) -> *mut Self {
        self.config.initialState = state.into();
        self
    }

    /// Change the channel count of an input bus as `bus_index`
    pub fn set_in_channel_count(&mut self, bus_index: usize, channels: u32) -> &mut Self {
        self.busses.change_chanels_in(bus_index, channels);
        self
    }

    /// Change the channel count of an output bus at `bus_index`
    pub fn set_out_channel_count(&mut self, bus_index: usize, channels: u32) -> &mut Self {
        self.busses.change_chanels_out(bus_index, channels);
        self
    }

    /// Adds a new output bus with the channel count specified
    ///
    /// If channel count is `None`, the channel count of the NodeGraph will be used instead.
    pub fn add_output_bus(&mut self, channels: Option<u32>) -> &mut Self {
        self.busses.add_output_bus(channels);
        self
    }

    /// Adds a new output bus with the channel count specified
    ///
    /// If channel count is `None`, the channel count of the NodeGraph will be used instead.
    pub fn add_input_bus(&mut self, channels: Option<u32>) -> &mut Self {
        self.busses.add_input_bus(channels);
        self
    }

    /// Takes a slice where slice.len() is the number of output busses with a channel count
    /// specified for each bus.
    ///
    /// Will over-write any existing bus layout.
    ///
    /// Becomes a no-op is the slice has length zero, or if any channel count is zero.
    pub fn set_outputs(&mut self, busses: &[u32]) -> &mut Self {
        self.busses.set_outputs(busses);
        self
    }

    /// Takes a slice where slice.len() is the number of input busses with a channel count
    /// specified for each bus.
    ///
    /// Will over-write any existing bus layout.
    ///
    /// Becomes a no-op is the slice has length zero, or if any channel count is zero.
    pub fn set_inputs(&mut self, busses: &[u32]) -> &mut Self {
        self.busses.set_inputs(busses);
        self
    }

    /// Enables continuous processing for the node.
    ///
    /// Normally, a node with input busses only has its processing callback called
    /// when input data is available from an attached upstream node. If no inputs are
    /// attached, or if the attached inputs do not provide any data, the callback may
    /// not be called.
    ///
    /// Continuous processing changes that behavior so the processing callback can be
    /// called even when no input data is received.
    ///
    /// This is useful for effects that still need to produce output after their
    /// input has stopped, such as delays, echoes, and reverbs with an audible tail.
    /// It can also be useful for nodes that need their callback to run even when no
    /// inputs are currently attached.
    pub fn continuous_processing(&mut self) -> &mut Self {
        self.flags.insert(NodeFlags::CONTINUOUS_PROCESSING);
        self
    }

    /// Allows the processing callback to receive null input when no input is available.
    ///
    /// This also enables [`continuous_processing`](Self::continuous_processing),
    /// because null input only applies when the processing callback can be called
    /// without input data from upstream nodes.
    ///
    /// After this method is called, if the node is processed without any input
    /// frames available, the callback receives no input busses instead of silent
    /// input buffers.
    ///
    /// This is useful when the processor needs to distinguish between missing input
    /// and real input that happens to contain silence.
    ///
    /// If this method is called, a callback that runs without input from upstream
    /// nodes receives an empty [`InputBusses`](crate::engine::node_graph::node_on_process) instead of silence-filled input
    /// buffers. The input will have zero busses, zero frames per bus, and
    /// [`InputBusses::null_input`](crate::engine::node_graph::node_on_process) will return `true`.
    pub fn allow_null_input(&mut self) -> &mut Self {
        self.flags.insert(NodeFlags::CONTINUOUS_PROCESSING);
        self.flags.insert(NodeFlags::ALLOW_NULL_INPUT);
        self
    }

    /// Marks the node's output as silent.
    ///
    /// This tells miniaudio that the node's output should not be mixed into
    /// downstream input busses, even though the processing callback may still run.
    ///
    /// The callback should still report how many frames it processed, this
    /// means returning both the input frames consumed and the output frames processed.
    ///
    /// After this method is called, the callback does not need to write meaningful
    /// audio to the output buffers because miniaudio ignores the node's output when
    /// mixing. This is useful for side-effect nodes such as analyzers, meters,
    /// recorders, or branches that write audio to a file.
    pub fn silent_output(&mut self) -> &mut Self {
        self.flags.insert(NodeFlags::SILENT_OUTPUT);
        self
    }
}

impl NodeBuilder {
    /// Creates a sink node builder.
    ///
    /// The node is initialized with one input bus, no output busses. This is useful
    /// for nodes that inspect or consume input without producing output.
    ///
    /// The input or outpus bus count cannot be changed.
    pub fn sink() -> CustomSinkNodeBuilder {
        let config = unsafe { sys::ma_node_config_init() };
        let busses = NodeBusChannelsConfig::new(1, 0, None);

        CustomSinkNodeBuilder {
            config,
            busses,
            flags: NodeFlags::NONE,
        }
    }

    /// Creates a source node builder.
    ///
    /// The node is initialized with no input busses and one output bus. This is
    /// useful for nodes that generate audio rather than processing audio from
    /// another node.
    ///
    /// The input or outpus bus count cannot be changed.
    pub fn source() -> CustomSourceNodeBuilder {
        let config = unsafe { sys::ma_node_config_init() };
        let busses = NodeBusChannelsConfig::new(0, 1, None);

        CustomSourceNodeBuilder {
            config,
            busses,
            flags: NodeFlags::NONE,
        }
    }

    /// Creates a processor node builder.
    ///
    /// The node is initialized with one input bus and one output bus. This is
    /// useful for effects and processors that consume input audio and write
    /// processed output audio at the same frame rate.
    ///
    /// The input and output bus counts can be changed.
    pub fn effect() -> CustomEffectNodeBuilder {
        let config = unsafe { sys::ma_node_config_init() };
        let busses = NodeBusChannelsConfig::new(1, 1, None);

        CustomEffectNodeBuilder {
            config,
            busses,
            flags: NodeFlags::NONE,
        }
    }

    /// Creates a resampler node builder.
    ///
    /// The node is initialized with one input bus and one output bus, and is
    /// configured for different input and output processing rates. This is useful
    /// for nodes that may consume a different number of input frames than the
    /// number of output frames they produce.
    pub fn transformer() -> CustomTransformerNodeBuilder {
        let mut config = unsafe { sys::ma_node_config_init() };
        config.inputBusCount = 1;
        config.outputBusCount = 1;
        let busses = NodeBusChannelsConfig::new(1, 1, None);

        CustomTransformerNodeBuilder {
            config,
            busses,
            flags: NodeFlags::DIFFERENT_PROCESSING_RATES,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        node_graph::{node_on_process::ProcessResult, nodes::NodeInner, NodeGraphOps},
        Engine, EngineOps,
    };

    #[test]
    fn node_bus_test_channels_config_new_allows_graph_channel_fallback() {
        let config = NodeBusChannelsConfig::new(2, 1, None);

        assert_eq!(config.inputs, vec![None, None]);
        assert_eq!(config.outputs, vec![None]);
    }

    #[test]
    fn node_bus_test_channels_config_build_nodes_uses_explicit_channels() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let config = NodeBusChannelsConfig {
            inputs: vec![Some(1), Some(2)],
            outputs: vec![Some(4), Some(6)],
        };

        let busses = config.build_nodes(&node_graph);

        assert_eq!(busses.inputs, vec![1, 2]);
        assert_eq!(busses.outputs, vec![4, 6]);
    }

    #[test]
    fn node_bus_test_channels_config_build_nodes_replaces_none_with_graph_channels() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let graph_channels = node_graph.channels();

        let config = NodeBusChannelsConfig {
            inputs: vec![Some(1), None, Some(4)],
            outputs: vec![None, Some(6)],
        };

        let busses = config.build_nodes(&node_graph);

        assert_eq!(busses.inputs, vec![1, graph_channels, 4]);
        assert_eq!(busses.outputs, vec![graph_channels, 6]);
    }

    #[test]
    fn node_bus_test_channels_config_set_inputs_replaces_inputs() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(2));

        config.set_inputs(&[1, 2, 6]);

        assert_eq!(config.inputs, vec![Some(1), Some(2), Some(6)]);
        assert_eq!(config.outputs, vec![Some(2)]);
    }

    #[test]
    fn node_bus_test_channels_config_set_outputs_replaces_outputs() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(2));

        config.set_outputs(&[2, 4]);

        assert_eq!(config.inputs, vec![Some(2)]);
        assert_eq!(config.outputs, vec![Some(2), Some(4)]);
    }

    #[test]
    fn node_bus_test_channels_config_set_inputs_ignores_empty_slice() {
        let mut config = NodeBusChannelsConfig::new(2, 1, Some(2));

        config.set_inputs(&[]);

        assert_eq!(config.inputs, vec![Some(2), Some(2)]);
    }

    #[test]
    fn node_bus_test_channels_config_set_outputs_ignores_empty_slice() {
        let mut config = NodeBusChannelsConfig::new(1, 2, Some(2));

        config.set_outputs(&[]);

        assert_eq!(config.outputs, vec![Some(2), Some(2)]);
    }

    #[test]
    fn node_bus_test_channels_config_set_inputs_ignores_zero_channel_bus() {
        let mut config = NodeBusChannelsConfig::new(2, 1, Some(2));

        config.set_inputs(&[1, 0, 4]);

        assert_eq!(config.inputs, vec![Some(2), Some(2)]);
    }

    #[test]
    fn node_bus_test_channels_config_set_outputs_ignores_zero_channel_bus() {
        let mut config = NodeBusChannelsConfig::new(1, 2, Some(2));

        config.set_outputs(&[1, 0, 4]);

        assert_eq!(config.outputs, vec![Some(2), Some(2)]);
    }

    #[test]
    fn node_bus_test_channels_config_change_channels_in_updates_existing_bus() {
        let mut config = NodeBusChannelsConfig::new(2, 1, Some(2));

        config.change_chanels_in(1, 6);

        assert_eq!(config.inputs, vec![Some(2), Some(6)]);
    }

    #[test]
    fn node_bus_test_channels_config_change_channels_out_updates_existing_bus() {
        let mut config = NodeBusChannelsConfig::new(1, 2, Some(2));

        config.change_chanels_out(0, 6);

        assert_eq!(config.outputs, vec![Some(6), Some(2)]);
    }

    #[test]
    fn node_bus_test_channels_config_change_channels_in_ignores_out_of_bounds_index() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(2));

        config.change_chanels_in(1, 6);

        assert_eq!(config.inputs, vec![Some(2)]);
    }

    #[test]
    fn node_bus_test_channels_config_change_channels_out_ignores_out_of_bounds_index() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(2));

        config.change_chanels_out(1, 6);

        assert_eq!(config.outputs, vec![Some(2)]);
    }

    #[test]
    fn node_bus_test_channels_config_change_channels_in_allows_zero_currently() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(2));

        config.change_chanels_in(0, 0);

        assert_eq!(config.inputs, vec![Some(0)]);
    }

    #[test]
    fn node_bus_test_channels_config_change_channels_out_allows_zero_currently() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(2));

        config.change_chanels_out(0, 0);

        assert_eq!(config.outputs, vec![Some(0)]);
    }

    #[test]
    fn node_bus_test_channels_config_add_input_bus_adds_explicit_channels() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(2));

        config.add_input_bus(Some(6));

        assert_eq!(config.inputs, vec![Some(2), Some(6)]);
    }

    #[test]
    fn node_bus_test_channels_config_add_output_bus_adds_explicit_channels() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(2));

        config.add_output_bus(Some(6));

        assert_eq!(config.outputs, vec![Some(2), Some(6)]);
    }

    #[test]
    fn node_bus_test_channels_config_add_input_bus_adds_graph_channel_fallback() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(4));

        config.add_input_bus(None);

        assert_eq!(config.inputs, vec![Some(4), None]);
    }

    #[test]
    fn node_bus_test_channels_config_add_output_bus_adds_graph_channel_fallback() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(4));

        config.add_output_bus(None);

        assert_eq!(config.outputs, vec![Some(4), None]);
    }

    #[test]
    fn node_bus_test_channels_config_add_input_bus_ignores_zero_channels() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(2));

        config.add_input_bus(Some(0));

        assert_eq!(config.inputs, vec![Some(2)]);
    }

    #[test]
    fn node_bus_test_channels_config_add_output_bus_ignores_zero_channels() {
        let mut config = NodeBusChannelsConfig::new(1, 1, Some(2));

        config.add_output_bus(Some(0));

        assert_eq!(config.outputs, vec![Some(2)]);
    }

    #[test]
    fn node_bus_test_channels_config_added_none_resolves_to_graph_channels_when_built() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let graph_channels = node_graph.channels();

        let mut config = NodeBusChannelsConfig::new(1, 1, Some(4));
        config.add_input_bus(None);
        config.add_output_bus(None);

        let busses = config.build_nodes(&node_graph);

        assert_eq!(busses.inputs, vec![4, graph_channels]);
        assert_eq!(busses.outputs, vec![4, graph_channels]);
    }

    #[derive(Debug)]
    struct TestSink {
        value: u32,
    }

    impl SinkCallback for TestSink {
        fn on_audio(&mut self, _input: &[f32]) -> MaResult<()> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct TestSource {
        value: u32,
    }

    impl SourceCallback for TestSource {
        fn on_audio(&mut self, _output: &mut [f32]) -> MaResult<u32> {
            Ok(0)
        }
    }

    #[derive(Debug)]
    struct TestEffect {
        value: u32,
    }

    impl EffectCallback for TestEffect {
        fn on_audio(
            &mut self,
            _input: &crate::engine::node_graph::node_on_process::InputBusses,
            _output: &mut crate::engine::node_graph::node_on_process::OutputBusses,
        ) -> MaResult<u32> {
            Ok(0)
        }
    }

    #[derive(Debug)]
    struct TestTransform {
        value: u32,
    }

    impl TransformCallback for TestTransform {
        fn on_audio(
            &mut self,
            _input: &crate::engine::node_graph::node_on_process::InputBusses,
            _output: &mut crate::engine::node_graph::node_on_process::OutputBusses,
        ) -> MaResult<crate::engine::node_graph::node_on_process::ProcessResult> {
            Ok(ProcessResult {
                frames_in_consumed: 0,
                frames_out_written: 0,
            })
        }
    }

    impl InputDemandCallback for TestTransform {
        fn required_input_frames(&mut self, out_frames: u32) -> MaResult<u32> {
            Ok(out_frames * 2)
        }
    }

    fn node_inner<'a, C>(node: &'a Node<'_, C>) -> &'a NodeInner<'a, C> {
        unsafe { &*node.inner }
    }

    fn assert_vtable<C>(
        inner: &NodeInner<'_, C>,
        expected_inputs: u32,
        expected_outputs: u32,
        expected_has_required_frames_callback: bool,
    ) {
        assert!(!inner.vtable.is_null());

        let vtable = unsafe { &*inner.vtable };

        assert_eq!(vtable.inputBusCount as u32, expected_inputs);
        assert_eq!(vtable.outputBusCount as u32, expected_outputs);
        assert!(vtable.onProcess.is_some());

        assert_eq!(
            vtable.onGetRequiredInputFrameCount.is_some(),
            expected_has_required_frames_callback
        );
    }

    #[test]
    fn node_build_test_sink_builder_builds_passthrough_node_with_one_input_and_no_outputs() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut builder = NodeBuilder::sink();
        builder.input_channel_count(1);

        let node = builder.build(&node_graph, TestSink { value: 10 }).unwrap();

        assert_eq!(node.value, 10);

        let inner = node_inner(&node);

        assert!(matches!(inner.op, NodeFunction::Passthrough));
        assert_eq!(inner.busses.inputs, vec![1]);
        assert_eq!(inner.busses.outputs, Vec::<u32>::new());

        assert_vtable(inner, 1, 0, false);
    }

    #[test]
    fn node_build_test_sink_builder_uses_graph_channels_by_default() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let graph_channels = node_graph.channels();

        let mut builder = NodeBuilder::sink();

        let node = builder.build(&node_graph, TestSink { value: 10 }).unwrap();

        let inner = node_inner(&node);

        assert_eq!(inner.busses.inputs, vec![graph_channels]);
        assert_eq!(inner.busses.outputs, Vec::<u32>::new());

        assert_vtable(inner, 1, 0, false);
    }

    #[test]
    fn node_build_test_source_builder_builds_source_node_with_no_inputs_and_one_output() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut builder = NodeBuilder::source();
        builder.output_channel_count(6);

        let node = builder
            .build(&node_graph, TestSource { value: 20 })
            .unwrap();

        assert_eq!(node.value, 20);

        let inner = node_inner(&node);

        assert!(matches!(inner.op, NodeFunction::Source));
        assert_eq!(inner.busses.inputs, Vec::<u32>::new());
        assert_eq!(inner.busses.outputs, vec![6]);

        assert_vtable(inner, 0, 1, false);
    }

    // #[test]
    // fn node_build_test_source_builder_builds_source_node_pass_with_no_inputs_and_one_output() {
    //     let engine = Engine::new_for_tests().unwrap();
    //     let node_graph = engine.as_node_graph().unwrap();

    //     let mut builder = NodeBuilder::source();
    //     builder.output_channel_count(6).passthrough();

    //     let node = builder
    //         .build(&node_graph, TestSource { value: 20 })
    //         .unwrap();

    //     assert_eq!(node.value, 20);

    //     let inner = node_inner(&node);

    //     assert!(matches!(inner.op, NodeFunction::Source));
    //     assert_eq!(inner.busses.inputs, Vec::<u32>::new());
    //     assert_eq!(inner.busses.outputs, vec![6]);

    //     assert_vtable(inner, 0, 1, false);
    // }

    #[test]
    fn node_build_test_source_builder_uses_graph_channels_by_default() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let graph_channels = node_graph.channels();

        let mut builder = NodeBuilder::source();

        let node = builder
            .build(&node_graph, TestSource { value: 20 })
            .unwrap();

        let inner = node_inner(&node);

        assert_eq!(inner.busses.inputs, Vec::<u32>::new());
        assert_eq!(inner.busses.outputs, vec![graph_channels]);

        assert_vtable(inner, 0, 1, false);
    }

    #[test]
    fn node_build_test_effect_builder_builds_process_node_with_default_one_input_and_one_output() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let graph_channels = node_graph.channels();

        let mut builder = NodeBuilder::effect();

        let node = builder
            .build(&node_graph, TestEffect { value: 30 })
            .unwrap();

        assert_eq!(node.value, 30);

        let inner = node_inner(&node);

        assert!(matches!(inner.op, NodeFunction::Process));
        assert_eq!(inner.busses.inputs, vec![graph_channels]);
        assert_eq!(inner.busses.outputs, vec![graph_channels]);

        assert_vtable(inner, 1, 1, false);
    }

    #[test]
    fn node_build_test_effect_builder_builds_process_node_with_custom_bus_layout() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let graph_channels = node_graph.channels();

        let mut builder = NodeBuilder::effect();
        builder
            .set_inputs(&[1, 2])
            .add_input_bus(None)
            .set_outputs(&[4])
            .add_output_bus(None);

        let node = builder
            .build(&node_graph, TestEffect { value: 30 })
            .unwrap();

        let inner = node_inner(&node);

        assert!(matches!(inner.op, NodeFunction::Process));
        assert_eq!(inner.busses.inputs, vec![1, 2, graph_channels]);
        assert_eq!(inner.busses.outputs, vec![4, graph_channels]);

        assert_vtable(inner, 3, 2, false);
    }

    #[test]
    fn node_build_test_transformer_builder_builds_resampler_node_with_default_one_input_and_one_output(
    ) {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let graph_channels = node_graph.channels();

        let mut builder = NodeBuilder::transformer();

        let node = builder
            .build(&node_graph, TestTransform { value: 40 })
            .unwrap();

        assert_eq!(node.value, 40);

        let inner = node_inner(&node);

        assert!(matches!(inner.op, NodeFunction::Resampler));
        assert_eq!(inner.busses.inputs, vec![graph_channels]);
        assert_eq!(inner.busses.outputs, vec![graph_channels]);

        assert_vtable(inner, 1, 1, false);
    }

    #[test]
    fn node_build_test_transformer_builder_builds_resampler_node_with_custom_bus_layout() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let graph_channels = node_graph.channels();

        let mut builder = NodeBuilder::transformer();
        builder
            .set_inputs(&[1])
            .add_input_bus(None)
            .set_outputs(&[2, 4])
            .add_output_bus(None);

        let node = builder
            .build(&node_graph, TestTransform { value: 40 })
            .unwrap();

        let inner = node_inner(&node);

        assert!(matches!(inner.op, NodeFunction::Resampler));
        assert_eq!(inner.busses.inputs, vec![1, graph_channels]);
        assert_eq!(inner.busses.outputs, vec![2, 4, graph_channels]);

        assert_vtable(inner, 2, 3, false);
    }

    #[test]
    fn node_build_test_transformer_builder_build_req_frames_uses_required_frames_vtable() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut builder = NodeBuilder::transformer();

        let node = builder
            .build_req_frames(&node_graph, TestTransform { value: 50 })
            .unwrap();

        assert_eq!(node.value, 50);

        let inner = node_inner(&node);

        assert!(matches!(inner.op, NodeFunction::Resampler));
        assert_vtable(inner, 1, 1, true);
    }

    #[test]
    fn node_build_test_transformer_builder_build_req_frames_keeps_custom_bus_layout() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut builder = NodeBuilder::transformer();
        builder.set_inputs(&[1, 2]).set_outputs(&[4, 6]);

        let node = builder
            .build_req_frames(&node_graph, TestTransform { value: 60 })
            .unwrap();

        let inner = node_inner(&node);

        assert!(matches!(inner.op, NodeFunction::Resampler));
        assert_eq!(inner.busses.inputs, vec![1, 2]);
        assert_eq!(inner.busses.outputs, vec![4, 6]);

        assert_vtable(inner, 2, 2, true);
    }
}
