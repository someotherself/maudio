//! Node graph primitives.
//!
//! This module provides a view of **miniaudio nodes** (`ma_node`) and the
//! node-graph operations that can be performed on them.
//!
//! ## What is a node (in miniaudio)?
//!
//! In miniaudio, an audio **node** is a unit of processing/routing inside a **node graph**.
//! Nodes have *input buses* and *output buses* (each bus is a multi-channel audio stream).
//! Nodes can be connected together so that audio flows from upstream nodes into downstream
//! nodes, potentially being mixed, filtered, delayed, split, etc.
//!
//! Many high-level engine objects are also nodes. For example, a `Sound` can be treated as a
//! node for routing purposes: it can be connected to effect nodes, mixers, splitters, and
//! ultimately to the graph endpoint.
//!
//! ## What is a node graph?
//!
//! A **node graph** is the routing/processing graph that miniaudio evaluates to produce
//! audio output. Conceptually:
//!
//! - connections go from an output bus of one node to an input bus of another node,
//! - multiple outputs can feed the same input (mixing),
//! - the graph is evaluated as the engine/device pulls audio from the endpoint.
//!
//! You usually work with a graph indirectly through [`Engine`](crate::engine) and [`NodeGraph`].
//! How nodes work in miniaudio’s node graph (conceptually)
//!
//! ## Why `ma_node` looks like a `void*` (and why there is no `Node::new`)
//!
//! The type `ma_node` in miniaudio is an *opaque* / *transparent* “base pointer”:
//!
//! - In the C headers it is effectively `typedef void ma_node;`.
//! - A `ma_node*` does **not** point to a standalone `ma_node` allocation.
//! - Instead, it points at a **concrete node type** (e.g. `ma_delay_node`, `ma_splitter_node`,
//!   `ma_sound`, `ma_node_graph`, …) whose internal layout begins with a common base.
//!
//! This is how miniaudio implements a form of polymorphism in C: the public API accepts
//! `ma_node*`, while the actual object behind it is one of many concrete node structs with
//! node-specific behavior.
//!
//! Miniaudio does allow creating custom nodes however, this is an advanced feature that is
//! current not implemented in this crate.
//!
//! Because `ma_node` has no size and is not a constructible C struct, this crate does **not**
//! expose `Node::new()`. You cannot allocate a generic node. You either:
//!
//! 1. **Borrow a node view** from an existing object (recommended).
//!    For example: `sound.as_node()` returns a [`NodeRef`].
//! 2. **Own a concrete node type** (e.g. `DelayNode`, `SplitterNode`, etc.) and treat it as a node
//!    when connecting/routing.
//! 3. **Create a custom node** (advanced, not implemented): allocate a concrete node struct compatible with miniaudio
//!    and provide callbacks/vtables. This crate currently keeps generic node creation internal.
//!
//! ## How to use nodes
//!
//! Most node-graph operations are provided via [`NodeOps`]. Any type that can yield an underlying
//! `ma_node*` implements the internal [`AsNodePtr`] adapter and therefore gets the shared methods.
//!
//! ### Example: treating a sound as a node
//!
//! ```no_run
//! use maudio::engine::{Engine, node_graph::nodes::NodeOps};
//!
//! let engine = Engine::new().unwrap();
//! let sound  = engine.new_sound().unwrap();
//!
//! // Borrow a node view of the sound.
//! let node = sound.as_node();
//!
//! // Use node-level methods.
//! let state = node.state().unwrap();
//! println!("node state: {:?}", state);
//! ```
use std::{cell::Cell, marker::PhantomData};

use maudio_sys::ffi as sys;

use crate::{
    engine::{
        node_graph::{node_builder::NodeState, NodeGraph, NodeGraphRef},
        AllocationCallbacks,
    },
    Binding, MaResult,
};

pub mod effects;
pub mod filters;
pub mod routing;
pub mod source;

// Would be used for fully custom nodes. Not used for now
struct Node<'a> {
    inner: *mut sys::ma_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
    _not_sync: PhantomData<Cell<()>>,
}

impl Binding for Node<'_> {
    type Raw = *mut sys::ma_node;

    /// !! unimplemented !!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

/// A borrowed view of a `Node` of any kind
#[derive(Clone, Copy)]
pub struct NodeRef<'a> {
    ptr: *mut sys::ma_node,
    _marker: PhantomData<&'a ()>,
    _not_sync: PhantomData<Cell<()>>,
}

impl Binding for NodeRef<'_> {
    type Raw = *mut sys::ma_node;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            ptr: raw,
            _marker: PhantomData,
            _not_sync: PhantomData,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.ptr
    }
}

/// Allows the AsNodePtr trait to stay public and the node_ptr mothod to stay private
///
/// node_ptr allows any custom node to be passed around as a Node and access the methods on NodeOps implicitly
pub(crate) mod private_node {
    use crate::{
        data_source::AsSourcePtr,
        engine::node_graph::nodes::{
            effects::delay::DelayNode,
            filters::{
                biquad::BiquadNode, hishelf::HiShelfNode, hpf::HpfNode, loshelf::LoShelfNode,
                lpf::LpfNode, notch::NotchNode, peak::PeakNode,
            },
            routing::splitter::SplitterNode,
            source::source_node::{AttachedSourceNode, SourceNode},
        },
    };

    use super::*;
    use maudio_sys::ffi as sys;

    pub trait NodePtrProvider<T: ?Sized> {
        fn as_node_ptr(t: &T) -> *mut sys::ma_node;
    }

    pub struct NodeProvider;
    pub struct NodeRefProvider;
    pub struct DelayNodeProvider;
    pub struct BiquadNodeProvider;
    pub struct HiShelfNodeProvider;
    pub struct HpfNodeProvider;
    pub struct LoShelfNodeProvider;
    pub struct LpfNodeProvider;
    pub struct NotchNodeProvider;
    pub struct PeakNodeProvider;
    pub struct SplitterNodeProvider;
    pub struct SourceNodeProvider;
    pub struct AttachedSourceNodeProvider;

    impl<'a> NodePtrProvider<Node<'a>> for NodeProvider {
        #[inline]
        fn as_node_ptr(t: &Node<'a>) -> *mut sys::ma_node {
            t.to_raw()
        }
    }

    impl<'a> NodePtrProvider<NodeRef<'a>> for NodeRefProvider {
        #[inline]
        fn as_node_ptr(t: &NodeRef<'a>) -> *mut sys::ma_node {
            t.to_raw()
        }
    }

    impl<'a> NodePtrProvider<DelayNode<'a>> for DelayNodeProvider {
        #[inline]
        fn as_node_ptr(t: &DelayNode<'a>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    impl<'a> NodePtrProvider<BiquadNode<'a>> for BiquadNodeProvider {
        #[inline]
        fn as_node_ptr(t: &BiquadNode<'a>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    impl<'a> NodePtrProvider<HiShelfNode<'a>> for HiShelfNodeProvider {
        #[inline]
        fn as_node_ptr(t: &HiShelfNode<'a>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    impl<'a> NodePtrProvider<HpfNode<'a>> for HpfNodeProvider {
        #[inline]
        fn as_node_ptr(t: &HpfNode<'a>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    impl<'a> NodePtrProvider<LoShelfNode<'a>> for LoShelfNodeProvider {
        #[inline]
        fn as_node_ptr(t: &LoShelfNode<'a>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    impl<'a> NodePtrProvider<LpfNode<'a>> for LpfNodeProvider {
        #[inline]
        fn as_node_ptr(t: &LpfNode<'a>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    impl<'a> NodePtrProvider<NotchNode<'a>> for NotchNodeProvider {
        #[inline]
        fn as_node_ptr(t: &NotchNode<'a>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    impl<'a> NodePtrProvider<PeakNode<'a>> for PeakNodeProvider {
        #[inline]
        fn as_node_ptr(t: &PeakNode<'a>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    impl<'a> NodePtrProvider<SplitterNode<'a>> for SplitterNodeProvider {
        #[inline]
        fn as_node_ptr(t: &SplitterNode<'a>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    impl<'a> NodePtrProvider<SourceNode<'a>> for SourceNodeProvider {
        #[inline]
        fn as_node_ptr(t: &SourceNode<'a>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    impl<'a, S: AsSourcePtr> NodePtrProvider<AttachedSourceNode<'a, S>> for AttachedSourceNodeProvider {
        #[inline]
        fn as_node_ptr(t: &AttachedSourceNode<'a, S>) -> *mut sys::ma_node {
            t.as_node().to_raw()
        }
    }

    pub fn node_ptr<T: AsNodePtr + ?Sized>(t: &T) -> *mut sys::ma_node {
        <T as AsNodePtr>::__PtrProvider::as_node_ptr(t)
    }
}

#[doc(hidden)]
pub trait AsNodePtr {
    type __PtrProvider: private_node::NodePtrProvider<Self>;
}

#[doc(hidden)]
impl AsNodePtr for Node<'_> {
    type __PtrProvider = private_node::NodeProvider;
}

#[doc(hidden)]
impl AsNodePtr for NodeRef<'_> {
    type __PtrProvider = private_node::NodeRefProvider;
}

impl<T: AsNodePtr + ?Sized> NodeOps for T {}

/// NodeOps trait contains shared methods for `Node` and [`NodeRef`]
pub trait NodeOps: AsNodePtr {
    fn attach_output_bus<P: AsNodePtr + ?Sized>(
        &mut self,
        output_bus: u32,
        other_node: &mut P,
        other_node_input_bus: u32,
    ) -> MaResult<()> {
        node_ffi::ma_node_attach_output_bus(self, output_bus, other_node, other_node_input_bus)
    }

    fn detach_output_bus(&mut self, output_bus: u32) -> MaResult<()> {
        node_ffi::ma_node_detach_output_bus(self, output_bus)
    }

    fn detach_all_outputs(&mut self) -> MaResult<()> {
        node_ffi::ma_node_detach_all_output_buses(self)
    }

    fn node_graph(&self) -> Option<NodeGraphRef<'_>> {
        node_ffi::ma_node_get_node_graph(self)
    }

    fn in_bus_count(&self) -> u32 {
        node_ffi::ma_node_get_input_bus_count(self)
    }

    fn out_bus_count(&self) -> u32 {
        node_ffi::ma_node_get_output_bus_count(self)
    }

    fn input_channels(&self, in_bus_index: u32) -> u32 {
        node_ffi::ma_node_get_input_channels(self, in_bus_index)
    }

    fn output_channels(&self, out_bus_index: u32) -> u32 {
        node_ffi::ma_node_get_output_channels(self, out_bus_index)
    }

    fn output_bus_volume(&mut self, out_bux_index: u32) -> f32 {
        node_ffi::ma_node_get_output_bus_volume(self, out_bux_index)
    }

    fn set_output_bus_volume(&mut self, out_bux_index: u32, volume: f32) -> MaResult<()> {
        node_ffi::ma_node_set_output_bus_volume(self, out_bux_index, volume)
    }

    fn state(&self) -> MaResult<NodeState> {
        node_ffi::ma_node_get_state(self)
    }

    fn set_state(&mut self, state: NodeState) -> MaResult<()> {
        node_ffi::ma_node_set_state(self, state)
    }

    fn state_time(&self, state: NodeState) -> u64 {
        node_ffi::ma_node_get_state_time(self, state)
    }

    fn set_state_time(&mut self, state: NodeState, global_time: u64) -> MaResult<()> {
        node_ffi::ma_node_set_state_time(self, state, global_time)
    }

    fn state_by_time(&self, global_time: u64) -> MaResult<NodeState> {
        node_ffi::ma_node_get_state_by_time(self, global_time)
    }

    fn state_by_time_range(
        &self,
        global_time_beg: u64,
        global_time_end: u64,
    ) -> MaResult<NodeState> {
        node_ffi::ma_node_get_state_by_time_range(self, global_time_beg, global_time_end)
    }

    fn time(&self) -> u64 {
        node_ffi::ma_node_get_time(self)
    }

    fn set_time(&mut self, local_time: u64) -> MaResult<()> {
        node_ffi::ma_node_set_time(self, local_time)
    }
}

// These should be not available to NodeRef
impl<'a> Node<'a> {
    pub(crate) fn new(inner: *mut sys::ma_node, alloc_cb: Option<&'a AllocationCallbacks>) -> Self {
        Self {
            inner,
            alloc_cb,
            _marker: PhantomData,
            _not_sync: PhantomData,
        }
    }

    #[inline]
    fn alloc_cb_ptr(&self) -> *const sys::ma_allocation_callbacks {
        match &self.alloc_cb {
            Some(cb) => &cb.inner as *const _,
            None => core::ptr::null(),
        }
    }
}

pub(super) mod node_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        engine::node_graph::{
            node_builder::NodeState,
            nodes::{private_node, AsNodePtr, Node},
            NodeGraph, NodeGraphRef,
        },
        Binding, MaRawResult, MaResult,
    };

    // Do not expose to public API. Used internally by ma_node_init
    #[inline]
    pub(crate) fn ma_node_get_heap_size(
        node_graph: &mut NodeGraph,
        config: *const sys::ma_node_config,
    ) -> usize {
        let mut heap_size: usize = 0;
        unsafe { sys::ma_node_get_heap_size(node_graph.to_raw(), config, &mut heap_size) };
        heap_size
    }

    // Do not expose to public API. Used internally by ma_node_init
    #[inline]
    pub(crate) fn ma_node_init_preallocated(
        node_graph: &mut NodeGraph,
        config: *const sys::ma_node_config,
        heap: *mut core::ffi::c_void,
        node: *mut sys::ma_node,
    ) -> sys::ma_result {
        unsafe { sys::ma_node_init_preallocated(node_graph.to_raw(), config, heap, node) }
    }

    // Not exposed to public API yet. Used for creating custom nodes only.
    #[inline]
    pub(crate) fn ma_node_init(
        node_graph: &NodeGraph,
        config: *const sys::ma_node_config,
        allocation_callbacks: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_node,
    ) -> MaResult<()> {
        let res =
            unsafe { sys::ma_node_init(node_graph.to_raw(), config, allocation_callbacks, node) };
        MaRawResult::check(res)
    }

    // Creating nodes is currently not supported. Any nodes that used are not owned and should not be dropped.
    #[inline]
    fn ma_node_uninit(node: &mut Node, allocation_callbacks: *const sys::ma_allocation_callbacks) {
        unsafe { sys::ma_node_uninit(node.to_raw(), allocation_callbacks) }
    }

    #[inline]
    pub(crate) fn ma_node_get_node_graph<'a, P: AsNodePtr + ?Sized>(
        node: &'a P,
    ) -> Option<NodeGraphRef<'a>> {
        let ptr = unsafe { sys::ma_node_get_node_graph(private_node::node_ptr(node) as *const _) };
        if ptr.is_null() {
            None
        } else {
            Some(NodeGraphRef::from_ptr(ptr))
        }
    }

    #[inline]
    pub(crate) fn ma_node_get_input_bus_count<P: AsNodePtr + ?Sized>(node: &P) -> u32 {
        unsafe { sys::ma_node_get_input_bus_count(private_node::node_ptr(node) as *const _) }
    }

    #[inline]
    pub(crate) fn ma_node_get_output_bus_count<P: AsNodePtr + ?Sized>(node: &P) -> u32 {
        unsafe { sys::ma_node_get_output_bus_count(private_node::node_ptr(node) as *const _) }
    }

    #[inline]
    pub(crate) fn ma_node_get_input_channels<P: AsNodePtr + ?Sized>(
        node: &P,
        input_bus_index: u32,
    ) -> u32 {
        unsafe {
            sys::ma_node_get_input_channels(
                private_node::node_ptr(node) as *const _,
                input_bus_index,
            )
        }
    }

    #[inline]
    pub(crate) fn ma_node_get_output_channels<P: AsNodePtr + ?Sized>(
        node: &P,
        output_bus_index: u32,
    ) -> u32 {
        unsafe {
            sys::ma_node_get_output_channels(
                private_node::node_ptr(node) as *const _,
                output_bus_index,
            )
        }
    }

    #[inline]
    pub(crate) fn ma_node_attach_output_bus<P: AsNodePtr + ?Sized, Q: AsNodePtr + ?Sized>(
        node: &mut P,
        output_bus_index: u32,
        other_node: &mut Q,
        other_node_input_bus_index: u32,
    ) -> MaResult<()> {
        unsafe {
            let res = sys::ma_node_attach_output_bus(
                private_node::node_ptr(node),
                output_bus_index,
                private_node::node_ptr(other_node),
                other_node_input_bus_index,
            );
            MaRawResult::check(res)
        }
    }

    #[inline]
    pub(crate) fn ma_node_detach_output_bus<P: AsNodePtr + ?Sized>(
        node: &mut P,
        output_bus_index: u32,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_node_detach_output_bus(private_node::node_ptr(node), output_bus_index)
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub(crate) fn ma_node_detach_all_output_buses<P: AsNodePtr + ?Sized>(
        node: &mut P,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_node_detach_all_output_buses(private_node::node_ptr(node)) };
        MaRawResult::check(res)
    }

    #[inline]
    pub(crate) fn ma_node_set_output_bus_volume<P: AsNodePtr + ?Sized>(
        node: &mut P,
        output_bus_index: sys::ma_uint32,
        volume: f32,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_node_set_output_bus_volume(
                private_node::node_ptr(node),
                output_bus_index,
                volume,
            )
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub(crate) fn ma_node_get_output_bus_volume<P: AsNodePtr + ?Sized>(
        node: &mut P,
        output_bus_index: sys::ma_uint32,
    ) -> f32 {
        unsafe {
            sys::ma_node_get_output_bus_volume(private_node::node_ptr(node), output_bus_index)
        }
    }

    #[inline]
    pub(crate) fn ma_node_set_state<P: AsNodePtr + ?Sized>(
        node: &mut P,
        state: NodeState,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_node_set_state(private_node::node_ptr(node), state.into()) };
        MaRawResult::check(res)
    }

    #[inline]
    pub(crate) fn ma_node_get_state<P: AsNodePtr + ?Sized>(node: &P) -> MaResult<NodeState> {
        let res = unsafe { sys::ma_node_get_state(private_node::node_ptr(node) as *const _) };
        res.try_into()
    }

    #[inline]
    pub(crate) fn ma_node_set_state_time<P: AsNodePtr + ?Sized>(
        node: &mut P,
        state: NodeState,
        global_time: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_node_set_state_time(private_node::node_ptr(node), state.into(), global_time)
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub(crate) fn ma_node_get_state_time<P: AsNodePtr + ?Sized>(node: &P, state: NodeState) -> u64 {
        unsafe {
            sys::ma_node_get_state_time(private_node::node_ptr(node) as *const _, state.into())
        }
    }

    #[inline]
    pub(crate) fn ma_node_get_state_by_time<P: AsNodePtr + ?Sized>(
        node: &P,
        global_time: u64,
    ) -> MaResult<NodeState> {
        let res = unsafe {
            sys::ma_node_get_state_by_time(private_node::node_ptr(node) as *const _, global_time)
        };
        res.try_into()
    }

    #[inline]
    pub(crate) fn ma_node_get_state_by_time_range<P: AsNodePtr + ?Sized>(
        node: &P,
        global_time_beg: u64,
        global_time_end: u64,
    ) -> MaResult<NodeState> {
        unsafe {
            let res = sys::ma_node_get_state_by_time_range(
                private_node::node_ptr(node) as *const _,
                global_time_beg,
                global_time_end,
            );
            res.try_into()
        }
    }

    #[inline]
    pub(crate) fn ma_node_get_time<P: AsNodePtr + ?Sized>(node: &P) -> u64 {
        unsafe { sys::ma_node_get_time(private_node::node_ptr(node) as *const _) }
    }

    #[inline]
    pub(crate) fn ma_node_set_time<P: AsNodePtr + ?Sized>(
        node: &mut P,
        local_time: u64,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_node_set_time(private_node::node_ptr(node), local_time) };
        MaRawResult::check(res)
    }
}

// Creating nodes is currently not supported. Any nodes that used are not owned and should not be dropped.
// impl<'a> Drop for Node<'a> {
//     fn drop(&mut self) {
//         node_ffi::ma_node_uninit(self, self.alloc_cb_ptr());
//         drop(unsafe { Box::<sys::ma_node>::from_raw(self.to_raw()) });
//     }
// }
