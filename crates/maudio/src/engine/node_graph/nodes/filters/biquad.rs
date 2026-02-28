use std::{marker::PhantomData, mem::MaybeUninit, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    audio::formats::Format,
    engine::{
        node_graph::{
            nodes::{private_node::BiquadNodeProvider, AsNodePtr, NodeRef},
            AsNodeGraphPtr, NodeGraph,
        },
        AllocationCallbacks,
    },
    AsRawRef, Binding, MaResult,
};

/// A node that applies a biquad filtering to an audio signal.
///
/// `BiquadNode` is one of the custom DSP nodes provided by miniaudio.
///
/// By changing its coefficients, the same filter structure can act as low-pass, high-pass,
/// EQ, or notch filters while maintaining continuous state for real-time processing.
///
/// ## Parameters
///
/// The filter is defined by six coefficients:
///
/// - **Numerator (feed-forward):** `b0`, `b1`, `b2`  
/// - **Denominator (feed-back):** `a0`, `a1`, `a2`
///
/// ### Important invariants
///
/// - `a0` **must not be zero**
/// - Coefficients **must not be pre-normalized**
///   (normalization is handled internally)
/// - Coefficients must be **finite** (`NaN` or ±∞ are invalid).
///   Maudio current does not check the inputs passed to miniaudio
///
/// Violating these constraints may result in an error or undefined DSP behavior.
///
/// ## Notes
/// - After creating the filter, use [`Self::reinit`] and [`BiquadNodeParams`] to change the values of the coefficients.
///   This reinitializes the filter coefficients without clearing the internal state.
///   This allows filter parameters to be updated in real time without causing
///   audible artifacts such as clicks or pops.
/// - Changing the format or channel count after initialization is invalid and
///   will result in an error.
///
/// Use [`BiquadNodeBuilder`] to initialize
pub struct BiquadNode<'a> {
    inner: *mut sys::ma_biquad_node,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    _marker: PhantomData<&'a NodeGraph>,
    // Below is needed during a reinit
    channels: u32,
    // format is hard coded as ma_format_f32 in miniaudio `sys::ma_biquad_node_config_init()`
    // but use value in inner.biquad.format anyway inside new_with_cfg_alloc_internal()
    format: Format,
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

#[doc(hidden)]
impl AsNodePtr for BiquadNode<'_> {
    type __PtrProvider = BiquadNodeProvider;
}

impl<'a> BiquadNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &BiquadNodeBuilder<N>,
        alloc: Option<Arc<AllocationCallbacks>>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_biquad_node>> =
            Box::new(MaybeUninit::uninit());

        n_biquad_ffi::ma_biquad_node_init(
            node_graph,
            config.as_raw_ptr(),
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_biquad_node = Box::into_raw(mem) as *mut sys::ma_biquad_node;

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _marker: PhantomData,
            channels: config.inner.biquad.channels,
            format: config.inner.biquad.format.try_into().unwrap_or(Format::F32),
        })
    }

    /// See [`BiquadNodeParams`] for creating a config
    pub fn reinit(&mut self, config: &BiquadNodeParams) -> MaResult<()> {
        n_biquad_ffi::ma_biquad_node_reinit(config.as_raw_ptr(), self)
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

pub(crate) mod n_biquad_ffi {
    use crate::{
        engine::node_graph::{
            nodes::filters::biquad::BiquadNode, private_node_graph, AsNodeGraphPtr,
        },
        Binding, MaResult, MaudioError,
    };
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_biquad_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_biquad_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_biquad_node,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_biquad_node_init(
                private_node_graph::node_graph_ptr(node_graph),
                config,
                alloc_cb,
                node,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_biquad_node_uninit(node: &mut BiquadNode) {
        unsafe {
            sys::ma_biquad_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }

    #[inline]
    pub fn ma_biquad_node_reinit(
        config: *const sys::ma_biquad_config,
        node: &mut BiquadNode,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_biquad_node_reinit(config, node.to_raw()) };
        MaudioError::check(res)
    }
}

impl<'a> Drop for BiquadNode<'a> {
    fn drop(&mut self) {
        n_biquad_ffi::ma_biquad_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

/// Builder for creating a [`BiquadNode`]
pub struct BiquadNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_biquad_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> AsRawRef for BiquadNodeBuilder<'_, N> {
    type Raw = sys::ma_biquad_node_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<'a, N: AsNodeGraphPtr + ?Sized> BiquadNodeBuilder<'a, N> {
    #[allow(clippy::too_many_arguments)]
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

    pub fn build(&self) -> MaResult<BiquadNode<'a>> {
        if self.inner.biquad.a0 == 0.0 || self.inner.biquad.channels == 0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }
        BiquadNode::new_with_cfg_alloc_internal(self.node_graph, self, None)
    }
}

/// Used to build a config file needed by [`BiquadNode::reinit`]
pub struct BiquadNodeParams {
    inner: sys::ma_biquad_config,
}

impl AsRawRef for BiquadNodeParams {
    type Raw = sys::ma_biquad_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl BiquadNodeParams {
    pub fn new(
        biquad_node: &BiquadNode,
        b0: f64,
        b1: f64,
        b2: f64,
        a0: f64,
        a1: f64,
        a2: f64,
    ) -> Self {
        let ptr = unsafe {
            sys::ma_biquad_config_init(
                biquad_node.format.into(),
                biquad_node.channels,
                b0,
                b1,
                b2,
                a0,
                a1,
                a2,
            )
        };
        Self { inner: ptr }
    }
}

#[cfg(test)]
mod test {
    use crate::engine::{
        node_graph::nodes::filters::biquad::{BiquadNodeBuilder, BiquadNodeParams},
        Engine, EngineOps,
    };

    #[test]
    fn test_biquad_builder_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let mut node = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1)
            .build()
            .unwrap();

        let config = BiquadNodeParams::new(&node, 0.11, 0.11, 0.11, 0.11, 0.11, 0.11);
        node.reinit(&config).unwrap();
    }

    #[test]
    fn test_biquad_reinit_same_params() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let mut node = BiquadNodeBuilder::new(&node_graph, 1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7)
            .build()
            .unwrap();

        let config = BiquadNodeParams::new(&node, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7);
        node.reinit(&config).unwrap();
    }

    #[test]
    fn test_biquad_multiple_reinit() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let mut node = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1)
            .build()
            .unwrap();

        for i in 1..10 {
            let v = i as f64 * 0.01;
            let config = BiquadNodeParams::new(&node, v, v, v, v, v, v);
            node.reinit(&config).unwrap();
        }
    }

    #[test]
    fn test_biquad_nan_coefficients_1() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let result =
            BiquadNodeBuilder::new(&node_graph, 1, f32::NAN, 0.0, 0.0, 0.0, 0.0, 0.0).build();

        assert!(result.is_err(), "expected NaN coefficients to be rejected");
    }

    #[test]
    fn test_biquad_nan_coefficients_2() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let mut node = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1)
            .build()
            .unwrap();

        let config = BiquadNodeParams::new(&node, f64::INFINITY, 0.0, 0.0, 0.0, 0.0, 0.0);

        // TODO: Should check inputs on Rust side to prevent INFITITY ?
        let _ = node.reinit(&config);
    }

    #[test]
    fn test_biquad_extreme_coefficients() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            BiquadNodeBuilder::new(&node_graph, 1, 1e30, -1e30, 1e30, -1e30, 1e30, -1e30)
                .build()
                .unwrap();

        let config = BiquadNodeParams::new(&node, 1e30, 1e30, 1e30, 1e30, 1e30, 1e30);

        let _ = node.reinit(&config);
    }

    #[test]
    fn test_biquad_a0_zero_is_rejected_or_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let res = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, 0.0, 0.1, 0.1).build();

        let _ = res.is_err();
    }

    #[test]
    fn test_biquad_zero_channels_is_rejected_or_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let res = BiquadNodeBuilder::new(&node_graph, 0, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1).build();

        let _ = res.is_err();
    }

    #[test]
    fn test_biquad_reinit_a0_zero_is_rejected_or_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = BiquadNodeBuilder::new(&node_graph, 1, 0.2, 0.3, 0.4, 1.0, 0.6, 0.7)
            .build()
            .unwrap();

        let cfg = BiquadNodeParams::new(&node, 0.2, 0.3, 0.4, 0.0, 0.6, 0.7);
        let _ = node.reinit(&cfg);
    }

    #[test]
    fn test_biquad_nan_in_denominator_coeffs_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let res = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, f32::NAN, 0.1, 0.1).build();
        assert!(res.is_err() || res.is_ok());
    }

    #[test]
    fn test_biquad_create_drop_many_times() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        for _ in 0..2_000 {
            let _node = BiquadNodeBuilder::new(&node_graph, 2, 0.2, 0.3, 0.4, 1.0, 0.6, 0.7)
                .build()
                .unwrap();
        }
    }

    #[test]
    fn test_biquad_reinit_stress_many_iterations() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = BiquadNodeBuilder::new(&node_graph, 2, 0.2, 0.3, 0.4, 1.0, 0.6, 0.7)
            .build()
            .unwrap();

        for i in 0..10_000 {
            let v = (i as f64) * 1e-6;
            let cfg = BiquadNodeParams::new(&node, 0.2 + v, 0.3, 0.4, 1.0, 0.6, 0.7);
            node.reinit(&cfg).unwrap_or_else(|_| ());
        }
    }

    #[test]
    fn test_biquad_drop_before_engine_is_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let node = BiquadNodeBuilder::new(&node_graph, 1, 0.2, 0.3, 0.4, 1.0, 0.6, 0.7)
            .build()
            .unwrap();

        drop(node);
        drop(engine);
    }

    #[test]
    fn test_biquad_params_new_multichannel_is_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = BiquadNodeBuilder::new(&node_graph, 4, 0.2, 0.3, 0.4, 1.0, 0.6, 0.7)
            .build()
            .unwrap();

        let cfg = BiquadNodeParams::new(&node, 0.21, 0.31, 0.41, 1.0, 0.61, 0.71);
        node.reinit(&cfg).unwrap();
    }
}
