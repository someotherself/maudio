use std::{marker::PhantomData, mem::MaybeUninit, ptr::NonNull};

pub mod node_flags;
pub mod node_graph_builder;
pub mod nodes;

use maudio_sys::ffi as sys;

use crate::{
    MaError, MaRawResult, Result,
    engine::{
        AllocationCallbacks,
        node_graph::{node_graph_builder::NodeGraphConfig, nodes::NodeRef},
    },
};

/// A pull-based audio processing graph. Represents `ma_node_graph`
///
/// `NodeGraph` is the root of miniaudio’s node-based audio system. It owns an
/// internal *endpoint node* and produces audio by **pulling** data from all
/// nodes connected upstream.
///
/// Unlike a traditional “pipeline” where audio is pushed into a graph,
/// a `NodeGraph` has **no external input**. Instead, audio is generated
/// on demand when the graph is read (for example by an audio device or
/// by calling `ma_node_graph_read_pcm_frames`) TODO.
///
/// ## How audio flows
///
/// Each box in the diagram below represents a **node**. A `NodeGraph` is simply
/// a collection of interconnected nodes put together.
///
/// ```text
/// [Decoder / Generator Nodes] → [Mixers / Effects] → [Graph Endpoint] → output
/// ```
///
/// ## Allocation callbacks
/// See [`AllocationCallbacks`]
///
/// ## Relationship to `Engine`
///
/// A `NodeGraph` can be used directly for offline rendering or device-driven
/// playback. Higher-level constructs (such as an `Engine`) may internally
/// own and manage a node graph.
///
/// ## When you probably don’t need `NodeGraph`
///
/// You usually don’t need `NodeGraph` unless you are building an audio engine
/// or implementing custom audio processing.
/// If your goal is simply to play sounds, music, or sound effects,
/// a higher-level API (such as an `Engine` or `Sound`) is usually a better fit.
///
/// `NodeGraph` is primarily useful when you need:
/// - custom audio routing or mixing
/// - non-standard processing graphs
/// - offline rendering of audio
/// - fine-grained control over how audio is evaluated
pub struct NodeGraph<'a> {
    inner: sys::ma_node_graph,
    alloc_cb: Option<&'a AllocationCallbacks>,
}

/// ## Borrowed node graphs (`NodeGraphRef`)
///
/// Some node graphs are owned internally by higher-level objects such as an
/// [`Engine`]. These graphs must not be uninitialized or moved by the user.
///
/// `NodeGraphRef` represents a **borrowed view** into such an engine-owned node
/// graph. It does not own the underlying `ma_node_graph`, does not store
/// allocation callbacks, and its lifetime is tied to the owner.
///
/// This type exists to safely model miniaudio APIs that return pointers to
/// internally managed node graphs (for example `ma_engine_get_node_graph`).
pub struct NodeGraphRef<'e> {
    ptr: NonNull<sys::ma_node_graph>,
    _engine: PhantomData<&'e mut sys::ma_engine>,
}

impl<'e> NodeGraphRef<'e> {
    pub(crate) fn from_ptr(ptr: *mut sys::ma_node_graph) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("engine returned null node_graph"),
            _engine: PhantomData,
        }
    }
}

impl<'a> NodeGraph<'a> {
    pub fn new(config: &NodeGraphConfig) -> Result<Self> {
        NodeGraph::with_alloc_callbacks(config, None)
    }

    pub fn endpoint(&self) -> NodeRef<'_> {
        graph_ffi::ma_node_graph_get_endpoint(self)
    }

    pub fn read_pcm_frames_f32(
        &mut self,
        _frames_out: &mut [f32],
        _frame_count: u64,
        _channels_out: u32,
    ) {
        // let res = graph_ffi::ma_node_graph_read_pcm_frames(
        //     &mut self,
        //     frames_out,
        //     frame_count,
        //     frames_read,
        // );
    }

    pub fn channels(&self) -> u32 {
        graph_ffi::ma_node_graph_get_channels(self)
    }

    pub fn time(&self) -> u64 {
        graph_ffi::ma_node_graph_get_time(self)
    }

    pub fn set_time(&mut self, global_time: u64) -> Result<()> {
        let res = graph_ffi::ma_node_graph_set_time(self, global_time);
        MaRawResult::resolve(res)
    }
}

impl<'a> NodeGraph<'a> {
    fn with_alloc_callbacks(
        config: &NodeGraphConfig,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> Result<Self> {
        let mut node_graph = MaybeUninit::<sys::ma_node_graph>::uninit();
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);
        graph_ffi::ma_node_graph_init(config.get_raw(), alloc_cb, node_graph.as_mut_ptr());
        Ok(Self {
            inner: unsafe { node_graph.assume_init() },
            alloc_cb: alloc,
        })
    }

    #[inline]
    fn alloc_cb_ptr(&self) -> *const sys::ma_allocation_callbacks {
        match &self.alloc_cb {
            Some(cb) => &cb.inner as *const _,
            None => core::ptr::null(),
        }
    }

    #[inline]
    pub(crate) fn inner_ptr_mut(&self) -> *mut sys::ma_node_graph {
        // This does *not* create an `&mut` reference; it just produces a raw pointer.
        core::ptr::addr_of!(self.inner) as *mut sys::ma_node_graph
    }
}

mod graph_ffi {
    use maudio_sys::ffi as sys;

    use crate::engine::node_graph::{NodeGraph, nodes::NodeRef};

    #[inline]
    pub(crate) fn ma_node_graph_init(
        config: *const sys::ma_node_graph_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node_graph: *mut sys::ma_node_graph,
    ) -> i32 {
        unsafe { sys::ma_node_graph_init(config, alloc_cb, node_graph) }
    }

    #[inline]
    pub(crate) fn ma_node_graph_uninit(
        node_graph: *mut sys::ma_node_graph,
        alloc_cb: *const sys::ma_allocation_callbacks,
    ) {
        unsafe { sys::ma_node_graph_uninit(node_graph, alloc_cb) }
    }

    #[inline]
    pub(crate) fn ma_node_graph_get_endpoint<'a>(node_graph: &'a NodeGraph) -> NodeRef<'a> {
        let ptr = unsafe { sys::ma_node_graph_get_endpoint(node_graph.inner_ptr_mut()) };
        NodeRef::from_ptr(ptr)
    }

    #[inline]
    pub(crate) fn ma_node_graph_read_pcm_frames(
        node_graph: &mut NodeGraph,
        frames_out: *mut core::ffi::c_void,
        frame_count: u64,
        frames_read: *mut u64,
    ) -> i32 {
        unsafe {
            sys::ma_node_graph_read_pcm_frames(
                &mut node_graph.inner,
                frames_out,
                frame_count,
                frames_read,
            )
        }
    }

    #[inline]
    pub(crate) fn ma_node_graph_get_channels(node_graph: &NodeGraph) -> u32 {
        unsafe { sys::ma_node_graph_get_channels(&node_graph.inner as *const _) }
    }

    #[inline]
    pub(crate) fn ma_node_graph_get_time(node_graph: &NodeGraph) -> u64 {
        unsafe { sys::ma_node_graph_get_time(&node_graph.inner as *const _) }
    }

    #[inline]
    pub(crate) fn ma_node_graph_set_time(node_graph: &mut NodeGraph, global_time: u64) -> i32 {
        unsafe { sys::ma_node_graph_set_time(node_graph.inner_ptr_mut(), global_time) }
    }
}

impl<'a> Drop for NodeGraph<'a> {
    fn drop(&mut self) {
        unsafe {
            sys::ma_node_graph_uninit(&mut self.inner, self.alloc_cb_ptr());
        }
    }
}
