/// Builder for a [`NodeGraph`].
///
/// A node graph is a lightweight DSP graph that processes interleaved
/// `f32` PCM frames. Its configuration is intentionally minimal: the only
/// required parameter is the number of output channels.
///
/// The graph itself is sample-rate agnostic and does not own an audio device.
/// Timing, sample rate, and format conversion are defined by the caller
/// (for example, an engine or audio device).
///
/// Most behavior is defined by the nodes attached to the graph rather than
/// by the graph configuration itself.
use maudio_sys::ffi as sys;

use crate::{engine::node_graph::NodeGraph, Binding, MaResult};

/// Configures and constructs a [`NodeGraph`].
///
/// This builder wraps `ma_node_graph_config` and exposes the small set of
/// options required to create a node graph.
pub struct NodeGraphBuilder {
    inner: sys::ma_node_graph_config,
}

impl Binding for NodeGraphBuilder {
    type Raw = sys::ma_node_graph_config;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self { inner: raw }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl NodeGraphBuilder {
    /// Creates a new node graph builder.
    ///
    /// - `channels`: The number of output channels for the graph.
    ///
    /// The channel count defines the width of the graph and must match the
    /// expected channel layout of the graph's endpoint.
    pub fn new(channels: u32) -> Self {
        let ptr = unsafe { sys::ma_node_graph_config_init(channels) };
        NodeGraphBuilder::from_ptr(ptr)
    }

    /// Builds the [`NodeGraph`] using the current configuration.
    ///
    /// The resulting graph is not attached to any audio device and does not
    /// begin processing until it is driven by a caller (such as an engine
    /// or manual frame reads).
    pub fn build<'a>(self) -> MaResult<NodeGraph<'a>> {
        NodeGraph::with_alloc_callbacks(&self, None)
    }
}
