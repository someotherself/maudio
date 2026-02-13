use std::{marker::PhantomData, mem::MaybeUninit, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    data_source::{private_data_source, AsSourcePtr, DataSourceRef},
    engine::{
        node_graph::{
            nodes::{
                private_node::{AttachedSourceNodeProvider, SourceNodeProvider},
                AsNodePtr, NodeRef,
            },
            AsNodeGraphPtr,
        },
        AllocationCallbacks,
    },
    Binding, MaResult,
};

pub struct SourceNode<'a> {
    inner: *mut sys::ma_data_source_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _src_graph: PhantomData<&'a ()>, // borrow to the graph and source
}

impl Binding for SourceNode<'_> {
    type Raw = *mut sys::ma_data_source_node;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[doc(hidden)]
impl AsNodePtr for SourceNode<'_> {
    type __PtrProvider = SourceNodeProvider;
}

impl<'a> SourceNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr, S: AsSourcePtr>(
        node_graph: &N,
        config: &SourceNodeBuilder<'a, N, S>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_data_source_node>> =
            Box::new(MaybeUninit::uninit());

        n_datasource_ffi::ma_data_source_node_init(
            node_graph,
            config.to_raw(),
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_data_source_node =
            Box::into_raw(mem) as *mut sys::ma_data_source_node;

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _src_graph: PhantomData,
        })
    }

    /// Returns a **borrowed view** as a node in the engine's node graph.
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

pub struct AttachedSourceNode<'a, S: AsSourcePtr> {
    inner: *mut sys::ma_data_source_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    source: Arc<S>,
    _graph: PhantomData<&'a ()>,
}

#[doc(hidden)]
impl<S: AsSourcePtr> AsNodePtr for AttachedSourceNode<'_, S> {
    type __PtrProvider = AttachedSourceNodeProvider;
}

#[doc(hidden)]
impl<S: AsSourcePtr> AsSourcePtr for AttachedSourceNode<'_, S> {
    type __PtrProvider = private_data_source::AttachedSourceNodeProvider;
}

impl<'a, S: AsSourcePtr> AttachedSourceNode<'a, S> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr>(
        node_graph: &N,
        config: &AttachedSourceNodeBuilder<'a, N, S>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_data_source_node>> =
            Box::new(MaybeUninit::uninit());

        n_datasource_ffi::ma_data_source_node_init(
            node_graph,
            config.to_raw(),
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_data_source_node =
            Box::into_raw(mem) as *mut sys::ma_data_source_node;

        Ok(Self {
            inner,
            alloc_cb: alloc,
            source: config.source.clone(),
            _graph: PhantomData,
        })
    }

    /// Returns a **borrowed view** as a node in the engine's node graph.
    pub fn as_node(&self) -> NodeRef<'a> {
        debug_assert!(!self.inner.is_null());
        let ptr = self.inner.cast::<sys::ma_node>();
        NodeRef::from_ptr(ptr)
    }

    pub fn as_source(&self) -> DataSourceRef<'_> {
        debug_assert!(!private_data_source::source_ptr(self.source.as_ref()).is_null());
        let ptr =
            private_data_source::source_ptr(self.source.as_ref()).cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }

    #[inline]
    fn alloc_cb_ptr(&self) -> *const sys::ma_allocation_callbacks {
        match &self.alloc_cb {
            Some(cb) => &cb.inner as *const _,
            None => core::ptr::null(),
        }
    }
}

pub(crate) mod n_datasource_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        engine::node_graph::{
            nodes::source::source_node::SourceNode, private_node_graph, AsNodeGraphPtr,
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
    pub fn ma_data_source_node_uninit(node: &mut SourceNode) {
        unsafe {
            sys::ma_data_source_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }
}

impl<'a> Drop for SourceNode<'a> {
    fn drop(&mut self) {
        n_datasource_ffi::ma_data_source_node_uninit(self);
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

impl<N, S> Binding for SourceNodeBuilder<'_, N, S>
where
    N: AsNodeGraphPtr,
    S: AsSourcePtr,
{
    type Raw = *const sys::ma_data_source_node_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
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

    pub fn build(&self) -> MaResult<SourceNode<'a>> {
        SourceNode::new_with_cfg_alloc_internal(self.node_graph, self, None)
    }
}

pub struct AttachedSourceNodeBuilder<'a, N, S>
where
    N: AsNodeGraphPtr,
    S: AsSourcePtr,
{
    inner: sys::ma_data_source_node_config,
    node_graph: &'a N,
    source: Arc<S>,
}

impl<N, S> Binding for AttachedSourceNodeBuilder<'_, N, S>
where
    N: AsNodeGraphPtr,
    S: AsSourcePtr,
{
    type Raw = *const sys::ma_data_source_node_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

impl<'a, N: AsNodeGraphPtr, S: AsSourcePtr> AttachedSourceNodeBuilder<'a, N, S> {
    pub fn new(node_graph: &'a N, source: S) -> Self {
        let src_arc = Arc::new(source);
        let inner = unsafe {
            sys::ma_data_source_node_config_init(private_data_source::source_ptr(src_arc.as_ref()))
        };
        Self {
            inner,
            node_graph,
            source: src_arc,
        }
    }

    pub fn build(&self) -> MaResult<AttachedSourceNode<'a, S>> {
        AttachedSourceNode::new_with_cfg_alloc_internal(self.node_graph, self, None)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        data_source::sources::buffer::AudioBufferBuilder,
        engine::{node_graph::nodes::source::source_node::SourceNodeBuilder, Engine, EngineOps},
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
        let graph = engine.as_node_graph().unwrap();

        let data = ramp_f32_interleaved(2, 32);

        let buf = AudioBufferBuilder::from_f32(2, 32, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        let src_node = SourceNodeBuilder::new(&graph, &buf).build().unwrap();
        drop(src_node);
    }

    #[test]
    fn source_node_test_as_node_non_null_and_stable() {
        let engine = Engine::new_for_tests().unwrap();
        let graph = engine.as_node_graph().unwrap();

        let data = ramp_f32_interleaved(2, 32);
        let buf = AudioBufferBuilder::from_f32(2, 32, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        let src_node = SourceNodeBuilder::new(&graph, &buf).build().unwrap();

        let n1 = src_node.as_node().to_raw();
        let n2 = src_node.as_node().to_raw();

        assert!(!n1.is_null());
        assert_eq!(n1, n2);
    }

    #[test]
    fn source_node_test_multiple_nodes_same_source() {
        let engine = Engine::new_for_tests().unwrap();
        let graph = engine.as_node_graph().unwrap();

        let data = ramp_f32_interleaved(2, 64);
        let buf = AudioBufferBuilder::from_f32(2, 64, &data)
            .unwrap()
            .build_copy()
            .unwrap();

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
        let graph = engine.as_node_graph().unwrap();

        let d1 = ramp_f32_interleaved(2, 32);
        let b1 = AudioBufferBuilder::from_f32(2, 32, &d1)
            .unwrap()
            .build_copy()
            .unwrap();

        let d2 = ramp_f32_interleaved(2, 48);
        let b2 = AudioBufferBuilder::from_f32(2, 48, &d2)
            .unwrap()
            .build_copy()
            .unwrap();

        let n1 = SourceNodeBuilder::new(&graph, &b1).build().unwrap();
        let n2 = SourceNodeBuilder::new(&graph, &b2).build().unwrap();

        assert_ne!(n1.to_raw(), n2.to_raw());
    }
}
