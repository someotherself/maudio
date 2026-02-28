use std::{marker::PhantomData, mem::MaybeUninit, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    engine::{
        node_graph::{
            nodes::{private_node::HiShelfNodeProvider, AsNodePtr, NodeRef},
            AsNodeGraphPtr, NodeGraph,
        },
        AllocationCallbacks,
    },
    AsRawRef, Binding, MaResult,
};

/// A node that applies a **high-shelf EQ** to an audio signal.
///
/// A high-shelf boosts or attenuates frequencies **above** a cutoff frequency,
/// leaving lower frequencies mostly unchanged. This is commonly used to add
/// "brightness" (boost) or "darkness" (cut) to a signal.
///
/// `HiShelfNode` is a node-graph wrapper around miniaudio's hi-shelf filter implementation.
/// It is designed for real-time use inside a [`NodeGraph`], and maintains internal filter state
/// across parameter changes.
///
/// ## Parameters
/// - **gain_db**: Amount of boost/cut in decibels. Positive boosts highs; negative reduces.
/// - **frequency**: The cutoff frequency (Hz) where the shelf begins transitioning.
/// - **shelf_slope**: Controls how sharp/gradual the shelf transition is.
///   (In many EQs this is related to Q; miniaudio exposes it as a shelf slope.)
///
/// ## Notes
/// After creating the filter, use [`Self::reinit`] and [`HiShelfNodeParams`] to change the values of the coefficients.
/// This reinitializes the filter coefficients without clearing the internal state.
/// This allows filter parameters to be updated in real time without causing
/// audible artifacts such as clicks or pops.
///
/// Use [`HiShelfNodeBuilder`] to initialize
pub struct HiShelfNode<'a> {
    inner: *mut sys::ma_hishelf_node,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    _marker: PhantomData<&'a NodeGraph>,
    // Below is needed during a reinit
    channels: u32,
    // format is hard coded as ma_format_f32 in miniaudio `sys::ma_hishelf_node_config_init()`
    // but use value in inner.hishelf.format anyway inside new_with_cfg_alloc_internal()
    format: Format,
}

impl Binding for HiShelfNode<'_> {
    type Raw = *mut sys::ma_hishelf_node;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[doc(hidden)]
impl AsNodePtr for HiShelfNode<'_> {
    type __PtrProvider = HiShelfNodeProvider;
}

impl<'a> HiShelfNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &HiShelfNodeBuilder<N>,
        alloc: Option<Arc<AllocationCallbacks>>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_hishelf_node>> =
            Box::new(MaybeUninit::uninit());

        n_hishelf_ffi::ma_hishelf_node_init(
            node_graph,
            config.as_raw_ptr(),
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_hishelf_node = Box::into_raw(mem) as *mut sys::ma_hishelf_node;

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _marker: PhantomData,
            channels: config.inner.hishelf.channels,
            format: config
                .inner
                .hishelf
                .format
                .try_into()
                .unwrap_or(Format::F32),
        })
    }

    /// See [`HiShelfNodeParams`] for creating a config
    pub fn reinit(&mut self, config: &HiShelfNodeParams) -> MaResult<()> {
        if !config.inner.frequency.is_finite() || config.inner.frequency <= 0.0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        if !config.inner.shelfSlope.is_finite() || config.inner.shelfSlope <= 0.0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        n_hishelf_ffi::ma_hishelf_node_reinit(config.as_raw_ptr(), self)
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

pub(crate) mod n_hishelf_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        engine::node_graph::{
            nodes::filters::hishelf::HiShelfNode, private_node_graph, AsNodeGraphPtr,
        },
        Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_hishelf_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_hishelf_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_hishelf_node,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_hishelf_node_init(
                private_node_graph::node_graph_ptr(node_graph),
                config,
                alloc_cb,
                node,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_hishelf_node_uninit(node: &mut HiShelfNode) {
        unsafe {
            sys::ma_hishelf_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }

    #[inline]
    pub fn ma_hishelf_node_reinit(
        config: *const sys::ma_hishelf_config,
        node: &mut HiShelfNode,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_hishelf_node_reinit(config, node.to_raw()) };
        MaudioError::check(res)
    }
}

impl<'a> Drop for HiShelfNode<'a> {
    fn drop(&mut self) {
        n_hishelf_ffi::ma_hishelf_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

/// Builder for creating a [`HiShelfNode`]
pub struct HiShelfNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_hishelf_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> AsRawRef for HiShelfNodeBuilder<'_, N> {
    type Raw = sys::ma_hishelf_node_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<'a, N: AsNodeGraphPtr + ?Sized> HiShelfNodeBuilder<'a, N> {
    pub fn new(
        node_graph: &'a N,
        channels: u32,
        sample_rate: SampleRate,
        gain_db: f64,
        shelf_slope: f64,
        frequency: f64,
    ) -> Self {
        let ptr = unsafe {
            sys::ma_hishelf_node_config_init(
                channels,
                sample_rate.into(),
                gain_db,
                shelf_slope,
                frequency,
            )
        };
        HiShelfNodeBuilder {
            inner: ptr,
            node_graph,
        }
    }

    pub fn build(&self) -> MaResult<HiShelfNode<'a>> {
        if self.inner.hishelf.channels == 0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        if !self.inner.hishelf.frequency.is_finite() || self.inner.hishelf.frequency <= 0.0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        if !self.inner.hishelf.shelfSlope.is_finite() || self.inner.hishelf.shelfSlope <= 0.0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        HiShelfNode::new_with_cfg_alloc_internal(self.node_graph, self, None)
    }
}

pub struct HiShelfNodeParams {
    inner: sys::ma_hishelf_config,
}

impl AsRawRef for HiShelfNodeParams {
    type Raw = sys::ma_hishelf_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl HiShelfNodeParams {
    pub fn new(
        node: &HiShelfNode,
        sample_rate: SampleRate,
        gain_db: f64,
        shelf_slope: f64,
        frequency: f64,
    ) -> Self {
        let ptr = unsafe {
            sys::ma_hishelf2_config_init(
                node.format.into(),
                node.channels,
                sample_rate.into(),
                gain_db,
                shelf_slope,
                frequency,
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
            node_graph::nodes::filters::hishelf::{HiShelfNodeBuilder, HiShelfNodeParams},
            Engine, EngineOps,
        },
    };

    #[test]
    fn test_hishelf_builder_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            HiShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.1, 1000.0)
                .build()
                .unwrap();

        let config = HiShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.1, 1200.0);
        node.reinit(&config).unwrap();
    }

    #[test]
    fn test_hishelf_reinit_sample_rate_change_ok() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            HiShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 2000.0)
                .build()
                .unwrap();

        let cfg = HiShelfNodeParams::new(&node, SampleRate::Sr44100, 0.0, 0.5, 2000.0);
        node.reinit(&cfg).unwrap();
    }

    #[test]
    fn test_hishelf_reinit_invalid_frequency_errors() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            HiShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 2000.0)
                .build()
                .unwrap();

        let cfg = HiShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.5, 0.0);
        assert!(node.reinit(&cfg).is_err());
    }

    #[test]
    fn test_hishelf_reinit_invalid_slope_errors() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            HiShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 2000.0)
                .build()
                .unwrap();

        // Slope 0 or negative should error.
        let cfg = HiShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.0, 2000.0);
        assert!(node.reinit(&cfg).is_err());
    }

    #[test]
    fn test_hishelf_builder_channels_zero_is_err() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let res =
            HiShelfNodeBuilder::new(&node_graph, 0, SampleRate::Sr48000, 0.0, 0.5, 2000.0).build();
        assert!(res.is_err());
    }

    #[test]
    fn test_hishelf_as_node_is_non_null() {
        use crate::engine::node_graph::nodes::private_node;

        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let node = HiShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 2000.0)
            .build()
            .unwrap();

        let node_ref = node.as_node();
        assert!(!private_node::node_ptr(&node_ref).is_null());
    }

    #[test]
    fn test_hishelf_create_drop_many_times() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        for _ in 0..2_000 {
            let _node =
                HiShelfNodeBuilder::new(&node_graph, 2, SampleRate::Sr48000, 3.0, 0.7, 6000.0)
                    .build()
                    .unwrap();
        }
    }

    #[test]
    fn test_hishelf_reinit_stress_many_iterations() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            HiShelfNodeBuilder::new(&node_graph, 2, SampleRate::Sr48000, 0.0, 0.7, 4000.0)
                .build()
                .unwrap();

        for i in 0..10_000 {
            let gain = ((i % 200) as f64) * 0.1 - 10.0; // [-10dB, +10dB]
            let freq = 1000.0 + ((i % 3000) as f64); // 1k..4k
            let slope = 0.1 + ((i % 90) as f64) * 0.01; // 0.1..1.0

            let cfg = HiShelfNodeParams::new(&node, SampleRate::Sr48000, gain, slope, freq);
            let _ = node.reinit(&cfg);
        }
    }

    #[test]
    fn test_hishelf_drop_before_engine_is_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let node = HiShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.7, 4000.0)
            .build()
            .unwrap();

        drop(node);
        drop(engine);
    }

    #[test]
    fn test_hishelf_builder_nan_inputs_no_panic() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let _ = HiShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, f64::NAN, 0.5, 2000.0)
            .build();
        let _ = HiShelfNodeBuilder::new(
            &node_graph,
            1,
            SampleRate::Sr48000,
            0.0,
            f64::INFINITY,
            2000.0,
        )
        .build();
        let _ = HiShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, f64::NAN)
            .build();
    }

    #[test]
    fn test_hishelf_reinit_nan_inputs_errors_no_panic() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            HiShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 2000.0)
                .build()
                .unwrap();

        let cfg = HiShelfNodeParams::new(&node, SampleRate::Sr48000, f64::NAN, 0.5, 2000.0);
        let _ = node.reinit(&cfg); // no panic

        let cfg = HiShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, f64::INFINITY, 2000.0);
        let res = node.reinit(&cfg);
        assert!(res.is_err());

        let cfg = HiShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.5, f64::NAN);
        let res = node.reinit(&cfg);
        assert!(res.is_err());
    }
}
