use std::{cell::Cell, marker::PhantomData};

use maudio_sys::ffi as sys;

use crate::{
    Binding, Result,
    engine::{
        AllocationCallbacks,
        node_graph::{NodeGraph, NodeGraphRef, node_builder::NodeState},
    },
};

/// Prelude for the [`node`](super) module.
///
/// This module re-exports the most commonly used engine types and traits
/// so they can be imported with a single global import.
///
/// Import this when you want access to [`Node`] and [`NodeRef`] and all shared engine
/// methods (provided by [`NodeOps`]) without having to import each item
/// individually.
/// This is purely a convenience module; importing from `engine` directly
/// works just as well if you prefer explicit imports.
pub mod prelude {
    pub use super::{Node, NodeOps};
}

pub struct Node<'a> {
    inner: *mut sys::ma_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
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

pub struct NodeRef<'a> {
    // TODO: Use *mut sys::ma_node_base instead, and cast it to ma_node
    ptr: *mut sys::ma_node,
    _marker: PhantomData<&'a mut ()>,
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

pub trait AsNodePtr {
    fn as_engine_ptr(&self) -> *mut sys::ma_node;
}

impl AsNodePtr for Node<'_> {
    fn as_engine_ptr(&self) -> *mut sys::ma_node {
        self.to_raw()
    }
}

impl AsNodePtr for NodeRef<'_> {
    fn as_engine_ptr(&self) -> *mut sys::ma_node {
        self.to_raw()
    }
}

impl<T: AsNodePtr + ?Sized> NodeOps for T {}

pub trait NodeOps: AsNodePtr {
    fn attach_output_bus<P: AsNodePtr + ?Sized>(
        &mut self,
        output_bus: u32,
        other_node: &mut P,
        other_node_input_bus: u32,
    ) -> Result<()> {
        node_ffi::ma_node_attach_output_bus(self, output_bus, other_node, other_node_input_bus)
    }

    fn detach_output_bus(&mut self, output_bus: u32) -> Result<()> {
        node_ffi::ma_node_detach_output_bus(self, output_bus)
    }

    fn detach_all_outputs(&mut self) -> Result<()> {
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

    fn set_output_bus_volume(&mut self, out_bux_index: u32, volume: f32) -> Result<()> {
        node_ffi::ma_node_set_output_bus_volume(self, out_bux_index, volume)
    }

    fn state(&self) -> Result<NodeState> {
        node_ffi::ma_node_get_state(self)
    }

    fn set_state(&mut self, state: NodeState) -> Result<()> {
        node_ffi::ma_node_set_state(self, state)
    }

    fn state_time(&self, state: NodeState) -> u64 {
        node_ffi::ma_node_get_state_time(self, state)
    }

    fn set_state_time(&mut self, state: NodeState, global_time: u64) -> Result<()> {
        node_ffi::ma_node_set_state_time(self, state, global_time)
    }

    fn state_by_time(&self, global_time: u64) -> Result<NodeState> {
        node_ffi::ma_node_get_state_by_time(self, global_time)
    }

    fn state_by_time_range(&self, global_time_beg: u64, global_time_end: u64) -> Result<NodeState> {
        node_ffi::ma_node_get_state_by_time_range(self, global_time_beg, global_time_end)
    }

    fn time(&self) -> u64 {
        node_ffi::ma_node_get_time(self)
    }

    fn set_time(&mut self, local_time: u64) -> Result<()> {
        node_ffi::ma_node_set_time(self, local_time)
    }
}

impl<'a> Node<'a> {
    pub(crate) fn new(inner: *mut sys::ma_node, alloc_cb: Option<&'a AllocationCallbacks>) -> Self {
        Self {
            inner,
            alloc_cb,
            _marker: PhantomData,
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
        Binding, MaRawResult, Result,
        engine::node_graph::{
            NodeGraph, NodeGraphRef,
            node_builder::NodeState,
            nodes::{AsNodePtr, Node},
        },
    };

    // Do not expose to public API
    #[inline]
    pub(crate) fn ma_node_get_heap_size(
        node_graph: &mut NodeGraph,
        config: *const sys::ma_node_config,
    ) -> usize {
        let mut heap_size: usize = 0;
        unsafe { sys::ma_node_get_heap_size(node_graph.to_raw(), config, &mut heap_size) };
        heap_size
    }

    // Do not expose to public API
    #[inline]
    pub(crate) fn ma_node_init_preallocated(
        node_graph: &mut NodeGraph,
        config: *const sys::ma_node_config,
        heap: *mut core::ffi::c_void,
        node: *mut sys::ma_node,
    ) -> sys::ma_result {
        unsafe { sys::ma_node_init_preallocated(node_graph.to_raw(), config, heap, node) }
    }

    // Expose on NodeGraph
    #[inline]
    pub(crate) fn ma_node_init(
        node_graph: &NodeGraph,
        config: *const sys::ma_node_config,
        allocation_callbacks: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_node,
    ) -> Result<()> {
        let res =
            unsafe { sys::ma_node_init(node_graph.to_raw(), config, allocation_callbacks, node) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub(crate) fn ma_node_uninit(
        node: &mut Node,
        allocation_callbacks: *const sys::ma_allocation_callbacks,
    ) {
        unsafe { sys::ma_node_uninit(node.to_raw(), allocation_callbacks) }
    }

    // ---- graph / bus info ----

    #[inline]
    pub(crate) fn ma_node_get_node_graph<'a, P: AsNodePtr + ?Sized>(
        node: &'a P,
    ) -> Option<NodeGraphRef<'a>> {
        let ptr = unsafe { sys::ma_node_get_node_graph(node.as_engine_ptr() as *const _) };
        if ptr.is_null() {
            None
        } else {
            Some(NodeGraphRef::from_ptr(ptr))
        }
    }

    #[inline]
    pub(crate) fn ma_node_get_input_bus_count<P: AsNodePtr + ?Sized>(node: &P) -> u32 {
        unsafe { sys::ma_node_get_input_bus_count(node.as_engine_ptr() as *const _) }
    }

    #[inline]
    pub(crate) fn ma_node_get_output_bus_count<P: AsNodePtr + ?Sized>(node: &P) -> u32 {
        unsafe { sys::ma_node_get_output_bus_count(node.as_engine_ptr() as *const _) }
    }

    #[inline]
    pub(crate) fn ma_node_get_input_channels<P: AsNodePtr + ?Sized>(
        node: &P,
        input_bus_index: u32,
    ) -> u32 {
        unsafe {
            sys::ma_node_get_input_channels(node.as_engine_ptr() as *const _, input_bus_index)
        }
    }

    #[inline]
    pub(crate) fn ma_node_get_output_channels<P: AsNodePtr + ?Sized>(
        node: &P,
        output_bus_index: u32,
    ) -> u32 {
        unsafe {
            sys::ma_node_get_output_channels(node.as_engine_ptr() as *const _, output_bus_index)
        }
    }

    // ---- connections ----

    #[inline]
    pub(crate) fn ma_node_attach_output_bus<P: AsNodePtr + ?Sized, Q: AsNodePtr + ?Sized>(
        node: &mut P,
        output_bus_index: u32,
        other_node: &mut Q,
        other_node_input_bus_index: u32,
    ) -> Result<()> {
        unsafe {
            let res = sys::ma_node_attach_output_bus(
                node.as_engine_ptr(),
                output_bus_index,
                other_node.as_engine_ptr(),
                other_node_input_bus_index,
            );
            MaRawResult::resolve(res)
        }
    }

    #[inline]
    pub(crate) fn ma_node_detach_output_bus<P: AsNodePtr + ?Sized>(
        node: &mut P,
        output_bus_index: u32,
    ) -> Result<()> {
        let res = unsafe { sys::ma_node_detach_output_bus(node.as_engine_ptr(), output_bus_index) };
        MaRawResult::resolve(res)
    }

    /// Shared???
    #[inline]
    pub(crate) fn ma_node_detach_all_output_buses<P: AsNodePtr + ?Sized>(
        node: &mut P,
    ) -> Result<()> {
        let res = unsafe { sys::ma_node_detach_all_output_buses(node.as_engine_ptr()) };
        MaRawResult::resolve(res)
    }

    // ---- per-output-bus volume ----

    #[inline]
    pub(crate) fn ma_node_set_output_bus_volume<P: AsNodePtr + ?Sized>(
        node: &mut P,
        output_bus_index: sys::ma_uint32,
        volume: f32,
    ) -> Result<()> {
        let res = unsafe {
            sys::ma_node_set_output_bus_volume(node.as_engine_ptr(), output_bus_index, volume)
        };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub(crate) fn ma_node_get_output_bus_volume<P: AsNodePtr + ?Sized>(
        node: &mut P,
        output_bus_index: sys::ma_uint32,
    ) -> f32 {
        unsafe { sys::ma_node_get_output_bus_volume(node.as_engine_ptr(), output_bus_index) }
    }

    // ---- state ----

    #[inline]
    pub(crate) fn ma_node_set_state<P: AsNodePtr + ?Sized>(
        node: &mut P,
        state: NodeState,
    ) -> Result<()> {
        let res = unsafe { sys::ma_node_set_state(node.as_engine_ptr(), state.into()) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub(crate) fn ma_node_get_state<P: AsNodePtr + ?Sized>(node: &P) -> Result<NodeState> {
        let res = unsafe { sys::ma_node_get_state(node.as_engine_ptr() as *const _) };
        res.try_into()
    }

    #[inline]
    pub(crate) fn ma_node_set_state_time<P: AsNodePtr + ?Sized>(
        node: &mut P,
        state: NodeState,
        global_time: u64,
    ) -> Result<()> {
        let res =
            unsafe { sys::ma_node_set_state_time(node.as_engine_ptr(), state.into(), global_time) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub(crate) fn ma_node_get_state_time<P: AsNodePtr + ?Sized>(node: &P, state: NodeState) -> u64 {
        unsafe { sys::ma_node_get_state_time(node.as_engine_ptr() as *const _, state.into()) }
    }

    #[inline]
    pub(crate) fn ma_node_get_state_by_time<P: AsNodePtr + ?Sized>(
        node: &P,
        global_time: u64,
    ) -> Result<NodeState> {
        let res = unsafe {
            sys::ma_node_get_state_by_time(node.as_engine_ptr() as *const _, global_time)
        };
        res.try_into()
    }

    #[inline]
    pub(crate) fn ma_node_get_state_by_time_range<P: AsNodePtr + ?Sized>(
        node: &P,
        global_time_beg: u64,
        global_time_end: u64,
    ) -> Result<NodeState> {
        unsafe {
            let res = sys::ma_node_get_state_by_time_range(
                node.as_engine_ptr() as *const _,
                global_time_beg,
                global_time_end,
            );
            res.try_into()
        }
    }

    // ---- time ----

    #[inline]
    pub(crate) fn ma_node_get_time<P: AsNodePtr + ?Sized>(node: &P) -> u64 {
        unsafe { sys::ma_node_get_time(node.as_engine_ptr() as *const _) }
    }

    #[inline]
    pub(crate) fn ma_node_set_time<P: AsNodePtr + ?Sized>(
        node: &mut P,
        local_time: u64,
    ) -> Result<()> {
        let res = unsafe { sys::ma_node_set_time(node.as_engine_ptr(), local_time) };
        MaRawResult::resolve(res)
    }
}

// impl<'a> Drop for Node<'a> {
//     fn drop(&mut self) {
//         node_ffi::ma_node_uninit(self, self.alloc_cb_ptr());
//         drop(unsafe { Box::<sys::ma_node>::from_raw(self.to_raw()) });
//     }
// }
