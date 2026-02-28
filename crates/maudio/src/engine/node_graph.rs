//! A pull-based audio processing graph.
use std::{cell::Cell, marker::PhantomData, mem::MaybeUninit, sync::Arc};

mod node_builder; // Creating nodes is not implemented yet.
mod node_flags; // Creating nodes is not implemented yet.
pub mod node_graph_builder;
pub mod nodes;
mod voice;

use maudio_sys::ffi as sys;

use crate::{
    audio::formats::SampleBuffer,
    engine::{
        node_graph::{node_graph_builder::NodeGraphBuilder, nodes::NodeRef},
        AllocationCallbacks, Engine,
    },
    AsRawRef, Binding, MaResult, MaudioError,
};

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
pub struct NodeGraph {
    inner: *mut sys::ma_node_graph,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    _not_sync: PhantomData<Cell<()>>,
}

impl Binding for NodeGraph {
    type Raw = *mut sys::ma_node_graph;

    /// !!! Not Implemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

/// Borrowed view of a node graph
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
    ptr: *mut sys::ma_node_graph,
    _engine: PhantomData<&'e mut Engine>,
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

    impl NodeGraphPtrProvider<NodeGraph> for NodeGraphProvider {
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
impl AsNodeGraphPtr for NodeGraph {
    type __PtrProvider = private_node_graph::NodeGraphProvider;
}

#[doc(hidden)]
impl AsNodeGraphPtr for NodeGraphRef<'_> {
    type __PtrProvider = private_node_graph::NodeGraphRefProvider;
}

impl<T: AsNodeGraphPtr + ?Sized> NodeGraphOps for T {}

pub trait NodeGraphOps: AsNodeGraphPtr {
    /// Returns the endpoint node of the graph, if any.
    fn endpoint(&self) -> Option<NodeRef<'_>> {
        graph_ffi::ma_node_graph_get_endpoint(self)
    }

    /// Reads PCM frames into `dst`, returning the number of frames read.
    fn read_pcm_frames_into(&mut self, dst: &mut [f32]) -> MaResult<usize> {
        graph_ffi::ma_node_graph_read_pcm_frames_into(self, dst)
    }

    /// Allocates and reads `frame_count` PCM frames from the graph.
    fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<SampleBuffer<f32>> {
        graph_ffi::ma_node_graph_read_pcm_frames(self, frame_count)
    }

    /// Returns the number of output channels in the graph.
    fn channels(&self) -> u32 {
        graph_ffi::ma_node_graph_get_channels(self)
    }

    /// Returns the current global time in PCM frames.
    fn time(&self) -> u64 {
        graph_ffi::ma_node_graph_get_time(self)
    }

    /// Sets the global time in PCM frames.
    fn set_time(&mut self, global_time: u64) -> MaResult<()> {
        let res = graph_ffi::ma_node_graph_set_time(self, global_time);
        MaudioError::check(res)
    }
}

impl NodeGraph {
    fn with_alloc_callbacks(
        config: &NodeGraphBuilder,
        alloc: Option<Arc<AllocationCallbacks>>,
    ) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_node_graph>> = Box::new(MaybeUninit::uninit());

        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        graph_ffi::ma_node_graph_init(config.as_raw_ptr(), alloc_cb, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_node_graph = Box::into_raw(mem) as *mut sys::ma_node_graph;
        Ok(Self {
            inner,
            alloc_cb: alloc,
            _not_sync: PhantomData,
        })
    }

    #[inline]
    fn alloc_cb_ptr(&self) -> *const sys::ma_allocation_callbacks {
        match &self.alloc_cb {
            Some(cb) => cb.as_raw_ptr(),
            None => core::ptr::null(),
        }
    }
}

mod graph_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        audio::formats::SampleBuffer,
        engine::node_graph::{nodes::NodeRef, private_node_graph, AsNodeGraphPtr, NodeGraphOps},
        Binding, MaResult, MaudioError,
    };

    #[inline]
    pub(crate) fn ma_node_graph_init(
        config: *const sys::ma_node_graph_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node_graph: *mut sys::ma_node_graph,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_node_graph_init(config, alloc_cb, node_graph) };
        MaudioError::check(res)
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
    pub fn ma_node_graph_read_pcm_frames_into<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &mut N,
        dst: &mut [f32],
    ) -> MaResult<usize> {
        let channels = node_graph.channels();
        let len = dst.len() as u64;

        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        // May truncate, and that is desired
        let frame_count = len / channels as u64;

        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_node_graph_read_pcm_frames(
                private_node_graph::node_graph_ptr(node_graph),
                dst.as_mut_ptr() as *mut std::ffi::c_void,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;

        Ok(frames_read as usize)
    }

    #[inline]
    pub(crate) fn ma_node_graph_read_pcm_frames<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &mut N,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<f32>> {
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
        MaudioError::check(res)?;
        SampleBuffer::<f32>::from_storage(buffer, frames_read as usize, channels)
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

impl Drop for NodeGraph {
    fn drop(&mut self) {
        graph_ffi::ma_node_graph_uninit(self.to_raw(), self.alloc_cb_ptr());
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}
