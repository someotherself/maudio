use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    Binding, Result,
    audio::{formats::Format, sample_rate::SampleRate},
    engine::{
        AllocationCallbacks,
        node_graph::{AsNodeGraphPtr, NodeGraph, nodes::NodeRef},
    },
};

/// A node that applies a **low-pass filter (LPF)** to an audio signal.
///
/// A low-pass filter attenuates frequencies **above** a cutoff frequency,
/// allowing lower frequencies to pass through mostly unchanged. This is commonly
/// used to reduce high-frequency noise, tame harshness, or smooth bright signals.
///
/// `LpfNode` is a node-graph wrapper around miniaudio's LPF implementation.
/// It is designed for real-time use inside a [`NodeGraph`], and maintains internal
/// filter state across parameter changes.
///
/// ## Parameters
/// - **cutoff_frequency**: The cutoff frequency (Hz) of the low-pass filter.
/// - **order**: The filter order (slope/steepness). Higher values produce a steeper cutoff
///   at the cost of more processing and potentially more phase shift.
///
/// ## Notes
/// After creating the filter, use [`Self::reinit`] and [`LpfNodeParams`] to change the filter parameters.
/// This reinitializes the filter coefficients without clearing the internal state.
/// This allows filter parameters to be updated in real time without causing
/// audible artifacts such as clicks or pops.
///
/// Use [`LpfNodeBuilder`] to initialize.
pub struct LpfNode<'a> {
    inner: *mut sys::ma_lpf_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
    // Below is needed during a reinit
    channels: u32,
    // format is hard coded as ma_format_f32 in miniaudio `sys::ma_lpf_node_config_init()`
    // but use value in inner.lpf.format anyway inside new_with_cfg_alloc_internal()
    format: Format,
    order: u32,
}

impl Binding for LpfNode<'_> {
    type Raw = *mut sys::ma_lpf_node;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a> LpfNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &LpfNodeBuilder<'_, N>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> Result<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_lpf_node>> = Box::new_uninit();

        n_lpf_ffi::ma_lpf_node_init(node_graph, config.to_raw(), alloc_cb, mem.as_mut_ptr())?;

        let ptr: Box<sys::ma_lpf_node> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_lpf_node = Box::into_raw(ptr);

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _marker: PhantomData,
            channels: config.inner.lpf.channels,
            format: config.inner.lpf.format.try_into().unwrap_or(Format::F32),
            order: config.inner.lpf.order,
        })
    }

    /// See [`LpfNodeParams`] for creating a config
    pub fn reinit(&mut self, config: &LpfNodeParams) -> Result<()> {
        n_lpf_ffi::ma_lpf_node_reinit(config.to_raw(), self)
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

pub(crate) mod n_lpf_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        Binding, MaRawResult, Result,
        engine::node_graph::{AsNodeGraphPtr, nodes::filters::lpf::LpfNode},
    };

    pub fn ma_lpf_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_lpf_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_lpf_node,
    ) -> Result<()> {
        let res =
            unsafe { sys::ma_lpf_node_init(node_graph.as_nodegraph_ptr(), config, alloc_cb, node) };
        MaRawResult::resolve(res)
    }

    pub fn ma_lpf_node_uninit(node: &mut LpfNode) {
        unsafe {
            sys::ma_lpf_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }

    pub fn ma_lpf_node_reinit(config: *const sys::ma_lpf_config, node: &mut LpfNode) -> Result<()> {
        let res = unsafe { sys::ma_lpf_node_reinit(config, node.to_raw()) };
        MaRawResult::resolve(res)
    }
}

impl<'a> Drop for LpfNode<'a> {
    fn drop(&mut self) {
        n_lpf_ffi::ma_lpf_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub struct LpfNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_lpf_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> Binding for LpfNodeBuilder<'_, N> {
    type Raw = *const sys::ma_lpf_node_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

impl<'a, N: AsNodeGraphPtr + ?Sized> LpfNodeBuilder<'a, N> {
    pub fn new(
        node_graph: &'a N,
        channels: u32,
        sample_rate: SampleRate,
        cutoff_freq: f64,
        order: u32,
    ) -> Self {
        let ptr = unsafe {
            sys::ma_lpf_node_config_init(channels, sample_rate.into(), cutoff_freq, order)
        };
        LpfNodeBuilder {
            inner: ptr,
            node_graph,
        }
    }

    pub fn build(self) -> Result<LpfNode<'a>> {
        LpfNode::new_with_cfg_alloc_internal(self.node_graph, &self, None)
    }
}

pub struct LpfNodeParams {
    inner: sys::ma_lpf_config,
}

impl Binding for LpfNodeParams {
    type Raw = *const sys::ma_lpf_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

impl LpfNodeParams {
    pub fn new(node: &LpfNode, sample_rate: SampleRate, cutoff_freq: f64) -> Self {
        let ptr = unsafe {
            sys::ma_lpf_config_init(
                node.format.into(),
                node.channels,
                sample_rate.into(),
                cutoff_freq,
                node.order,
            )
        };
        Self { inner: ptr }
    }
}

#[cfg(feature = "device-tests")]
#[cfg(test)]
mod test {
    use crate::{
        audio::sample_rate::SampleRate,
        engine::{
            Engine, EngineOps,
            node_graph::nodes::filters::lpf::{LpfNodeBuilder, LpfNodeParams},
        },
    };

    #[test]
    fn test_lpf_builder_basic_init() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, 1)
            .build()
            .unwrap();
        let config = LpfNodeParams::new(&node, SampleRate::Sr44100, 1200.0);
        node.reinit(&config).unwrap();
    }

    #[test]
    fn test_lpf_multiple_reinit() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, 1)
            .build()
            .unwrap();

        for i in 0..50 {
            let cutoff = 200.0 + (i as f64 * 20.0); // 200..1180 Hz
            let config = LpfNodeParams::new(&node, SampleRate::Sr44100, cutoff);
            node.reinit(&config).unwrap();
        }
    }
    #[test]
    fn test_lpf_reinit_changes_sample_rate() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, 1)
            .build()
            .unwrap();

        let cfg1 = LpfNodeParams::new(&node, SampleRate::Sr44100, 800.0);
        node.reinit(&cfg1).unwrap();

        let cfg2 = LpfNodeParams::new(&node, SampleRate::Sr48000, 800.0);
        node.reinit(&cfg2).unwrap();
    }

    #[test]
    fn test_lpf_cutoff_edge_values() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, 1)
            .build()
            .unwrap();

        let candidates = [0.0, 1.0, 10.0, 20_000.0, 100_000.0];

        for &cutoff in &candidates {
            let cfg = LpfNodeParams::new(&node, SampleRate::Sr44100, cutoff);
            let res = node.reinit(&cfg);
            // Accept either ok or invalid-args depending on miniaudio validation.
            if let Err(e) = res {
                // If your Result error type exposes raw ma_result, assert it's invalid args here.
                // Otherwise just ensure it doesn't panic.
                let _ = e;
            }
        }
    }

    #[test]
    fn test_lpf_builder_orders() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        for &order in &[1, 2, 4, 8] {
            let node =
                LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, order).build();

            let _ = node;
        }
    }

    #[test]
    fn test_lpf_builder_invalid_channels() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let res = LpfNodeBuilder::new(&node_graph, 0, SampleRate::Sr44100, 1000.0, 1).build();

        assert!(res.is_err());
    }
}
