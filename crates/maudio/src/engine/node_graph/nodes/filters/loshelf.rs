use std::{marker::PhantomData, mem::MaybeUninit, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    engine::{
        node_graph::{
            nodes::{private_node::LoShelfNodeProvider, AsNodePtr, NodeRef},
            AsNodeGraphPtr, NodeGraph,
        },
        AllocationCallbacks,
    },
    AsRawRef, Binding, MaResult,
};

/// A node that applies a **low-shelf EQ** to an audio signal.
///
/// A low-shelf boosts or attenuates frequencies **below** a cutoff frequency,
/// leaving higher frequencies mostly unchanged. This is commonly used to add
/// "warmth"/"bass" (boost) or reduce "boom"/"rumble" (cut) in a signal.
///
/// `LoShelfNode` is a node-graph wrapper around miniaudio's lo-shelf filter implementation.
/// It is designed for real-time use inside a [`NodeGraph`], and maintains internal filter state
/// across parameter changes.
///
/// ## Parameters
/// - **gain_db**: Amount of boost/cut in decibels. Positive boosts lows; negative reduces.
/// - **frequency**: The cutoff frequency (Hz) where the shelf begins transitioning.
/// - **shelf_slope**: Controls how sharp/gradual the shelf transition is.
///   (In many EQs this is related to Q; miniaudio exposes it as a shelf slope.)
///
/// ## Notes
/// After creating the filter, use [`Self::reinit()`] and [`LoShelfNodeParams`] to change the values of the coefficients.
/// This reinitializes the filter coefficients without clearing the internal state.
/// This allows filter parameters to be updated in real time without causing
/// audible artifacts such as clicks or pops.
///
/// Use [`LoShelfNodeBuilder`] to initialize
pub struct LoShelfNode<'a> {
    inner: *mut sys::ma_loshelf_node,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    _marker: PhantomData<&'a NodeGraph>,
    // Below is needed during a reinit
    channels: u32,
    // format is hard coded as ma_format_f32 in miniaudio `sys::ma_loshelf_node_config_init()`
    // but use value in inner.loshelf.format anyway inside new_with_cfg_alloc_internal()
    format: Format,
}

impl Binding for LoShelfNode<'_> {
    type Raw = *mut sys::ma_loshelf_node;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[doc(hidden)]
impl AsNodePtr for LoShelfNode<'_> {
    type __PtrProvider = LoShelfNodeProvider;
}

impl<'a> LoShelfNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &LoShelfNodeBuilder<N>,
        alloc: Option<Arc<AllocationCallbacks>>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_loshelf_node>> =
            Box::new(MaybeUninit::uninit());

        n_loshelf_ffi::ma_loshelf_node_init(
            node_graph,
            config.as_raw_ptr(),
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_loshelf_node = Box::into_raw(mem) as *mut sys::ma_loshelf_node;

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _marker: PhantomData,
            channels: config.inner.loshelf.channels,
            format: config
                .inner
                .loshelf
                .format
                .try_into()
                .unwrap_or(Format::F32),
        })
    }

    /// See [`LoShelfNodeParams`] for creating a config
    pub fn reinit(&mut self, config: &LoShelfNodeParams) -> MaResult<()> {
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

        n_loshelf_ffi::ma_loshelf_node_reinit(config.as_raw_ptr(), self)
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

pub(crate) mod n_loshelf_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        engine::node_graph::{
            nodes::filters::loshelf::LoShelfNode, private_node_graph, AsNodeGraphPtr,
        },
        Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_loshelf_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_loshelf_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_loshelf_node,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_loshelf_node_init(
                private_node_graph::node_graph_ptr(node_graph),
                config,
                alloc_cb,
                node,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_loshelf_node_uninit(node: &mut LoShelfNode) {
        unsafe {
            sys::ma_loshelf_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }

    #[inline]
    pub fn ma_loshelf_node_reinit(
        config: *const sys::ma_loshelf_config,
        node: &mut LoShelfNode,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_loshelf_node_reinit(config, node.to_raw()) };
        MaudioError::check(res)
    }
}

impl<'a> Drop for LoShelfNode<'a> {
    fn drop(&mut self) {
        n_loshelf_ffi::ma_loshelf_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

/// Builder for creating a [`LoShelfNode`]
pub struct LoShelfNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_loshelf_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> AsRawRef for LoShelfNodeBuilder<'_, N> {
    type Raw = sys::ma_loshelf_node_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<'a, N: AsNodeGraphPtr + ?Sized> LoShelfNodeBuilder<'a, N> {
    pub fn new(
        node_graph: &'a N,
        channels: u32,
        sample_rate: SampleRate,
        gain_db: f64,
        shelf_slope: f64,
        frequency: f64,
    ) -> Self {
        let ptr = unsafe {
            sys::ma_loshelf_node_config_init(
                channels,
                sample_rate.into(),
                gain_db,
                shelf_slope,
                frequency,
            )
        };
        Self {
            inner: ptr,
            node_graph,
        }
    }

    pub fn build(&self) -> MaResult<LoShelfNode<'a>> {
        if self.inner.loshelf.channels == 0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        if !self.inner.loshelf.frequency.is_finite() || self.inner.loshelf.frequency <= 0.0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        LoShelfNode::new_with_cfg_alloc_internal(self.node_graph, self, None)
    }
}

pub struct LoShelfNodeParams {
    inner: sys::ma_loshelf_config,
}

impl AsRawRef for LoShelfNodeParams {
    type Raw = sys::ma_loshelf_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl LoShelfNodeParams {
    pub fn new(
        node: &LoShelfNode,
        sample_rate: SampleRate,
        gain_db: f64,
        shelf_slope: f64,
        frequency: f64,
    ) -> Self {
        let ptr = unsafe {
            sys::ma_loshelf2_config_init(
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
            node_graph::nodes::filters::loshelf::{LoShelfNodeBuilder, LoShelfNodeParams},
            Engine, EngineOps,
        },
        Binding,
    };

    #[test]
    fn test_loshelf_builder_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.1, 1000.0)
                .build()
                .unwrap();

        let config = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.1, 1200.0);
        node.reinit(&config).unwrap();
    }

    #[test]
    fn test_loshelf_reinit_sample_rate_change_ok() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        // Change sample rate: should be OK.
        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr44100, 0.0, 0.5, 200.0);
        node.reinit(&cfg).unwrap();
    }

    #[test]
    fn test_loshelf_reinit_invalid_frequency_errors() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.5, 0.0);
        assert!(node.reinit(&cfg).is_err());
    }

    #[test]
    fn test_loshelf_reinit_invalid_slope_errors() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.0, 200.0);
        assert!(node.reinit(&cfg).is_err());
    }

    #[test]
    fn test_loshelf_reinit_extreme_gain_ok() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 120.0, 0.5, 200.0);
        assert!(node.reinit(&cfg).is_ok());

        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, -120.0, 0.5, 200.0);
        assert!(node.reinit(&cfg).is_ok());
    }

    #[test]
    fn test_loshelf_channels_zero_error() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let res =
            LoShelfNodeBuilder::new(&node_graph, 0, SampleRate::Sr48000, 0.0, 0.5, 200.0).build();
        assert!(res.is_err());
    }

    #[test]
    fn valgrind_loshelf_create_drop_many() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        for _ in 0..10_000 {
            let _node =
                LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                    .build()
                    .unwrap();
        }
    }

    #[test]
    fn valgrind_loshelf_reinit_stress() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 2, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        for i in 0..50_000 {
            let freq = 20.0 + ((i % 10_000) as f64) * 0.1;
            let gain = ((i % 240) as f64) - 120.0; // [-120, 119]
            let slope = 0.1 + ((i % 100) as f64) * 0.01; // (0.1..1.09)

            let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, gain, slope, freq);
            node.reinit(&cfg).unwrap();
        }
    }

    #[test]
    fn test_loshelf_failed_reinit_then_drop_is_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.5, 0.0);
        assert!(node.reinit(&cfg).is_err());

        let cfg2 = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 3.0, 0.8, 250.0);
        node.reinit(&cfg2).unwrap();
    }

    #[test]
    fn test_loshelf_as_node_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let node = LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
            .build()
            .unwrap();

        let nref = node.as_node();
        assert!(!nref.to_raw().is_null());
    }

    #[test]
    fn test_loshelf_reinit_negative_frequency_ok_or_error() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.5, -1.0);

        assert!(node.reinit(&cfg).is_err());
    }

    #[test]
    fn test_loshelf_reinit_nan_rejected() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.5, f64::NAN);
        assert!(node.reinit(&cfg).is_err());
    }
}
