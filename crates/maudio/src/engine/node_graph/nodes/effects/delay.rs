use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    Binding, Result,
    engine::{
        AllocationCallbacks,
        node_graph::{AsNodeGraphPtr, NodeGraph, nodes::NodeRef},
    },
};

/// A node that applies a delay (echo) effect to an audio signal.
///
/// `DelayNode` is one of the custom DSP nodes provided by miniaudio.
/// It mixes the original (dry) signal with a delayed (wet) copy, allowing
/// control over the delay length, feedback (decay), and wet/dry balance.
/// The node is intended to be used as part of a node graph and processes
/// audio in fixed-size frames according to the graph’s format.
///
/// Use [`DelayNodeBuilder`] to initialize
pub struct DelayNode<'a> {
    inner: *mut sys::ma_delay_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
}

impl Binding for DelayNode<'_> {
    type Raw = *mut sys::ma_delay_node;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a> DelayNode<'a> {
    /// Read the gain of the *wet* (delayed) signal.
    pub fn wet(&self) -> f32 {
        n_delay_ffi::ma_delay_node_get_wet(self)
    }

    /// Sets the gain of the *wet* (delayed) signal.
    ///
    /// The wet signal is the audio after it has passed through the delay.
    /// Higher values make the echo more prominent in the final output.
    /// Values are not clamped.
    pub fn set_wet(&mut self, wet: f32) {
        n_delay_ffi::ma_delay_node_set_wet(self, wet);
    }

    /// Reads the gain of the *dry* (unprocessed) signal.
    pub fn dry(&self) -> f32 {
        n_delay_ffi::ma_delay_node_get_dry(self)
    }

    /// Sets the gain of the *dry* (unprocessed) signal.
    ///
    /// The dry signal is the original input audio before any delay is applied.
    /// Higher values preserve more of the original sound in the final output.
    /// Values are not clamped.
    pub fn set_dry(&mut self, dry: f32) {
        n_delay_ffi::ma_delay_node_set_dry(self, dry);
    }

    /// Reads the feedback amount of the delay line in frames
    pub fn decay_frames(&self) -> f32 {
        n_delay_ffi::ma_delay_node_get_decay(self)
    }

    /// Sets the feedback amount of the delay line in frames
    ///
    /// Higher values cause the delayed signal to repeat longer, while
    /// lower values fade out more quickly. Values near or above `1.0`
    /// may cause self-oscillation.
    pub fn set_decay_frames(&mut self, decay: f32) {
        n_delay_ffi::ma_delay_node_set_decay(self, decay);
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

    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: Option<&DelayNodeBuilder<N>>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> Result<Self> {
        let config: *const sys::ma_delay_node_config =
            config.map_or(core::ptr::null(), |c| c.to_raw());
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_delay_node>> = Box::new_uninit();

        n_delay_ffi::ma_delay_node_init_with_config(
            node_graph,
            config,
            alloc_cb,
            mem.as_mut_ptr(),
        )?;
        let ptr = unsafe { mem.assume_init() };
        let inner = Box::into_raw(ptr);
        Ok(Self {
            inner,
            alloc_cb: alloc,
            _marker: PhantomData,
        })
    }

    #[inline]
    fn alloc_cb_ptr(&self) -> *const sys::ma_allocation_callbacks {
        match &self.alloc_cb {
            Some(cb) => &cb.inner as *const _,
            None => core::ptr::null(),
        }
    }
}

pub(crate) mod n_delay_ffi {
    use crate::{
        Binding, MaRawResult, Result,
        engine::node_graph::{AsNodeGraphPtr, nodes::effects::delay::DelayNode},
    };
    use maudio_sys::ffi as sys;

    pub fn ma_delay_node_init_with_config<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_delay_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_delay_node,
    ) -> Result<()> {
        let res = unsafe {
            sys::ma_delay_node_init(node_graph.as_nodegraph_ptr(), config, alloc_cb, node)
        };
        MaRawResult::resolve(res)
    }

    pub fn ma_delay_node_uninit(node: &mut DelayNode) {
        unsafe { sys::ma_delay_node_uninit(node.to_raw(), node.alloc_cb_ptr()) }
    }

    pub fn ma_delay_node_set_wet(node: &mut DelayNode, wet: f32) {
        unsafe {
            sys::ma_delay_node_set_wet(node.to_raw(), wet);
        }
    }

    pub fn ma_delay_node_get_wet(node: &DelayNode) -> f32 {
        unsafe { sys::ma_delay_node_get_wet(node.to_raw() as *const _) }
    }

    pub fn ma_delay_node_set_dry(node: &mut DelayNode, dry: f32) {
        unsafe {
            sys::ma_delay_node_set_dry(node.to_raw(), dry);
        }
    }

    pub fn ma_delay_node_get_dry(node: &DelayNode) -> f32 {
        unsafe { sys::ma_delay_node_get_dry(node.to_raw() as *const _) }
    }

    pub fn ma_delay_node_set_decay(node: &mut DelayNode, decay: f32) {
        unsafe {
            sys::ma_delay_node_set_decay(node.to_raw(), decay);
        }
    }

    pub fn ma_delay_node_get_decay(node: &DelayNode) -> f32 {
        unsafe { sys::ma_delay_node_get_decay(node.to_raw() as *const _) }
    }
}

impl Drop for DelayNode<'_> {
    fn drop(&mut self) {
        n_delay_ffi::ma_delay_node_uninit(self);
    }
}

pub struct DelayNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_delay_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> Binding for DelayNodeBuilder<'_, N> {
    type Raw = *const sys::ma_delay_node_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

impl<'a, N: AsNodeGraphPtr + ?Sized> DelayNodeBuilder<'a, N> {
    pub fn new(
        node_graph: &'a N,
        channels: u32,
        sample_rate: u32,
        delay_frames: u32,
        decay: f32,
    ) -> Self {
        let inner =
            unsafe { sys::ma_delay_node_config_init(channels, sample_rate, delay_frames, decay) };
        Self { inner, node_graph }
    }

    /// Sets the gain of the *wet* (delayed) signal.
    ///
    /// The wet signal is the audio after it has passed through the delay.
    /// Higher values make the echo more prominent in the final output.
    /// Values are not clamped.
    pub fn wet(mut self, wet: f32) -> Self {
        self.inner.delay.wet = wet;
        self
    }

    /// Sets the gain of the *dry* (unprocessed) signal.
    ///
    /// The dry signal is the original input audio before any delay is applied.
    /// Higher values preserve more of the original sound in the final output.
    /// Values are not clamped.
    pub fn dry(mut self, dry: f32) -> Self {
        self.inner.delay.dry = dry;
        self
    }

    /// Sets the balance between the dry and wet signals.
    ///
    /// `0.0` is fully dry (no delay audible), and `1.0` is fully wet
    /// (only the delayed signal). Values are clamped to `0.0..=1.0`.
    ///
    /// This overwrites both the wet and dry gains.
    pub fn mix(mut self, mix: f32) -> Self {
        let mix = mix.clamp(0.0, 1.0);

        self.inner.delay.wet = mix;
        self.inner.delay.dry = 1.0 - mix;

        self
    }

    /// Sets the feedback amount of the delay line.
    ///
    /// Higher values cause the delayed signal to repeat longer, while
    /// lower values fade out more quickly. Values near or above `1.0`
    /// may cause self-oscillation.
    pub fn decay(mut self, decay: f32) -> Self {
        self.inner.delay.decay = decay;
        self
    }

    /// Emables or disables a delayed start
    pub fn delay_start(mut self, yes: bool) -> Self {
        let delay_start = yes as u32;
        self.inner.delay.delayStart = delay_start;
        self
    }

    /// Sets the frame at which the delay starts.
    ///
    /// This offsets when the delay begins relative to the input signal.
    pub fn start_frame(mut self, frame: u32) -> Self {
        self.inner.delay.delayInFrames = frame;
        self
    }

    /// Sets the length of the delay in milliseconds.
    ///
    /// This is a convenience wrapper around `delay_start` that converts
    /// milliseconds to frames using the configured sample rate.
    pub fn delay_milli(mut self, millis: u32) -> Self {
        self.inner.delay.delayInFrames = self.millis_to_frames(millis);
        self
    }

    /// Sets the delay start offset in milliseconds.
    ///
    /// This is a convenience wrapper around `start_frame` that converts
    /// millisseconds to frames using the configured sample rate.
    pub fn start_milli(mut self, millis: u32) -> Self {
        self.inner.delay.delayStart = self.millis_to_frames(millis);
        self
    }

    pub fn build(self) -> Result<DelayNode<'a>> {
        DelayNode::new_with_cfg_alloc_internal(self.node_graph, Some(&self), None)
    }

    #[inline]
    fn millis_to_frames(&self, millis: u32) -> u32 {
        let sr = self.inner.delay.sampleRate;
        (millis * sr + 500) / 1000
    }
}

#[cfg(feature = "device-tests")]
#[cfg(test)]
mod test {
    use crate::engine::{Engine, EngineOps, node_graph::nodes::AsNodePtr};

    use super::*;

    // Tiny float helper for comparisons.
    fn assert_approx_eq(a: f32, b: f32, eps: f32) {
        assert!(
            (a - b).abs() <= eps,
            "expected {a} ≈ {b} (eps={eps}), diff={}",
            (a - b).abs()
        );
    }

    #[test]
    fn test_delay_node_test_basic_init() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let delay = DelayNodeBuilder::new(&node_graph, 1, 441000, 0, 0.0)
            .build()
            .unwrap();

        let _ = delay.wet();
        let _ = delay.dry();
        let _ = delay.decay_frames();

        let _ = delay.as_node();
    }

    #[test]
    fn test_delay_node_test_set_get_wet_roundtrip() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let mut delay = DelayNodeBuilder::new(&node_graph, 1, 441000, 0, 0.0)
            .build()
            .unwrap();

        delay.set_wet(0.25);
        assert_approx_eq(delay.wet(), 0.25, 1e-6);

        delay.set_wet(1.5);
        assert_approx_eq(delay.wet(), 1.5, 1e-6);
    }

    #[test]
    fn test_delay_node_test_set_get_dry_roundtrip() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let mut delay = DelayNodeBuilder::new(&node_graph, 1, 441000, 0, 0.0)
            .build()
            .unwrap();

        delay.set_dry(0.75);
        assert_approx_eq(delay.dry(), 0.75, 1e-6);

        // Not clamped: negative should round-trip (phase inversion use-case).
        delay.set_dry(-0.5);
        assert_approx_eq(delay.dry(), -0.5, 1e-6);
    }

    #[test]
    fn test_delay_node_test_set_get_decay_roundtrip() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let mut delay = DelayNodeBuilder::new(&node_graph, 1, 441000, 0, 0.0)
            .build()
            .unwrap();

        delay.set_decay_frames(0.0);
        assert_approx_eq(delay.decay_frames(), 0.0, 1e-6);

        delay.set_decay_frames(0.4);
        assert_approx_eq(delay.decay_frames(), 0.4, 1e-6);

        // Not clamped: >= 1.0 may be unstable in audio terms, but API should accept it.
        delay.set_decay_frames(1.1);
        assert_approx_eq(delay.decay_frames(), 1.1, 1e-6);
    }

    #[test]
    fn test_delay_node_test_as_node_is_non_null() {
        let engine = Engine::new().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let delay = DelayNodeBuilder::new(&node_graph, 1, 441000, 0, 0.0)
            .build()
            .unwrap();

        let node_ref = delay.as_node();
        assert!(!node_ref.as_engine_ptr().is_null());
        let _ = node_ref;
    }

    struct FakeGraph;
    impl AsNodeGraphPtr for FakeGraph {
        fn as_nodegraph_ptr(&self) -> *mut sys::ma_node_graph {
            core::ptr::null_mut()
        }
    }

    fn builder_with(sr: u32) -> DelayNodeBuilder<'static, FakeGraph> {
        // Safety: 'static for tests; the FakeGraph is a plain value in this module.
        static G: FakeGraph = FakeGraph;
        DelayNodeBuilder::new(&G, 2, sr, 123, 0.5)
    }

    #[test]
    fn test_delay_builder_mix_clamps_and_overwrites_wet_dry() {
        let b = builder_with(48_000).wet(0.9).dry(0.1).mix(-1.0);

        // mix(-1.0) clamps to 0.0
        assert_approx_eq(b.inner.delay.wet, 0.0, 1e-6);
        assert_approx_eq(b.inner.delay.dry, 1.0, 1e-6);

        let b = builder_with(48_000).mix(2.0);
        // mix(2.0) clamps to 1.0
        assert_approx_eq(b.inner.delay.wet, 1.0, 1e-6);
        assert_approx_eq(b.inner.delay.dry, 0.0, 1e-6);

        let b = builder_with(48_000).mix(0.25);
        assert_approx_eq(b.inner.delay.wet, 0.25, 1e-6);
        assert_approx_eq(b.inner.delay.dry, 0.75, 1e-6);
    }

    #[test]
    fn test_delay_builder_wet_dry_are_not_clamped() {
        let b = builder_with(48_000).wet(1.5).dry(-0.5);
        assert_approx_eq(b.inner.delay.wet, 1.5, 1e-6);
        assert_approx_eq(b.inner.delay.dry, -0.5, 1e-6);
    }

    #[test]
    fn test_delay_builder_decay_accepts_nan_and_infinity_without_panic() {
        // You’re not clamping/validating decay in the builder, so the test here is:
        // it should not panic and should store the value.
        let b = builder_with(48_000).decay(f32::NAN);
        assert!(b.inner.delay.decay.is_nan());

        let b = builder_with(48_000).decay(f32::INFINITY);
        assert!(b.inner.delay.decay.is_infinite());

        let b = builder_with(48_000).decay(-10.0);
        assert_approx_eq(b.inner.delay.decay, -10.0, 1e-6);
    }

    #[test]
    fn test_delay_builder_millis_to_frames_rounding_works() {
        // Use 44.1k to ensure non-integer frames per ms.
        // frames = round(ms * sr / 1000) in integer form via +500.
        let b = builder_with(44_100);

        // 1ms at 44.1kHz = 44.1 frames => rounds to 44
        let b1 = b.clone_for_test().delay_milli(1);
        assert_eq!(b1.inner.delay.delayInFrames, 44);

        // 5ms at 44.1kHz = 220.5 frames => rounds to 221 (this demonstrates the +500 behavior)
        let b2 = b.clone_for_test().delay_milli(5);
        assert_eq!(b2.inner.delay.delayInFrames, 221);

        // 0ms => 0 frames
        let b3 = b.clone_for_test().delay_milli(0);
        assert_eq!(b3.inner.delay.delayInFrames, 0);
    }

    // Small helper since the builder consumes self in setters.
    trait CloneForTest {
        fn clone_for_test(&self) -> Self;
    }
    impl CloneForTest for DelayNodeBuilder<'static, FakeGraph> {
        fn clone_for_test(&self) -> Self {
            Self {
                inner: self.inner,
                node_graph: self.node_graph,
            }
        }
    }

    #[test]
    fn test_delay_builder_new_abuse_inputs_does_not_panic() {
        static G: FakeGraph = FakeGraph;

        // Extreme channels/sample rates/delay frames - the builder should construct without panicking.
        let _ = DelayNodeBuilder::new(&G, 0, 0, 0, 0.0);
        let _ = DelayNodeBuilder::new(&G, u32::MAX, u32::MAX, u32::MAX, 0.0);

        // Weird decay values should also not panic at construction time.
        let _ = DelayNodeBuilder::new(&G, 2, 48_000, 10_000, f32::NAN);
        let _ = DelayNodeBuilder::new(&G, 2, 48_000, 10_000, f32::INFINITY);
        let _ = DelayNodeBuilder::new(&G, 2, 48_000, 10_000, -1.0);
    }

    #[test]
    fn test_delay_builder_delay_start_sets_flag_field() {
        let b = builder_with(48_000).delay_start(false);
        assert_eq!(b.inner.delay.delayStart, 0);

        let b = builder_with(48_000).delay_start(true);
        assert_eq!(b.inner.delay.delayStart, 1);
    }

    #[test]
    fn test_delay_builder_start_frame_currently_sets_delay_length_field() {
        // This test reflects your CURRENT implementation.
        // If you meant start_frame() to set delayStart, then this test should be updated
        // after you fix the implementation.
        let b = builder_with(48_000).start_frame(1234);
        assert_eq!(b.inner.delay.delayInFrames, 1234);
    }
}
