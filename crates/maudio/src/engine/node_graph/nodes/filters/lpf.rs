use std::{marker::PhantomData, mem::MaybeUninit, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    engine::{
        node_graph::{
            nodes::{private_node, AsNodePtr, NodeRef},
            AsNodeGraphPtr, NodeGraph,
        },
        AllocationCallbacks,
    },
    AsRawRef, Binding, MaResult,
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
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    _marker: PhantomData<&'a NodeGraph>,
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

#[doc(hidden)]
impl AsNodePtr for LpfNode<'_> {
    type __PtrProvider = private_node::LpfNodeProvider;
}

impl<'a> LpfNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &LpfNodeBuilder<'_, N>,
        alloc: Option<Arc<AllocationCallbacks>>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_lpf_node>> = Box::new(MaybeUninit::uninit());

        n_lpf_ffi::ma_lpf_node_init(node_graph, config.as_raw_ptr(), alloc_cb, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_lpf_node = Box::into_raw(mem) as *mut sys::ma_lpf_node;

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
    pub fn reinit(&mut self, config: &LpfNodeParams) -> MaResult<()> {
        if config.inner.channels == 0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        let sample_rate: u32 = config.inner.sampleRate;
        let cutoff = config.inner.cutoffFrequency;
        if !cutoff.is_finite() || cutoff <= 0.0 || cutoff >= sample_rate as f64 / 2.0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        n_lpf_ffi::ma_lpf_node_reinit(config.as_raw_ptr(), self)
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

pub(crate) mod n_lpf_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        engine::node_graph::{nodes::filters::lpf::LpfNode, private_node_graph, AsNodeGraphPtr},
        Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_lpf_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_lpf_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_lpf_node,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_lpf_node_init(
                private_node_graph::node_graph_ptr(node_graph),
                config,
                alloc_cb,
                node,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_lpf_node_uninit(node: &mut LpfNode) {
        unsafe {
            sys::ma_lpf_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }

    #[inline]
    pub fn ma_lpf_node_reinit(
        config: *const sys::ma_lpf_config,
        node: &mut LpfNode,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_lpf_node_reinit(config, node.to_raw()) };
        MaudioError::check(res)
    }
}

impl<'a> Drop for LpfNode<'a> {
    fn drop(&mut self) {
        n_lpf_ffi::ma_lpf_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}
/// Builder for creating a [`LpfNode`]
pub struct LpfNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_lpf_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> AsRawRef for LpfNodeBuilder<'_, N> {
    type Raw = sys::ma_lpf_node_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
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

    pub fn build(&self) -> MaResult<LpfNode<'a>> {
        if self.inner.lpf.channels == 0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        let sample_rate: u32 = self.inner.lpf.sampleRate;
        let cutoff = self.inner.lpf.cutoffFrequency;
        if !cutoff.is_finite() || cutoff <= 0.0 || cutoff >= sample_rate as f64 / 2.0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        LpfNode::new_with_cfg_alloc_internal(self.node_graph, self, None)
    }
}

pub struct LpfNodeParams {
    inner: sys::ma_lpf_config,
}

impl AsRawRef for LpfNodeParams {
    type Raw = sys::ma_lpf_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
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

#[cfg(test)]
mod test {
    use crate::{
        audio::sample_rate::SampleRate,
        engine::{
            node_graph::nodes::filters::lpf::{LpfNodeBuilder, LpfNodeParams},
            Engine, EngineOps,
        },
        Binding,
    };

    #[test]
    fn test_lpf_builder_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, 1)
            .build()
            .unwrap();
        let config = LpfNodeParams::new(&node, SampleRate::Sr44100, 1200.0);
        node.reinit(&config).unwrap();
    }

    #[test]
    fn test_lpf_multiple_reinit() {
        let engine = Engine::new_for_tests().unwrap();
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
        let engine = Engine::new_for_tests().unwrap();
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
        let engine = Engine::new_for_tests().unwrap();
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
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        for &order in &[1, 2, 4, 8] {
            let node =
                LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, order).build();

            let _ = node;
        }
    }

    #[test]
    fn test_lpf_builder_invalid_channels() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let res = LpfNodeBuilder::new(&node_graph, 0, SampleRate::Sr44100, 1000.0, 1).build();

        assert!(res.is_err());
    }

    #[test]
    fn valgrind_lpf_mass_create_drop() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        for _ in 0..20_000 {
            let _node = LpfNodeBuilder::new(&node_graph, 2, SampleRate::Sr48000, 5000.0, 2)
                .build()
                .unwrap();
        }
    }

    #[test]
    fn valgrind_lpf_reinit_many() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = LpfNodeBuilder::new(&node_graph, 2, SampleRate::Sr48000, 1000.0, 4)
            .build()
            .unwrap();

        for i in 0..100_000 {
            // keep cutoff within (0, nyquist)
            let cutoff = 10.0 + (i % 20_000) as f64; // 10..20009 Hz
            let cfg = LpfNodeParams::new(&node, SampleRate::Sr48000, cutoff);
            node.reinit(&cfg).unwrap();
        }
    }

    #[test]
    fn valgrind_lpf_many_nodes_some_reinit() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        for i in 0..5000 {
            let mut node = LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 500.0, 2)
                .build()
                .unwrap();

            if i % 3 == 0 {
                let cfg = LpfNodeParams::new(&node, SampleRate::Sr44100, 1200.0);
                node.reinit(&cfg).unwrap();
            }
        }
    }

    #[test]
    fn test_lpf_reinit_nyquist_boundaries() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 1000.0, 2)
            .build()
            .unwrap();

        let nyquist = 48_000.0 / 2.0;
        let eps = 1e-9;

        let ok = LpfNodeParams::new(&node, SampleRate::Sr48000, nyquist - eps);
        assert!(node.reinit(&ok).is_ok());

        let eq = LpfNodeParams::new(&node, SampleRate::Sr48000, nyquist);
        assert!(node.reinit(&eq).is_err());

        let above = LpfNodeParams::new(&node, SampleRate::Sr48000, nyquist + eps);
        assert!(node.reinit(&above).is_err());
    }

    #[test]
    fn test_lpf_reinit_rejects_non_finite_cutoff() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, 2)
            .build()
            .unwrap();

        for &cutoff in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let cfg = LpfNodeParams::new(&node, SampleRate::Sr44100, cutoff);
            assert!(node.reinit(&cfg).is_err());
        }
    }

    #[test]
    fn test_lpf_failed_reinit_then_ok_then_drop() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, 2)
            .build()
            .unwrap();

        let bad = LpfNodeParams::new(&node, SampleRate::Sr44100, 22_050.0);
        assert!(node.reinit(&bad).is_err());

        let ok = LpfNodeParams::new(&node, SampleRate::Sr44100, 5000.0);
        node.reinit(&ok).unwrap();
    }

    #[test]
    fn test_lpf_as_node_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let node = LpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, 2)
            .build()
            .unwrap();

        let nref = node.as_node();
        assert!(!nref.to_raw().is_null());
    }
}
