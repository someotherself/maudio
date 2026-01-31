use std::{cell::Cell, marker::PhantomData, mem::MaybeUninit};

pub mod node_builder;
pub mod node_flags;
pub mod node_graph_builder;
pub mod nodes;
pub mod voice;

/// Prelude for the [`node_graph`](super) module.
///
/// This module re-exports the most commonly used engine types and traits
/// so they can be imported with a single global import.
///
/// Import this when you want access to [`NodeGraph`] and [`NodeGraphRef`] and all shared engine
/// methods (provided by [`EngineOps`](crate::engine)) without having to import each item
/// individually.
/// This is purely a convenience module; importing directly
/// works just as well if you prefer explicit imports.
pub mod prelude {
    pub use super::{NodeGraph, NodeGraphRef};
}

use maudio_sys::ffi as sys;

use crate::{
    engine::{
        node_graph::{node_graph_builder::NodeGraphBuilder, nodes::NodeRef},
        AllocationCallbacks, Engine,
    },
    Binding, MaRawResult, MaResult,
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
/// by calling `ma_node_graph_read_pcm_frames`).
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
    inner: *mut sys::ma_node_graph,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _not_sync: PhantomData<Cell<()>>,
}

impl Binding for NodeGraph<'_> {
    type Raw = *mut sys::ma_node_graph;

    /// !!! Not Implemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
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
#[derive(Clone, Copy)]
pub struct NodeGraphRef<'e> {
    ptr: *mut sys::ma_node_graph,
    _engine: PhantomData<&'e mut Engine>,
    // Should Ref also be not_sync?
    _not_sync: PhantomData<Cell<()>>,
}

impl Binding for NodeGraphRef<'_> {
    type Raw = *mut sys::ma_node_graph;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            ptr: raw,
            _engine: PhantomData,
            _not_sync: PhantomData,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.ptr
    }
}

pub(crate) mod private_node_graph {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait NodeGraphPtrProvider<T: ?Sized> {
        fn as_node_graph_ptr(t: &T) -> *mut sys::ma_node_graph;
    }

    pub struct NodeGraphProvider;
    pub struct NodeGraphRefProvider;

    impl<'a> NodeGraphPtrProvider<NodeGraph<'a>> for NodeGraphProvider {
        #[inline]
        fn as_node_graph_ptr(t: &NodeGraph) -> *mut sys::ma_node_graph {
            t.to_raw()
        }
    }

    impl<'a> NodeGraphPtrProvider<NodeGraphRef<'a>> for NodeGraphRefProvider {
        #[inline]
        fn as_node_graph_ptr(t: &NodeGraphRef) -> *mut sys::ma_node_graph {
            t.to_raw()
        }
    }

    pub fn node_graph_ptr<T: AsNodeGraphPtr + ?Sized>(t: &T) -> *mut sys::ma_node_graph {
        <T as AsNodeGraphPtr>::__PtrProvider::as_node_graph_ptr(t)
    }
}

#[doc(hidden)]
pub trait AsNodeGraphPtr {
    type __PtrProvider: private_node_graph::NodeGraphPtrProvider<Self>;
}

#[doc(hidden)]
impl AsNodeGraphPtr for NodeGraph<'_> {
    type __PtrProvider = private_node_graph::NodeGraphProvider;
}

#[doc(hidden)]
impl AsNodeGraphPtr for NodeGraphRef<'_> {
    type __PtrProvider = private_node_graph::NodeGraphRefProvider;
}

impl<T: AsNodeGraphPtr + ?Sized> NodeGraphOps for T {}

pub trait NodeGraphOps: AsNodeGraphPtr {
    fn endpoint(&self) -> Option<NodeRef<'_>> {
        graph_ffi::ma_node_graph_get_endpoint(self)
    }

    fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<f32>, u64)> {
        graph_ffi::ma_node_graph_read_pcm_frames(self, frame_count)
    }

    fn channels(&self) -> u32 {
        graph_ffi::ma_node_graph_get_channels(self)
    }

    fn time(&self) -> u64 {
        graph_ffi::ma_node_graph_get_time(self)
    }

    fn set_time(&mut self, global_time: u64) -> MaResult<()> {
        let res = graph_ffi::ma_node_graph_set_time(self, global_time);
        MaRawResult::check(res)
    }
}

// These should not be available to NodeGraphRef
impl<'a> NodeGraph<'a> {
    fn with_alloc_callbacks(
        config: &NodeGraphBuilder,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_node_graph>> = Box::new(MaybeUninit::uninit());

        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);
        graph_ffi::ma_node_graph_init(&config.to_raw() as *const _, alloc_cb, mem.as_mut_ptr())?;
        let mem: Box<sys::ma_node_graph> = unsafe { mem.assume_init() };
        let inner = Box::into_raw(mem);

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _not_sync: PhantomData,
        })
    }

    #[inline]
    fn alloc_cb_ptr(&self) -> *const sys::ma_allocation_callbacks {
        match &self.alloc_cb {
            Some(cb) => &cb.inner as *const _,
            None => core::ptr::null(),
        }
    }
}

mod graph_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        engine::node_graph::{nodes::NodeRef, private_node_graph, AsNodeGraphPtr, NodeGraphOps},
        Binding, MaRawResult, MaResult,
    };

    #[inline]
    pub(crate) fn ma_node_graph_init(
        config: *const sys::ma_node_graph_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node_graph: *mut sys::ma_node_graph,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_node_graph_init(config, alloc_cb, node_graph) };
        MaRawResult::check(res)
    }

    #[inline]
    pub(crate) fn ma_node_graph_uninit(
        node_graph: *mut sys::ma_node_graph,
        alloc_cb: *const sys::ma_allocation_callbacks,
    ) {
        unsafe { sys::ma_node_graph_uninit(node_graph, alloc_cb) }
    }

    #[inline]
    pub(crate) fn ma_node_graph_get_endpoint<'a, N: AsNodeGraphPtr + ?Sized>(
        node_graph: &'a N,
    ) -> Option<NodeRef<'a>> {
        let ptr = unsafe {
            sys::ma_node_graph_get_endpoint(private_node_graph::node_graph_ptr(node_graph))
        };
        if ptr.is_null() {
            None
        } else {
            Some(NodeRef::from_ptr(ptr))
        }
    }

    #[inline]
    pub(crate) fn ma_node_graph_read_pcm_frames<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &mut N,
        frame_count: u64,
    ) -> MaResult<(Vec<f32>, u64)> {
        let channels = node_graph.channels();
        let mut buffer = vec![0.0f32; (frame_count * channels as u64) as usize];
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_node_graph_read_pcm_frames(
                private_node_graph::node_graph_ptr(node_graph),
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
                frame_count,
                &mut frames_read,
            )
        };
        MaRawResult::check(res)?;
        Ok((buffer, frames_read))
    }

    #[inline]
    pub(crate) fn ma_node_graph_get_channels<N: AsNodeGraphPtr + ?Sized>(node_graph: &N) -> u32 {
        unsafe {
            sys::ma_node_graph_get_channels(
                private_node_graph::node_graph_ptr(node_graph) as *const _
            )
        }
    }

    #[inline]
    pub(crate) fn ma_node_graph_get_time<N: AsNodeGraphPtr + ?Sized>(node_graph: &N) -> u64 {
        unsafe {
            sys::ma_node_graph_get_time(private_node_graph::node_graph_ptr(node_graph) as *const _)
        }
    }

    #[inline]
    pub(crate) fn ma_node_graph_set_time<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &mut N,
        global_time: u64,
    ) -> i32 {
        unsafe {
            sys::ma_node_graph_set_time(private_node_graph::node_graph_ptr(node_graph), global_time)
        }
    }
}

impl<'a> Drop for NodeGraph<'a> {
    fn drop(&mut self) {
        graph_ffi::ma_node_graph_uninit(self.to_raw(), self.alloc_cb_ptr());
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}
