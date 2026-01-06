use std::{marker::PhantomData, ptr::NonNull};

use maudio_sys::ffi as sys;

pub struct Node {
    heap: Box<[u8]>,
    ptr: *mut sys::ma_node,
}

pub struct NodeRef<'a> {
    ptr: NonNull<sys::ma_node>,
    _marker: PhantomData<&'a mut ()>,
}

impl<'a> NodeRef<'a> {
    pub(crate) fn from_ptr(ptr: *mut sys::ma_node) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("returned null node_graph"),
            _marker: PhantomData,
        }
    }
}

pub trait NodePtr {
    fn as_node_ptr(&self) -> *const sys::ma_node;
    fn as_node_mut_ptr(&mut self) -> *mut sys::ma_node;
}

impl NodePtr for Node {
    fn as_node_ptr(&self) -> *const sys::ma_node {
        self.ptr
    }

    fn as_node_mut_ptr(&mut self) -> *mut sys::ma_node {
        self.ptr
    }
}

impl<'g> NodePtr for NodeRef<'g> {
    fn as_node_ptr(&self) -> *const sys::ma_node {
        self.ptr.as_ptr()
    }

    fn as_node_mut_ptr(&mut self) -> *mut sys::ma_node {
        unsafe { self.ptr.as_mut() }
    }
}

mod node_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        MaRawResult, Result,
        engine::node_graph::{NodeGraph, nodes::NodePtr},
    };

    // ---- init / uninit ----

    #[inline]
    pub(crate) fn ma_node_get_heap_size(
        node_graph: &mut NodeGraph,
        config: *const sys::ma_node_config,
        heap_size: *mut usize,
    ) -> i32 {
        unsafe { sys::ma_node_get_heap_size(node_graph.inner_ptr_mut(), config, heap_size) }
    }

    #[inline]
    pub(crate) fn ma_node_init_preallocated(
        node_graph: &mut NodeGraph,
        config: *const sys::ma_node_config,
        heap: *mut core::ffi::c_void,
        node: *mut sys::ma_node,
    ) -> sys::ma_result {
        unsafe { sys::ma_node_init_preallocated(node_graph.inner_ptr_mut(), config, heap, node) }
    }

    #[inline]
    pub(crate) fn ma_node_init(
        node_graph: &mut NodeGraph,
        config: *const sys::ma_node_config,
        allocation_callbacks: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_node,
    ) -> sys::ma_result {
        unsafe {
            sys::ma_node_init(
                node_graph.inner_ptr_mut(),
                config,
                allocation_callbacks,
                node,
            )
        }
    }

    #[inline]
    pub(crate) fn ma_node_uninit<P: NodePtr>(
        node: &mut P,
        allocation_callbacks: *const sys::ma_allocation_callbacks,
    ) {
        unsafe { sys::ma_node_uninit(node.as_node_mut_ptr(), allocation_callbacks) }
    }

    // ---- graph / bus info ----

    #[inline]
    pub(crate) fn ma_node_get_node_graph<P: NodePtr>(node: P) -> *mut sys::ma_node_graph {
        unsafe { sys::ma_node_get_node_graph(node.as_node_ptr()) }
    }

    #[inline]
    pub(crate) fn ma_node_get_input_bus_count<P: NodePtr>(node: P) -> sys::ma_uint32 {
        unsafe { sys::ma_node_get_input_bus_count(node.as_node_ptr()) }
    }

    #[inline]
    pub(crate) fn ma_node_get_output_bus_count<P: NodePtr>(node: P) -> sys::ma_uint32 {
        unsafe { sys::ma_node_get_output_bus_count(node.as_node_ptr()) }
    }

    #[inline]
    pub(crate) fn ma_node_get_input_channels<P: NodePtr>(
        node: P,
        input_bus_index: sys::ma_uint32,
    ) -> sys::ma_uint32 {
        unsafe { sys::ma_node_get_input_channels(node.as_node_ptr(), input_bus_index) }
    }

    #[inline]
    pub(crate) fn ma_node_get_output_channels<P: NodePtr>(
        node: P,
        output_bus_index: sys::ma_uint32,
    ) -> sys::ma_uint32 {
        unsafe { sys::ma_node_get_output_channels(node.as_node_ptr(), output_bus_index) }
    }

    // ---- connections ----

    #[inline]
    pub(crate) fn ma_node_attach_output_bus<P: NodePtr>(
        node: &mut P,
        output_bus_index: sys::ma_uint32,
        other_node: &mut P,
        other_node_input_bus_index: sys::ma_uint32,
    ) -> sys::ma_result {
        unsafe {
            sys::ma_node_attach_output_bus(
                node.as_node_mut_ptr(),
                output_bus_index,
                other_node.as_node_mut_ptr(),
                other_node_input_bus_index,
            )
        }
    }

    #[inline]
    pub(crate) fn ma_node_detach_output_bus<P: NodePtr>(
        node: &mut P,
        output_bus_index: sys::ma_uint32,
    ) -> Result<()> {
        let res =
            unsafe { sys::ma_node_detach_output_bus(node.as_node_mut_ptr(), output_bus_index) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub(crate) fn ma_node_detach_all_output_buses<P: NodePtr>(node: &mut P) -> Result<()> {
        let res = unsafe { sys::ma_node_detach_all_output_buses(node.as_node_mut_ptr()) };
        MaRawResult::resolve(res)
    }

    // ---- per-output-bus volume ----

    #[inline]
    pub(crate) fn ma_node_set_output_bus_volume<P: NodePtr>(
        node: &mut P,
        output_bus_index: sys::ma_uint32,
        volume: f32,
    ) -> Result<()> {
        let res = unsafe {
            sys::ma_node_set_output_bus_volume(node.as_node_mut_ptr(), output_bus_index, volume)
        };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub(crate) fn ma_node_get_output_bus_volume<P: NodePtr>(
        node: &mut P,
        output_bus_index: sys::ma_uint32,
    ) -> f32 {
        unsafe { sys::ma_node_get_output_bus_volume(node.as_node_mut_ptr(), output_bus_index) }
    }

    // ---- state ----

    #[inline]
    pub(crate) fn ma_node_set_state<P: NodePtr>(
        node: &mut P,
        state: sys::ma_node_state, // TODO
    ) -> sys::ma_result {
        unsafe { sys::ma_node_set_state(node.as_node_mut_ptr(), state) }
    }

    #[inline]
    pub(crate) fn ma_node_get_state<P: NodePtr>(node: P) -> sys::ma_node_state {
        unsafe { sys::ma_node_get_state(node.as_node_ptr()) }
    }

    #[inline]
    pub(crate) fn ma_node_set_state_time<P: NodePtr>(
        node: &mut P,
        state: sys::ma_node_state, // TODO
        global_time: sys::ma_uint64,
    ) -> sys::ma_result {
        unsafe { sys::ma_node_set_state_time(node.as_node_mut_ptr(), state, global_time) }
    }

    #[inline]
    pub(crate) fn ma_node_get_state_time<P: NodePtr>(
        node: P,
        state: sys::ma_node_state, // TODO
    ) -> sys::ma_uint64 {
        unsafe { sys::ma_node_get_state_time(node.as_node_ptr(), state) }
    }

    #[inline]
    pub(crate) fn ma_node_get_state_by_time<P: NodePtr>(
        node: P,
        global_time: sys::ma_uint64,
    ) -> sys::ma_node_state {
        unsafe { sys::ma_node_get_state_by_time(node.as_node_ptr(), global_time) }
    }

    #[inline]
    pub(crate) fn ma_node_get_state_by_time_range<P: NodePtr>(
        node: P,
        global_time_beg: sys::ma_uint64,
        global_time_end: sys::ma_uint64,
    ) -> sys::ma_node_state {
        unsafe {
            sys::ma_node_get_state_by_time_range(
                node.as_node_ptr(),
                global_time_beg,
                global_time_end,
            )
        }
    }

    // ---- time ----

    #[inline]
    pub(crate) fn ma_node_get_time<P: NodePtr>(node: P) -> sys::ma_uint64 {
        unsafe { sys::ma_node_get_time(node.as_node_ptr()) }
    }

    #[inline]
    pub(crate) fn ma_node_set_time<P: NodePtr>(
        node: &mut P,
        local_time: sys::ma_uint64,
    ) -> sys::ma_result {
        unsafe { sys::ma_node_set_time(node.as_node_mut_ptr(), local_time) }
    }
}
