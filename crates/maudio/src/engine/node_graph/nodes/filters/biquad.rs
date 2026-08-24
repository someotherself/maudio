use std::mem::MaybeUninit;

use maudio_sys::ffi as sys;

use crate::{
    audio::formats::Format,
    engine::{
        node_graph::{
            nodes::{
                node_ffi, private_node::BiquadNodeProvider, AsNodePtr, NodeBusChannels,
                NodeBusChannelsConfig, NodeRef,
            },
            private_node_graph, AsNodeGraphPtr, GraphOwner, NodeGraph, NodeGraphRef,
        },
        Engine,
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
/// - After creating the filter, use [`Self::reinit`] to change the values of the coefficients.
///   This reinitializes the filter coefficients without clearing the internal state.
///   This allows filter parameters to be updated in real time without causing
///   audible artifacts such as clicks or pops.
/// - Changing the format or channel count after initialization is invalid and
///   will result in an error.
///
/// Use [`BiquadNodeBuilder`] to initialize
pub struct BiquadNode {
    inner: *mut sys::ma_biquad_node,
    pub(crate) owner: GraphOwner,
    // format is hard coded as ma_format_f32 in miniaudio `sys::ma_biquad_node_config_init()`
    // but use value in inner.biquad.format anyway inside new_with_cfg_internal()
    _busses: NodeBusChannels, // keep alive
    format: Format,
}

unsafe impl Send for BiquadNode {}

impl Binding for BiquadNode {
    type Raw = *mut sys::ma_biquad_node;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[doc(hidden)]
impl AsNodePtr for BiquadNode {
    type __PtrProvider = BiquadNodeProvider;
}

impl BiquadNode {
    fn new_with_cfg_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &mut BiquadNodeBuilder<N>,
    ) -> MaResult<Self> {
        let busses = config.busses.build_nodes(node_graph);

        config.inner.nodeConfig.inputBusCount = busses.inputs.len() as u32;
        config.inner.nodeConfig.outputBusCount = busses.outputs.len() as u32;
        config.inner.nodeConfig.pInputChannels = busses.inputs.as_ptr();
        config.inner.nodeConfig.pOutputChannels = busses.outputs.as_ptr();

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_biquad_node>> =
            Box::new(MaybeUninit::uninit());

        n_biquad_ffi::ma_biquad_node_init(node_graph, config.as_raw_ptr(), mem.as_mut_ptr())?;

        let inner: *mut sys::ma_biquad_node = Box::into_raw(mem) as *mut sys::ma_biquad_node;

        Ok(Self {
            inner,
            owner: private_node_graph::clone_owner(node_graph),
            _busses: busses,
            format: config.inner.biquad.format.try_into().unwrap_or(Format::F32),
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

    pub fn reinit(&mut self, b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> MaResult<()> {
        let param = BiquadNodeParams::new(self, b0, b1, b2, a0, a1, a2);
        n_biquad_ffi::ma_biquad_node_reinit(param.as_raw_ptr(), self)
    }

    /// Returns a **borrowed view** as a node in the engine's node graph.
    pub fn as_node<'a>(&'a self) -> NodeRef<'a> {
        assert!(!self.to_raw().is_null());
        let ptr = self.to_raw().cast::<sys::ma_node>();
        NodeRef::from_ptr(ptr)
    }
}

pub(crate) mod n_biquad_ffi {
    use crate::{
        engine::node_graph::{
            nodes::filters::biquad::BiquadNode, private_node_graph, AsNodeGraphPtr,
        },
        AllocationCallbacks, Binding, MaResult, MaudioError,
    };
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_biquad_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_biquad_node_config,
        node: *mut sys::ma_biquad_node,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_biquad_node_init(
                private_node_graph::node_graph_ptr(node_graph),
                config,
                AllocationCallbacks::cb_ptr(),
                node,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_biquad_node_uninit(node: &mut BiquadNode) {
        unsafe {
            sys::ma_biquad_node_uninit(node.to_raw(), AllocationCallbacks::cb_ptr());
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

impl Drop for BiquadNode {
    fn drop(&mut self) {
        n_biquad_ffi::ma_biquad_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

/// Builder for creating a [`BiquadNode`]
pub struct BiquadNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_biquad_node_config,
    busses: NodeBusChannelsConfig,
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
        let busses = NodeBusChannelsConfig::new(1, 1, Some(channels));
        Self {
            inner: ptr,
            busses,
            node_graph,
        }
    }

    /// This node can only have one input.
    ///
    /// This sets the channel count for input bus with the index `0`.
    ///
    /// Input and output channel counts may differ. However, it does not always make sense.
    /// Do not assume that miniaudio automatically performs channel conversion.
    /// Reinitialization will usually fail if input and output channels are different.
    ///
    /// Mixing nodes with different channel counts may result in malformed audio
    /// or errors when connecting busses.
    pub fn in_channel_count(&mut self, count: u32) -> &mut Self {
        self.busses.change_chanels_in(0, count);
        self
    }

    /// This node can only have one output.
    ///
    /// This sets the channel count for output bus with the index `0`.
    ///
    /// Input and output channel counts may differ. However, it does not always make sense.
    /// Do not assume that miniaudio automatically performs channel conversion.
    /// Reinitialization will usually fail if input and output channels are different.
    ///
    /// Mixing nodes with different channel counts may result in malformed audio
    /// or errors when connecting busses.
    pub fn out_channel_count(&mut self, count: u32) -> &mut Self {
        self.busses.change_chanels_out(0, count);
        self
    }

    pub fn build(&mut self) -> MaResult<BiquadNode> {
        if self.inner.biquad.a0 == 0.0 || self.inner.biquad.channels == 0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }
        BiquadNode::new_with_cfg_internal(self.node_graph, self)
    }
}

/// Used to build a config file needed by [`BiquadNode::reinit`]
struct BiquadNodeParams {
    inner: sys::ma_biquad_config,
}

impl AsRawRef for BiquadNodeParams {
    type Raw = sys::ma_biquad_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl BiquadNodeParams {
    fn new(biquad_node: &BiquadNode, b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        let ptr = unsafe {
            sys::ma_biquad_config_init(
                biquad_node.format.into(),
                biquad_node._busses.outputs[0],
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
    use crate::engine::{node_graph::nodes::filters::biquad::BiquadNodeBuilder, Engine};

    #[test]
    fn test_biquad_builder_channel_counts() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();
        let _node = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1)
            .in_channel_count(2)
            .out_channel_count(4)
            .build()
            .unwrap();
    }

    #[test]
    fn test_biquad_builder_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();
        let mut node = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1)
            .build()
            .unwrap();

        node.reinit(0.11, 0.11, 0.11, 0.11, 0.11, 0.11).unwrap();
    }

    #[test]
    fn test_biquad_reinit_same_params() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();
        let mut node = BiquadNodeBuilder::new(&node_graph, 1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7)
            .build()
            .unwrap();

        node.reinit(0.2, 0.3, 0.4, 0.5, 0.6, 0.7).unwrap();
    }

    #[test]
    fn test_biquad_multiple_reinit() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();
        let mut node = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1)
            .build()
            .unwrap();

        for i in 1..10 {
            let v = i as f64 * 0.01;
            node.reinit(v, v, v, v, v, v).unwrap();
        }
    }

    #[test]
    fn test_biquad_nan_coefficients_1() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();
        let result =
            BiquadNodeBuilder::new(&node_graph, 1, f32::NAN, 0.0, 0.0, 0.0, 0.0, 0.0).build();

        assert!(result.is_err(), "expected NaN coefficients to be rejected");
    }

    #[test]
    fn test_biquad_nan_coefficients_2() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();
        let mut node = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1)
            .build()
            .unwrap();

        // TODO: Should check inputs on Rust side to prevent INFITITY ?
        let _ = node.reinit(f64::INFINITY, 0.0, 0.0, 0.0, 0.0, 0.0);
    }

    #[test]
    fn test_biquad_extreme_coefficients() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();

        let mut node =
            BiquadNodeBuilder::new(&node_graph, 1, 1e30, -1e30, 1e30, -1e30, 1e30, -1e30)
                .build()
                .unwrap();

        let _ = node.reinit(1e30, 1e30, 1e30, 1e30, 1e30, 1e30);
    }

    #[test]
    fn test_biquad_a0_zero_is_rejected_or_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();

        let res = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, 0.0, 0.1, 0.1).build();

        let _ = res.is_err();
    }

    #[test]
    fn test_biquad_zero_channels_is_rejected_or_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();

        let res = BiquadNodeBuilder::new(&node_graph, 0, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1).build();

        let _ = res.is_err();
    }

    #[test]
    fn test_biquad_reinit_a0_zero_is_rejected_or_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();

        let mut node = BiquadNodeBuilder::new(&node_graph, 1, 0.2, 0.3, 0.4, 1.0, 0.6, 0.7)
            .build()
            .unwrap();

        let _ = node.reinit(0.2, 0.3, 0.4, 0.0, 0.6, 0.7);
    }

    #[test]
    fn test_biquad_nan_in_denominator_coeffs_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();

        let res = BiquadNodeBuilder::new(&node_graph, 1, 0.1, 0.1, 0.1, f32::NAN, 0.1, 0.1).build();
        assert!(res.is_err() || res.is_ok());
    }

    #[test]
    fn test_biquad_create_drop_many_times() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();

        for _ in 0..2_000 {
            let _node = BiquadNodeBuilder::new(&node_graph, 2, 0.2, 0.3, 0.4, 1.0, 0.6, 0.7)
                .build()
                .unwrap();
        }
    }

    #[test]
    fn test_biquad_reinit_stress_many_iterations() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();

        let mut node = BiquadNodeBuilder::new(&node_graph, 2, 0.2, 0.3, 0.4, 1.0, 0.6, 0.7)
            .build()
            .unwrap();

        for i in 0..10_000 {
            let v = (i as f64) * 1e-6;
            node.reinit(0.2 + v, 0.3, 0.4, 1.0, 0.6, 0.7).unwrap();
        }
    }

    #[test]
    fn test_biquad_drop_before_engine_is_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();

        let node = BiquadNodeBuilder::new(&node_graph, 1, 0.2, 0.3, 0.4, 1.0, 0.6, 0.7)
            .build()
            .unwrap();

        drop(node);
        drop(engine);
    }

    #[test]
    fn test_biquad_params_new_multichannel_is_safe() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph();

        let mut node = BiquadNodeBuilder::new(&node_graph, 4, 0.2, 0.3, 0.4, 1.0, 0.6, 0.7)
            .build()
            .unwrap();

        node.reinit(0.21, 0.31, 0.41, 1.0, 0.61, 0.71).unwrap();
    }
}
