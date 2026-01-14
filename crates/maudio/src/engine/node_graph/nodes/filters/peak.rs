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

/// A node that applies a **peaking EQ (bell filter)** to an audio signal.
///
/// A peaking EQ boosts or attenuates a **band of frequencies around a center frequency**,
/// while leaving frequencies far below/above that band mostly unchanged. This is commonly
/// used to:
/// - reduce resonances or “ringing” (cut at a problem frequency)
/// - add presence/clarity (boost around mids/high-mids)
/// - shape tone without affecting the entire low or high range
///
/// `PeakNode` is a node-graph wrapper around miniaudio's peak filter implementation.
/// It is designed for real-time use inside a [`NodeGraph`], and maintains internal filter state
/// across parameter changes.
///
/// ## Parameters
/// - **gain_db**: Amount of boost/cut in decibels at the center frequency.
///   Positive values boost, negative values cut.
/// - **frequency**: The center frequency (Hz) of the peak/bell.
/// - **q** (quality factor): Controls the width of the affected band.
///   Higher Q = narrower band (more surgical). Lower Q = wider band (more gentle).
///
/// ## Notes
/// After creating the filter, use [`Self::reinit`] and [`PeakNodeParams`] to update parameters.
/// This reinitializes the filter coefficients **without clearing internal state**, allowing
/// real-time parameter changes with minimal risk of audible artifacts (clicks/pops).
///
/// Use [`PeakNodeBuilder`] to initialize.
pub struct PeakNode<'a> {
    inner: *mut sys::ma_peak_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
    // Below is needed during a reinit
    channels: u32,
    // format is hard coded as ma_format_f32 in miniaudio `sys::ma_peak_node_config_init()`
    // but use value in inner.peak.format anyway inside new_with_cfg_alloc_internal()
    format: Format,
    sample_rate: SampleRate,
}

impl Binding for PeakNode<'_> {
    type Raw = *mut sys::ma_peak_node;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a> PeakNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &PeakNodeBuilder<'_, N>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> Result<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_peak_node>> = Box::new_uninit();

        n_peak_ffi::ma_peak_node_init(node_graph, config.to_raw(), alloc_cb, mem.as_mut_ptr())?;

        let ptr: Box<sys::ma_peak_node> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_peak_node = Box::into_raw(ptr);

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _marker: PhantomData,
            format: config.inner.peak.format.try_into().unwrap_or(Format::F32),
            channels: config.inner.peak.channels,
            sample_rate: config.inner.peak.sampleRate.try_into()?,
        })
    }

    /// See [`PeakNodeParams`] for creating a config
    pub fn reinit(&mut self, config: &PeakNodeParams) -> Result<()> {
        n_peak_ffi::ma_peak_node_reinit(config.to_raw(), self)
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

pub(crate) mod n_peak_ffi {
    use crate::{
        Binding, MaRawResult, Result,
        engine::node_graph::{AsNodeGraphPtr, nodes::filters::peak::PeakNode},
    };
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_peak_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_peak_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_peak_node,
    ) -> Result<()> {
        let res = unsafe {
            sys::ma_peak_node_init(node_graph.as_nodegraph_ptr(), config, alloc_cb, node)
        };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_peak_node_uninit(node: &mut PeakNode) {
        unsafe {
            sys::ma_peak_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }

    #[inline]
    pub fn ma_peak_node_reinit(
        config: *const sys::ma_peak_config,
        node: &mut PeakNode,
    ) -> Result<()> {
        let res = unsafe { sys::ma_peak_node_reinit(config, node.to_raw()) };
        MaRawResult::resolve(res)
    }
}

impl<'a> Drop for PeakNode<'a> {
    fn drop(&mut self) {
        n_peak_ffi::ma_peak_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}
pub struct PeakNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_peak_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> Binding for PeakNodeBuilder<'_, N> {
    type Raw = *const sys::ma_peak_node_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

impl<'a, N: AsNodeGraphPtr + ?Sized> PeakNodeBuilder<'a, N> {
    pub fn new(
        node_graph: &'a N,
        channels: u32,
        sample_rate: SampleRate,
        gain_db: f64,
        q: f64,
        frequency: f64,
    ) -> Self {
        let ptr = unsafe {
            sys::ma_peak_node_config_init(channels, sample_rate.into(), gain_db, q, frequency)
        };
        Self {
            inner: ptr,
            node_graph,
        }
    }

    pub fn build(self) -> Result<PeakNode<'a>> {
        PeakNode::new_with_cfg_alloc_internal(self.node_graph, &self, None)
    }
}

pub struct PeakNodeParams {
    inner: sys::ma_peak_config,
}

impl Binding for PeakNodeParams {
    type Raw = *const sys::ma_peak_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

impl PeakNodeParams {
    pub fn new(node: &PeakNode, gain_db: f64, quality_factor: f64, frequency: f64) -> Self {
        let ptr = unsafe {
            sys::ma_peak2_config_init(
                node.format.into(),
                node.channels,
                node.sample_rate.into(),
                gain_db,
                quality_factor,
                frequency,
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
            node_graph::nodes::filters::peak::{PeakNodeBuilder, PeakNodeParams},
        },
    };

    #[test]
    fn test_peak_builder_basic_init() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = PeakNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 2.0, 1.1, 2000.0)
            .build()
            .unwrap();
        let config = PeakNodeParams::new(&node, 2.0, 1.0, 2000.0);
        node.reinit(&config).unwrap();
    }

    #[test]
    fn test_peak_multiple_reinit_updates() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = PeakNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 0.0, 1.0, 1000.0)
            .build()
            .unwrap();

        // Sweep parameters to ensure repeated reinit works and doesn't error.
        for i in 0..50 {
            let t = i as f64;

            // gain in [-12, +12]
            let gain_db = (t - 25.0) * (12.0 / 25.0);

            // Q in [0.5, 5.0]
            let q = 0.5 + (t / 49.0) * 4.5;

            // freq in [50, 12000]
            let freq = 50.0 + (t / 49.0) * (12_000.0 - 50.0);

            let cfg = PeakNodeParams::new(&node, gain_db, q, freq);
            node.reinit(&cfg).unwrap();
        }
    }

    #[test]
    fn test_peak_reinit_extreme_but_plausible_values() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            PeakNodeBuilder::new(&node_graph, 2, SampleRate::Sr48000, -6.0, 0.707, 120.0)
                .build()
                .unwrap();

        // Very low freq (still > 0)
        let cfg_low = PeakNodeParams::new(&node, -3.0, 0.5, 10.0);
        node.reinit(&cfg_low).unwrap();

        // Near-Nyquist-ish (leave headroom; Nyquist is 24000 at 48k)
        let cfg_high = PeakNodeParams::new(&node, 3.0, 2.0, 18_000.0);
        node.reinit(&cfg_high).unwrap();

        // Higher Q
        let cfg_q = PeakNodeParams::new(&node, 0.0, 10.0, 1000.0);
        node.reinit(&cfg_q).unwrap();
    }

    #[test]
    fn test_peak_builder_multi_channel_init() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        // Just validate that multi-channel init + reinit works.
        let mut node = PeakNodeBuilder::new(&node_graph, 8, SampleRate::Sr44100, 1.5, 1.0, 4000.0)
            .build()
            .unwrap();

        let cfg = PeakNodeParams::new(&node, 1.5, 1.0, 4000.0);
        node.reinit(&cfg).unwrap();
    }
}
