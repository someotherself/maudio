//! A pull-based audio processing graph.
use std::{
    mem::MaybeUninit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub mod node_builder;
pub(crate) mod node_flags;
pub mod node_graph_builder;
pub mod node_on_process;
pub mod node_vtable;
pub mod nodes;
mod voice; // experiment

use maudio_sys::ffi as sys;

use crate::{
    audio::formats::SampleBuffer,
    engine::{
        node_graph::{node_graph_builder::NodeGraphBuilder, nodes::NodeRef},
        AllocationCallbacks, Engine, EngineInner,
    },
    AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
};

/// `NodeGraph` is the root of miniaudio’s node-based audio system. It owns an
/// internal *endpoint node* and produces audio by **pulling** data from all
/// nodes connected upstream.
///
/// Unlike a traditional “pipeline” where audio is pushed into a graph,
/// a `NodeGraph` has **no external input**. Instead, audio is generated
/// on demand when the graph is read (for example by an audio device or
/// by calling [`NodeGraph::read_pcm_frames`]).
///
/// ## How audio flows
///
/// Each component in the diagram below represents a **node**. A `NodeGraph` is simply
/// a collection of interconnected nodes put together.
///
/// ```text
/// [Decoder / Generator Nodes] → [Mixers / Effects] → [Graph Endpoint] → playback
/// ```
///
/// In the high level API, you may only interact with an Engine. At its core,
/// an Engine is a NodeGraph, and performing dsp in the high level API
/// required you to create nodes that need to be added to the node graph.
///
/// This is when you must access the underlying node graph from the engine.
///
/// However, it is also possible to create and use a node graph outside of
/// this particular scenarion. The node graph is simply a tool that can be
/// used in various ways. The basic guiding rules are:
/// - The node graph is pull based. A call to read frames starts at the endpoint
///   (the final node) and then recursively requests frames from all the connected
///   nodes until it reaches the other end (the source nodes).
/// - The node graph will always satisfy the amount of frames requested. If no
///   frames are available from any source, it will produce silence.
/// - At other end, audio sources are connected. These sources can be a `Sound`,
///   noise, pulse, wave generators, an encoder etc... or even a capture device.
/// - The endpoint doesn't get connected to anything, at least not in the same way
///   nodes are connected within the graph. Instead, [`NodeGraph::read_pcm_frames`]
///   is called into an output buffer. This call can be made inside the audio callback
///   of a playback device directly into the `out` buffer, or into another
///   buffer (can be just a `Vec`) for further processing or to be used by an encoder.
/// - As you may already have noticed, the node graph can also be used in the
///   low level API, connected directly to a [`Device`](crate::device::Device).
///   This is not always necessary, but it is helpful if you are planning to do
///   more complex dsp. For more details on how to implement this, see the examples.
///
/// ## Thread-safety and usage
///
/// - In the node graph, calling read_pcm_frames is lock free. This is important as
///   it is often called inside the real time audio callback.
/// - However, `read_pcm_frames` is not thread safe. This is also applicable to any
///   type implementing this function. Only one call to this function is allowed
///   at any time. Otherwise, audio will be distorted.
///   Maudio tries to reduce situations that allow mutliple calls to read frames,
///   however currenly they are still possbile. Use these calls responsibly.
/// - Connecting and disconnecting nodes to the node graph have locks implemented
///   (spinlocks). Mutating the node graph from any thread while frames are being read is safe.
/// - Reading frames from the Node Graph is only possible by using the [`NodeGraphReader`].
///   It can be obtained via [`NodeGraph::try_acquire_reader`]. Only one instance of
///   `NodeGraphReader` can exist at any time.
///   For creating nodes, either the [`NodeGraph`] or the [`NodeGraphRef`] can be used.
pub struct NodeGraph {
    inner: Arc<GraphInner>,
}

unsafe impl Send for NodeGraph {}
unsafe impl Sync for NodeGraph {}

#[doc(hidden)]
pub struct GraphInner {
    pub(crate) base: *mut sys::ma_node_graph,
    pub(crate) alloc_cb: Option<Arc<AllocationCallbacks>>,
    pub(crate) reader_exists: Arc<AtomicBool>,
}

unsafe impl Send for GraphInner {}
unsafe impl Sync for GraphInner {}

impl Binding for NodeGraph {
    type Raw = *mut sys::ma_node_graph;

    fn to_raw(&self) -> Self::Raw {
        self.inner.base
    }
}

impl Binding for GraphInner {
    type Raw = *mut sys::ma_node_graph;

    fn to_raw(&self) -> Self::Raw {
        self.base
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
pub struct NodeGraphRef {
    pub(crate) inner: *mut sys::ma_node_graph,
    pub(crate) owner: GraphOwner,
}

unsafe impl Send for NodeGraphRef {}
unsafe impl Sync for NodeGraphRef {}

impl Binding for NodeGraphRef {
    type Raw = *mut sys::ma_node_graph;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl NodeGraphRef {
    /// Returns the owning engine, if any.
    pub fn engine(&self) -> Option<Engine> {
        self.owner.engine().map(Engine)
    }

    /// Returns the owning node graph, if any.
    pub fn graph(&self) -> Option<NodeGraph> {
        self.owner.graph().map(|g| NodeGraph { inner: g })
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub enum GraphOwner {
    Engine(Arc<EngineInner>),
    Graph(Arc<GraphInner>),
}

impl GraphOwner {
    pub(crate) fn engine(&self) -> Option<Arc<EngineInner>> {
        match self {
            Self::Engine(e) => Some(e.clone()),
            Self::Graph(_) => None,
        }
    }

    pub(crate) fn graph(&self) -> Option<Arc<GraphInner>> {
        match self {
            Self::Engine(_) => None,
            Self::Graph(g) => Some(g.clone()),
        }
    }
}

/// Dedicated type for reading frames from a Node Graph
///
/// Concurrent calls to `read_pcm_frames` are not safe. This type exists to enforce this.
pub struct NodeGraphReader(Arc<GraphInner>);

impl Binding for NodeGraphReader {
    type Raw = *mut sys::ma_node_graph;

    fn to_raw(&self) -> Self::Raw {
        self.0.base
    }
}

impl NodeGraphReader {
    /// Reads PCM frames into `dst`, returning the number of frames read.
    pub fn read_pcm_frames_into(&mut self, dst: &mut [f32]) -> MaResult<usize> {
        graph_ffi::ma_node_graph_read_pcm_frames_into(self, dst)
    }

    /// Allocates and reads `frame_count` PCM frames from the graph.
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<SampleBuffer<f32>> {
        graph_ffi::ma_node_graph_read_pcm_frames(self, frame_count)
    }
}

pub(crate) mod private_node_graph {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait NodeGraphPtrProvider<T: ?Sized> {
        fn as_node_graph_ptr(t: &T) -> *mut sys::ma_node_graph;
        fn clone_owner(t: &T) -> GraphOwner;
    }

    pub struct NodeGraphProvider;
    pub struct NodeGraphRefProvider;
    pub struct NodeGraphReaderProvider;

    impl NodeGraphPtrProvider<NodeGraph> for NodeGraphProvider {
        #[inline]
        fn as_node_graph_ptr(t: &NodeGraph) -> *mut sys::ma_node_graph {
            t.to_raw()
        }

        fn clone_owner(t: &NodeGraph) -> GraphOwner {
            GraphOwner::Graph(t.inner.clone())
        }
    }

    impl NodeGraphPtrProvider<NodeGraphRef> for NodeGraphRefProvider {
        #[inline]
        fn as_node_graph_ptr(t: &NodeGraphRef) -> *mut sys::ma_node_graph {
            t.to_raw()
        }

        fn clone_owner(t: &NodeGraphRef) -> GraphOwner {
            t.owner.clone()
        }
    }

    impl NodeGraphPtrProvider<NodeGraphReader> for NodeGraphReaderProvider {
        #[inline]
        fn as_node_graph_ptr(t: &NodeGraphReader) -> *mut sys::ma_node_graph {
            t.to_raw()
        }

        fn clone_owner(t: &NodeGraphReader) -> GraphOwner {
            GraphOwner::Graph(t.0.clone())
        }
    }

    pub fn node_graph_ptr<T: AsNodeGraphPtr + ?Sized>(t: &T) -> *mut sys::ma_node_graph {
        <T as AsNodeGraphPtr>::__PtrProvider::as_node_graph_ptr(t)
    }

    pub fn clone_owner<T: AsNodeGraphPtr + ?Sized>(t: &T) -> GraphOwner {
        <T as AsNodeGraphPtr>::__PtrProvider::clone_owner(t)
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
impl AsNodeGraphPtr for NodeGraphRef {
    type __PtrProvider = private_node_graph::NodeGraphRefProvider;
}

#[doc(hidden)]
impl AsNodeGraphPtr for NodeGraphReader {
    type __PtrProvider = private_node_graph::NodeGraphReaderProvider;
}

impl<T: AsNodeGraphPtr + ?Sized> NodeGraphOps for T {}

pub trait NodeGraphOps: AsNodeGraphPtr {
    /// Returns the endpoint node of the graph, if any.
    fn endpoint(&self) -> NodeRef<'_> {
        graph_ffi::ma_node_graph_get_endpoint(self)
    }

    /// Returns the number of output channels in the graph.
    fn channels(&self) -> u32 {
        graph_ffi::ma_node_graph_get_channels(self)
    }

    /// Returns the current global time in PCM frames.
    fn time(&self) -> u64 {
        graph_ffi::ma_node_graph_get_time(self)
    }
}

impl NodeGraph {
    pub fn try_acquire_reader(&self) -> MaResult<NodeGraphReader> {
        match self.inner.reader_exists.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(NodeGraphReader(self.inner.clone())),
            Err(_) => Err(MaudioError::new_ma_error(ErrorKinds::ReaderExists)),
        }
    }

    /// Sets the global time in PCM frames.
    pub fn set_time(&mut self, global_time: u64) -> MaResult<()> {
        let res = graph_ffi::ma_node_graph_set_time(self, global_time);
        MaudioError::check(res)
    }

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
            inner: Arc::new(GraphInner {
                base: inner,
                alloc_cb: alloc,
                reader_exists: Arc::new(AtomicBool::new(false)),
            }),
        })
    }

    /// Create a NodeGraphRef borrowing the NodeGraph
    pub fn as_ref(&self) -> NodeGraphRef {
        NodeGraphRef {
            inner: self.inner.base,
            owner: GraphOwner::Graph(self.inner.clone()),
        }
    }
}

mod graph_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::formats::SampleBuffer,
        engine::{
            node_graph::{
                nodes::NodeRef, private_node_graph, AsNodeGraphPtr, GraphInner, NodeGraphOps,
            },
            AllocationCallbacks,
        },
        AsRawRef, Binding, MaResult, MaudioError,
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
        node_graph: &mut GraphInner,
        alloc: Option<Arc<AllocationCallbacks>>,
    ) {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        unsafe { sys::ma_node_graph_uninit(node_graph.to_raw(), alloc_cb) }
    }

    #[inline]
    pub(crate) fn ma_node_graph_get_endpoint<'a, N: AsNodeGraphPtr + ?Sized>(
        node_graph: &'a N,
    ) -> NodeRef<'a> {
        let ptr = unsafe {
            sys::ma_node_graph_get_endpoint(private_node_graph::node_graph_ptr(node_graph))
        };
        NodeRef::from_ptr(ptr)
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

impl Drop for GraphInner {
    fn drop(&mut self) {
        let alloc_cb = self.alloc_cb.clone();
        graph_ffi::ma_node_graph_uninit(self, alloc_cb);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

impl Drop for NodeGraphReader {
    fn drop(&mut self) {
        self.0.reader_exists.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod test {
    use crate::engine::node_graph::node_graph_builder::NodeGraphBuilder;

    #[test]
    fn test_node_graph_acquire_reader() {
        let graph = NodeGraphBuilder::new(2).build().unwrap();
        let res = graph.try_acquire_reader();
        assert!(res.is_ok());
    }

    #[test]
    fn test_node_graph_acquire_reader_twice() {
        let graph = NodeGraphBuilder::new(2).build().unwrap();
        let _reader = graph.try_acquire_reader().unwrap();

        let res = graph.try_acquire_reader();
        assert!(res.is_err());
    }

    #[test]
    fn test_node_graph_acquire_reader_drop_reacquire() {
        let graph = NodeGraphBuilder::new(2).build().unwrap();
        let mut reader = graph.try_acquire_reader().unwrap();
        let res = reader.read_pcm_frames(10);
        assert!(res.is_ok());
        drop(reader);

        let res = graph.try_acquire_reader();
        assert!(res.is_ok());
    }
}
