use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    Binding, MaResult,
    engine::{
        AllocationCallbacks,
        node_graph::{
            AsNodeGraphPtr, NodeGraph,
            nodes::{AsNodePtr, NodeRef, private_node::DataSourceNodeProvider},
        },
    },
};

pub(crate) struct DataSourceNode<'a> {
    inner: *mut sys::ma_data_source_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
}

impl Binding for DataSourceNode<'_> {
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
impl AsNodePtr for DataSourceNode<'_> {
    type __PtrProvider = DataSourceNodeProvider;
}

impl<'a> DataSourceNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &DataSourceNodeBuilder<'_, N>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_data_source_node>> = Box::new_uninit();

        n_datasource_ffi::ma_data_source_node_init(
            node_graph,
            config.to_raw(),
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        let ptr: Box<sys::ma_data_source_node> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_data_source_node = Box::into_raw(ptr);

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

pub(crate) mod n_datasource_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        Binding, MaRawResult, MaResult,
        engine::node_graph::{
            AsNodeGraphPtr, nodes::source::data_source::DataSourceNode, private_node_graph,
        },
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
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_data_source_node_uninit(node: &mut DataSourceNode) {
        unsafe {
            sys::ma_data_source_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }
}

impl<'a> Drop for DataSourceNode<'a> {
    fn drop(&mut self) {
        n_datasource_ffi::ma_data_source_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub(crate) struct DataSourceNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_data_source_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> Binding for DataSourceNodeBuilder<'_, N> {
    type Raw = *const sys::ma_data_source_node_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}
