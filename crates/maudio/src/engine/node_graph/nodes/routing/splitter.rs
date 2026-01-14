use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    Binding, Result, engine::{
        AllocationCallbacks,
        node_graph::{AsNodeGraphPtr, NodeGraph, nodes::NodeRef},
    }
};

/// A node that **duplicates an input signal to multiple outputs** inside a node graph.
///
/// A splitter takes a single input bus and routes the same audio signal to
/// **multiple output buses**, allowing the signal to be processed by multiple
/// downstream nodes in parallel.
///
/// This is commonly used to:
/// - feed the same signal into multiple effects (e.g. dry + reverb + delay)
/// - build parallel processing chains
/// - create complex routing graphs without re-rendering audio
///
/// `SplitterNode` is a node-graph wrapper around miniaudio’s splitter node
/// implementation. It performs **no DSP processing** itself—it only handles
/// routing—and is therefore extremely lightweight.
///
/// ## Behavior
/// - The input signal is copied verbatim to each output bus
/// - All output buses are sample-accurate and phase-aligned
/// - The node has **one input bus** and **N output buses**
///
/// ## Configuration
/// - **channels**: Number of audio channels per bus (e.g. 1 = mono, 2 = stereo)
/// - **output_bus_count**: Number of output buses the signal is split into
///
/// The output bus count is fixed at initialization time and cannot be changed
/// after the node is created.
///
/// ## Routing & Control
/// Per-output behavior (such as volume or connection targets) is controlled
/// via the underlying node APIs exposed through [`NodeRef`], for example:
/// - setting per-output bus volume
/// - attaching output buses to downstream nodes
/// - starting or stopping the node
///
/// ## Notes
/// - `SplitterNode` does not allocate internal buffers beyond what is required
///   by the node graph
/// - It introduces no latency and performs no filtering or mixing
/// - Volume control and routing are handled at the node-graph level
///
/// Use [`SplitterNodeBuilder`] to initialize.
pub struct SplitterNode<'a> {
    inner: *mut sys::ma_splitter_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
}

impl Binding for SplitterNode<'_> {
    type Raw = *mut sys::ma_splitter_node;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a> SplitterNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &SplitterNodeBuilder<'_, N>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> Result<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_splitter_node>> = Box::new_uninit();

        n_splitter_ffi::ma_splitter_node_init(node_graph, config.to_raw(), alloc_cb, mem.as_mut_ptr())?;

        let ptr: Box<sys::ma_splitter_node> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_splitter_node = Box::into_raw(ptr);

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _marker: PhantomData,
        })
    }

    /// Returns a **borrowed view** as a node in the engine's node graph.
    ///
    /// ### What this is for
    ///
    /// Use `as_node()` when you want to:
    /// - connect this to other nodes (effects, mixers, splitters, etc.)
    /// - insert into a custom routing graph
    /// - query node-level state exposed by the graph
    pub fn as_node(&self) -> NodeRef<'a> {
        debug_assert!(!self.inner.is_null());
        let ptr = self.inner.cast::<sys::ma_node>();
        NodeRef::from_ptr(ptr)
    }

    #[inline]
    fn alloc_cb_ptr(&self) -> *const sys::ma_allocation_callbacks {
        match &self.alloc_cb {
            Some(cb) => &cb.inner as *const _,
            None => core::ptr::null(),
        }
    }
}

pub(crate) mod n_splitter_ffi {
    use crate::{Binding, MaRawResult, Result, engine::node_graph::{AsNodeGraphPtr, nodes::routing::splitter::SplitterNode}};
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_splitter_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_splitter_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_splitter_node,
    ) -> Result<()> {
        let res = unsafe {
            sys::ma_splitter_node_init(node_graph.as_nodegraph_ptr(), config, alloc_cb, node)
        };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_splitter_node_uninit(node: &mut SplitterNode) {
        unsafe {
            sys::ma_splitter_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }
}

impl<'a> Drop for SplitterNode<'a> {
    fn drop(&mut self) {
        n_splitter_ffi::ma_splitter_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub struct SplitterNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_splitter_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> Binding for SplitterNodeBuilder<'_, N> {
    type Raw = *const sys::ma_splitter_node_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

impl<'a, N: AsNodeGraphPtr + ?Sized> SplitterNodeBuilder<'a, N> {
    pub fn new(node_graph: &'a N, channels: u32) -> Self {
        let ptr = unsafe {
            sys::ma_splitter_node_config_init(channels)
        };
        Self { inner: ptr, node_graph }
    }

    pub fn output_bus_count(mut self, count: u32) -> Self {
        self.inner.outputBusCount = count;
        self
    }

    pub fn build(self) -> Result<SplitterNode<'a>> {
        SplitterNode::new_with_cfg_alloc_internal(self.node_graph, &self, None)
    }
}

#[cfg(test)]
mod test {
    use crate::engine::{Engine, EngineOps, node_graph::{node_builder::NodeState, nodes::{NodeOps, routing::splitter::SplitterNodeBuilder}}};

    #[test]
    fn test_splitter_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let _node = SplitterNodeBuilder::new(&node_graph, 2).output_bus_count(2).build().unwrap();
    }

    #[test]
    fn test_splitter_default_output_bus_count_builds() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let _node = SplitterNodeBuilder::new(&node_graph, 2).build().unwrap();
    }

    #[test]
    fn test_splitter_various_output_bus_counts() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        for &bus_count in &[1u32, 2, 4, 8] {
            let _node = SplitterNodeBuilder::new(&node_graph, 2)
                .output_bus_count(bus_count)
                .build()
                .unwrap();
        }
    }

    #[test]
    fn test_splitter_repeated_create_drop() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        for _ in 0..100 {
            let _node = SplitterNodeBuilder::new(&node_graph, 2)
                .output_bus_count(2)
                .build()
                .unwrap();
            // drop happens here each loop iteration
        }
    }

    #[test]
    fn test_splitter_zero_channels_is_err() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let res = SplitterNodeBuilder::new(&node_graph, 0)
            .output_bus_count(2)
            .build();

        assert!(res.is_err());
    }

    #[test]
    fn test_splitter_zero_output_buses_is_err() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let res = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(0)
            .build();

        // TODO: zero output busses is ok?
        assert!(res.is_ok());
    }

    #[test]
    fn test_splitter_as_node_is_valid() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let node = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(2)
            .build()
            .unwrap();

        let _as_node = node.as_node();
    }

    #[test]
    fn test_splitter_node_ref_bus_counts_and_channels() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let splitter = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(4)
            .build()
            .unwrap();

        let node_ref = splitter.as_node();

        // Splitter should have 1 input bus and N output buses.
        assert_eq!(node_ref.in_bus_count(), 1);
        assert_eq!(node_ref.out_bus_count(), 4);

        // Each bus should be 2 channels (stereo).
        assert_eq!(node_ref.input_channels(0), 2);

        for out_bus in 0..4 {
            assert_eq!(node_ref.output_channels(out_bus), 2);
        }
    }


    #[test]
    fn test_splitter_attach_and_detach_output_bus() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let splitter_a = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(2)
            .build()
            .unwrap();

        let splitter_b = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(2)
            .build()
            .unwrap();

        // NodeRef is a borrowed view, so make them mutable locals.
        let mut a = splitter_a.as_node();
        let mut b = splitter_b.as_node();

        // Attach A.out[0] -> B.in[0]
        a.attach_output_bus(0, &mut b, 0).unwrap();

        // Detach just that bus.
        a.detach_output_bus(0).unwrap();

        // And detach-all should be safe even if nothing is attached.
        a.detach_all_outputs().unwrap();
    }

    #[test]
    fn test_splitter_detach_all_outputs_after_multiple_attaches() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let splitter_a = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(4)
            .build()
            .unwrap();

        let splitter_b = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(2)
            .build()
            .unwrap();

        let mut a = splitter_a.as_node();
        let mut b = splitter_b.as_node();

        // Attach multiple outputs of A to the same input of B (legal for routing graphs;
        // the graph may mix them or the node may receive multiple connections depending
        // on miniaudio internals, but attach should succeed).
        for out_bus in 0..4 {
            a.attach_output_bus(out_bus, &mut b, 0).unwrap();
        }

        a.detach_all_outputs().unwrap();
    }

    #[test]
    fn test_splitter_output_bus_volume_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let splitter = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(3)
            .build()
            .unwrap();

        let mut node_ref = splitter.as_node();

        let vols = [0.0_f32, 0.5_f32, 1.0_f32];

        for (bus, &v) in vols.iter().enumerate() {
            node_ref.set_output_bus_volume(bus as u32, v).unwrap();
            let got = node_ref.output_bus_volume(bus as u32);
            assert!((got - v).abs() < 1.0e-6);
        }
    }

    #[test]
    fn test_splitter_state_set_get() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let splitter = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(2)
            .build()
            .unwrap();

        let mut node_ref = splitter.as_node();

        node_ref.set_state(NodeState::Stopped).unwrap();
        assert_eq!(node_ref.state().unwrap(), NodeState::Stopped);

        node_ref.set_state(NodeState::Started).unwrap();
        assert_eq!(node_ref.state().unwrap(), NodeState::Started);
    }

    #[test]
    fn test_splitter_node_graph_ref_is_some() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let splitter = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(2)
            .build()
            .unwrap();

        let node_ref = splitter.as_node();
        assert!(node_ref.node_graph().is_some());
    }

    #[test]
    fn test_splitter_invalid_attach_indices_is_err() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let splitter_a = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(2)
            .build()
            .unwrap();

        let splitter_b = SplitterNodeBuilder::new(&node_graph, 2)
            .output_bus_count(2)
            .build()
            .unwrap();

        let mut a = splitter_a.as_node();
        let mut b = splitter_b.as_node();

        // output_bus index 999 should be out of range => Err
        assert!(a.attach_output_bus(999, &mut b, 0).is_err());

        // input bus index 999 should be out of range => Err (splitter has 1 input bus)
        assert!(a.attach_output_bus(0, &mut b, 999).is_err());
    }
}