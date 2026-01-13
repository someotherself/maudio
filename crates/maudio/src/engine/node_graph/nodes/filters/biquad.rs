use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    Binding, Result,
    engine::{
        AllocationCallbacks,
        node_graph::{AsNodeGraphPtr, NodeGraph, nodes::NodeRef},
    },
};

/// A node that applies a biquad filtering to an audio signal.
/// 
/// `BiquadNode` is one of the custom DSP nodes provided by miniaudio.
/// 
/// By changing its coefficients, the same filter structure can act as low-pass, high-pass,
/// EQ, or notch filters while maintaining continuous state for real-time processing.

/// ## Notes
/// - After creating the filter, use [`Self::reinit`] to change the values of the coefficients.
/// This reinitializes the filter coefficients without clearing the internal state.
/// This allows filter parameters to be updated in real time without causing
/// audible artifacts such as clicks or pops.
/// - Changing the format or channel count after initialization is invalid and
/// will result in an error.
/// 
/// Use [`BiquadNodeBuilder`] to initialize
pub struct BiquadNode<'a> {
    inner: *mut sys::ma_biquad_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
    // May be needed during a reinit
    channels: u32,
    format: u32,
}

impl Binding for BiquadNode<'_> {
    type Raw = *mut sys::ma_biquad_node;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a> BiquadNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &BiquadNodeBuilder<N>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> Result<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_biquad_node>> = Box::new_uninit();

        n_biquad_ffi::ma_biquad_node_init(node_graph, config.to_raw(), alloc_cb, mem.as_mut_ptr())?;

        let ptr = unsafe { mem.assume_init() };
        let inner = Box::into_raw(ptr);
        Ok(Self {
            inner,
            alloc_cb: alloc,
            _marker: PhantomData,
            channels: config.inner.biquad.channels,
            format: config.inner.biquad.format,
        })
    }

    fn reinit(&mut self, config: &BiquadNodeParams) -> Result<()> {
        n_biquad_ffi::ma_biquad_node_reinit(config.to_raw(), self)
    }

    /// Returns a **borrowed view** of this sound as a node in the engine's node graph.
    ///
    /// ### What this is for
    ///
    /// Use `as_node()` when you want to:
    /// - connect this sound to other nodes (effects, mixers, splitters, etc.)
    /// - insert the sound into a custom routing graph
    /// - query node-level state exposed by the graph
    pub fn as_node(&'a self) -> NodeRef<'a> {
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

pub(crate) mod n_biquad_ffi {
    use crate::{
        Binding, MaRawResult, Result,
        engine::node_graph::{AsNodeGraphPtr, nodes::filters::biquad::BiquadNode},
    };
    use maudio_sys::ffi as sys;

    pub fn ma_biquad_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_biquad_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_biquad_node,
    ) -> Result<()> {
        let res = unsafe {
            sys::ma_biquad_node_init(node_graph.as_nodegraph_ptr(), config, alloc_cb, node)
        };
        MaRawResult::resolve(res)
    }

    pub fn ma_biquad_node_uninit(node: &mut BiquadNode) {
        unsafe {
            sys::ma_biquad_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }

    pub fn ma_biquad_node_reinit(
        config: *const sys::ma_biquad_config,
        node: &mut BiquadNode,
    ) -> Result<()> {
        let res = unsafe { sys::ma_biquad_node_reinit(config, node.to_raw()) };
        MaRawResult::resolve(res)
    }
}

impl<'a> Drop for BiquadNode<'a> {
    fn drop(&mut self) {
        n_biquad_ffi::ma_biquad_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub struct BiquadNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_biquad_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> Binding for BiquadNodeBuilder<'_, N> {
    type Raw = *const sys::ma_biquad_node_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

impl<'a, N: AsNodeGraphPtr + ?Sized> BiquadNodeBuilder<'a, N> {
    pub fn new(
        node_graph: &'a N,
        channels: u32,
        b0: f32,
        b1: f32,
        b2: f32,
        a0: f32,
        a1: f32,
        a2: f32,
    ) -> Self {
        let ptr = unsafe { sys::ma_biquad_node_config_init(channels, b0, b1, b2, a0, a1, a2) };
        Self {
            inner: ptr,
            node_graph,
        }
    }

    pub fn build(self) -> Result<BiquadNode<'a>> {
        BiquadNode::new_with_cfg_alloc_internal(self.node_graph, &self, None)
    }
}

pub struct BiquadNodeParams {
    inner: sys::ma_biquad_config,
}

impl Binding for BiquadNodeParams {
    type Raw = *const sys::ma_biquad_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

impl BiquadNodeParams {
    pub fn new(
        format: sys::ma_format,
        channels: u32,
        b0: f64,
        b1: f64,
        b2: f64,
        a0: f64,
        a1: f64,
        a2: f64,
    ) -> Self {
        let ptr = unsafe { sys::ma_biquad_config_init(format, channels, b0, b1, b2, a0, a1, a2) };
        Self { inner: ptr }
    }
}

#[cfg(test)]
mod test {
    use crate::engine::{Engine, EngineOps, node_graph::nodes::filters::biquad::BiquadNodeBuilder};

    #[test]
    fn test_biquad_builder_basic_init() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let _node = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1)
            .build()
            .unwrap();

        // let config = BiquadNodeParams::new(format, channels, b0, b1, b2, a0, a1, a2);
        // node.reinit(config);
    }
}
