use std::{mem::MaybeUninit, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    data_source::{private_data_source, AsSourcePtr, DataSourceRef},
    engine::{
        node_graph::{
            nodes::{
                node_ffi,
                private_node::{AttachedSourceNodeProvider, SourceNodeProvider},
                AsNodePtr, NodeRef,
            },
            private_node_graph, AsNodeGraphPtr, GraphOwner, NodeGraph, NodeGraphRef,
        },
        AllocationCallbacks, Engine,
    },
    AsRawRef, Binding, MaResult,
};

pub struct SourceNode<'a, S: AsSourcePtr> {
    inner: *mut sys::ma_data_source_node,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    _source: &'a S,
    pub(crate) owner: GraphOwner,
}

impl<S: AsSourcePtr> Binding for SourceNode<'_, S> {
    type Raw = *mut sys::ma_data_source_node;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[doc(hidden)]
impl<S: AsSourcePtr> AsNodePtr for SourceNode<'_, S> {
    type __PtrProvider = SourceNodeProvider;
}

impl<'a, S: AsSourcePtr> SourceNode<'a, S> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr>(
        node_graph: &N,
        config: &SourceNodeBuilder<'a, N, S>,
        alloc: Option<Arc<AllocationCallbacks>>,
        source: &'a S,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_data_source_node>> =
            Box::new(MaybeUninit::uninit());

        n_datasource_ffi::ma_data_source_node_init(
            node_graph,
            config.as_raw_ptr(),
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_data_source_node =
            Box::into_raw(mem) as *mut sys::ma_data_source_node;

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _source: source,
            owner: private_node_graph::clone_owner(node_graph),
        })
    }

    /// Returns the owning engine, if any.
    pub fn engine(&self) -> Option<Engine> {
        self.owner.engine().map(Engine)
    }

    /// Returns the owning node graph, if any.
    pub fn node_graph(&self) -> Option<NodeGraph> {
        self.owner.graph().map(|g| NodeGraph { inner: g })
    }

    /// Returns a reference to the node graph.
    pub fn node_graph_ref(&self) -> NodeGraphRef {
        let ptr = node_ffi::ma_node_get_node_graph(self);
        NodeGraphRef {
            inner: ptr,
            owner: self.owner.clone(),
        }
    }

    /// Returns a **borrowed view** as a node in the engine's node graph.
    pub fn as_node(&self) -> NodeRef<'a> {
        assert!(!self.to_raw().is_null());
        let ptr = self.to_raw().cast::<sys::ma_node>();
        NodeRef::from_ptr(ptr)
    }

    #[inline]
    fn alloc_cb_ptr(&self) -> *const sys::ma_allocation_callbacks {
        match &self.alloc_cb {
            Some(cb) => cb.as_raw_ptr(),
            None => core::ptr::null(),
        }
    }
}

pub struct AttachedSourceNode<S: AsSourcePtr> {
    inner: *mut sys::ma_data_source_node,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    source: S,
    pub(crate) owner: GraphOwner,
}

unsafe impl<S: AsSourcePtr> Send for AttachedSourceNode<S> {}

impl<S: AsSourcePtr> Binding for AttachedSourceNode<S> {
    type Raw = *mut sys::ma_data_source_node;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[doc(hidden)]
impl<S: AsSourcePtr> AsNodePtr for AttachedSourceNode<S> {
    type __PtrProvider = AttachedSourceNodeProvider;
}

#[doc(hidden)]
impl<S: AsSourcePtr> AsSourcePtr for AttachedSourceNode<S> {
    type Format = S::Format;
    type __PtrProvider = private_data_source::AttachedSourceNodeProvider;
}

impl<S: AsSourcePtr> AttachedSourceNode<S> {
    fn new_with_cfg_alloc_internal<'a, N: AsNodeGraphPtr>(
        node_graph: &N,
        config: AttachedSourceNodeBuilder<'a, N, S>,
        alloc: Option<Arc<AllocationCallbacks>>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_data_source_node>> =
            Box::new(MaybeUninit::uninit());

        n_datasource_ffi::ma_data_source_node_init(
            node_graph,
            config.as_raw_ptr(),
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_data_source_node =
            Box::into_raw(mem) as *mut sys::ma_data_source_node;

        Ok(Self {
            inner,
            alloc_cb: alloc,
            source: config.source,
            owner: private_node_graph::clone_owner(node_graph),
        })
    }

    /// Returns the owning engine, if any.
    pub fn engine(&self) -> Option<Engine> {
        self.owner.engine().map(Engine)
    }

    /// Returns the owning node graph, if any.
    pub fn node_graph(&self) -> Option<NodeGraph> {
        self.owner.graph().map(|g| NodeGraph { inner: g })
    }

    /// Returns a refer            inner: g,h.
    pub fn node_graph_ref(&self) -> NodeGraphRef {
        let ptr = node_ffi::ma_node_get_node_graph(self);
        NodeGraphRef {
            inner: ptr,
            owner: self.owner.clone(),
        }
    }

    /// Returns a **borrowed view** as a node in the engine's node graph.
    pub fn as_node<'a>(&'a self) -> NodeRef<'a> {
        assert!(!self.to_raw().is_null());
        let ptr = self.to_raw().cast::<sys::ma_node>();
        NodeRef::from_ptr(ptr)
    }

    /// Retrieve a reference to the underlying source
    pub fn source(&self) -> &S {
        &self.source
    }

    /// Retrieve a reference to the underlying source
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    pub fn as_source_ref<'a>(&'a self) -> DataSourceRef<'a, S::Format> {
        debug_assert!(!private_data_source::source_ptr(&self.source).is_null());
        let ptr = private_data_source::source_ptr(&self.source).cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }

    #[inline]
    fn alloc_cb_ptr(&self) -> *const sys::ma_allocation_callbacks {
        match &self.alloc_cb {
            Some(cb) => cb.as_raw_ptr(),
            None => core::ptr::null(),
        }
    }
}

pub(crate) mod n_datasource_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        data_source::AsSourcePtr,
        engine::node_graph::{
            nodes::source::source_node::{AttachedSourceNode, SourceNode},
            private_node_graph, AsNodeGraphPtr,
        },
        Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_data_source_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_data_source_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_data_source_node,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_data_source_node_init(
                private_node_graph::node_graph_ptr(node_graph),
                config,
                alloc_cb,
                node,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_data_source_node_uninit<S: AsSourcePtr>(node: &mut SourceNode<S>) {
        unsafe {
            sys::ma_data_source_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }

    // If more functions for AttachedSourceNode get added, create the common trait
    #[inline]
    pub fn ma_attached_data_source_node_uninit<S: AsSourcePtr>(node: &mut AttachedSourceNode<S>) {
        unsafe {
            sys::ma_data_source_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }
}

impl<'a, S: AsSourcePtr> Drop for SourceNode<'a, S> {
    fn drop(&mut self) {
        n_datasource_ffi::ma_data_source_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

impl<S: AsSourcePtr> Drop for AttachedSourceNode<S> {
    fn drop(&mut self) {
        n_datasource_ffi::ma_attached_data_source_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

/// Builder for creating a [`SourceNode`]
pub struct SourceNodeBuilder<'a, N, S>
where
    N: AsNodeGraphPtr + ?Sized,
    S: AsSourcePtr + ?Sized,
{
    inner: sys::ma_data_source_node_config,
    node_graph: &'a N,
    source: &'a S,
}

impl<N, S> AsRawRef for SourceNodeBuilder<'_, N, S>
where
    N: AsNodeGraphPtr + ?Sized,
    S: AsSourcePtr + ?Sized,
{
    type Raw = sys::ma_data_source_node_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<'a, N, S> SourceNodeBuilder<'a, N, S>
where
    N: AsNodeGraphPtr,
    S: AsSourcePtr,
{
    pub fn new(node_graph: &'a N, source: &'a S) -> Self {
        let inner = unsafe {
            sys::ma_data_source_node_config_init(private_data_source::source_ptr(source))
        };
        Self {
            inner,
            node_graph,
            source,
        }
    }

    pub fn build(&self) -> MaResult<SourceNode<'a, S>> {
        SourceNode::new_with_cfg_alloc_internal(self.node_graph, self, None, self.source)
    }
}

pub struct AttachedSourceNodeBuilder<'a, N, S>
where
    N: AsNodeGraphPtr,
    S: AsSourcePtr,
{
    inner: sys::ma_data_source_node_config,
    node_graph: &'a N,
    source: S,
}

impl<N, S> AsRawRef for AttachedSourceNodeBuilder<'_, N, S>
where
    N: AsNodeGraphPtr,
    S: AsSourcePtr,
{
    type Raw = sys::ma_data_source_node_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<'a, N: AsNodeGraphPtr, S: AsSourcePtr> AttachedSourceNodeBuilder<'a, N, S> {
    pub fn new(node_graph: &'a N, source: S) -> Self {
        let inner = unsafe {
            sys::ma_data_source_node_config_init(private_data_source::source_ptr(&source))
        };
        Self {
            inner,
            node_graph,
            source,
        }
    }

    pub fn build(self) -> MaResult<AttachedSourceNode<S>> {
        AttachedSourceNode::new_with_cfg_alloc_internal(self.node_graph, self, None)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        data_source::sources::buffer::AudioBufferBuilder,
        engine::{
            node_graph::nodes::source::source_node::{
                AttachedSourceNodeBuilder, SourceNodeBuilder,
            },
            Engine,
        },
        Binding,
    };

    fn ramp_f32_interleaved(channels: u32, frames: u64) -> Vec<f32> {
        let mut data = vec![0.0f32; (channels as usize) * (frames as usize)];
        for f in 0..frames as usize {
            for c in 0..channels as usize {
                // unique value per (frame, channel)
                data[f * channels as usize + c] = (f as f32) * 10.0 + (c as f32);
            }
        }
        data
    }

    #[test]
    fn source_node_test_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
        let graph = engine.as_node_graph();

        let data = ramp_f32_interleaved(2, 32);

        let buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        let src_node = SourceNodeBuilder::new(&graph, &buf).build().unwrap();
        drop(src_node);
    }

    #[test]
    fn source_node_test_as_node_non_null_and_stable() {
        let engine = Engine::new_for_tests().unwrap();
        let graph = engine.as_node_graph();

        let data = ramp_f32_interleaved(2, 32);
        let buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        let src_node = SourceNodeBuilder::new(&graph, &buf).build().unwrap();

        let n1 = src_node.as_node().to_raw();
        let n2 = src_node.as_node().to_raw();

        assert!(!n1.is_null());
        assert_eq!(n1, n2);
    }

    #[test]
    fn source_node_test_multiple_nodes_same_source() {
        let engine = Engine::new_for_tests().unwrap();
        let graph = engine.as_node_graph();

        let data = ramp_f32_interleaved(2, 64);
        let buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        let n1 = SourceNodeBuilder::new(&graph, &buf).build().unwrap();
        let n2 = SourceNodeBuilder::new(&graph, &buf).build().unwrap();

        // sanity: they should be different nodes
        assert_ne!(n1.to_raw(), n2.to_raw());
        assert!(!n1.as_node().to_raw().is_null());
        assert!(!n2.as_node().to_raw().is_null());

        drop(n1);
        drop(n2);
    }

    #[test]
    fn source_node_test_multiple_sources() {
        let engine = Engine::new_for_tests().unwrap();
        let graph = engine.as_node_graph();

        let d1 = ramp_f32_interleaved(2, 32);
        let b1 = AudioBufferBuilder::build_f32(2, &d1).unwrap();

        let d2 = ramp_f32_interleaved(2, 48);
        let b2 = AudioBufferBuilder::build_f32(2, &d2).unwrap();

        let n1 = SourceNodeBuilder::new(&graph, &b1).build().unwrap();
        let n2 = SourceNodeBuilder::new(&graph, &b2).build().unwrap();

        assert_ne!(n1.to_raw(), n2.to_raw());
    }

    #[test]
    fn source_node_attached_builder() {
        let engine = Engine::new_for_tests().unwrap();
        let graph = engine.as_node_graph();

        let d1 = ramp_f32_interleaved(2, 32);
        let b1 = AudioBufferBuilder::build_f32(2, &d1).unwrap();

        let src_node = AttachedSourceNodeBuilder::new(&graph, b1).build().unwrap();

        let _ = src_node.as_source_ref();
        let buff = src_node.source();
        let _ = buff.length_pcm().unwrap();
    }
}
