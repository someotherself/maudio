use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    engine::{
        node_graph::{
            nodes::{private_node, AsNodePtr, NodeRef},
            AsNodeGraphPtr, NodeGraph,
        },
        AllocationCallbacks,
    },
    Binding, MaResult,
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

#[doc(hidden)]
impl AsNodePtr for DelayNode<'_> {
    type __PtrProvider = private_node::DelayNodeProvider;
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
        config: &DelayNodeBuilder<N>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_delay_node>> =
            Box::new(MaybeUninit::uninit());

        n_delay_ffi::ma_delay_node_init(node_graph, config.to_raw(), alloc_cb, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_delay_node = Box::into_raw(mem) as *mut sys::ma_delay_node;

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
        engine::node_graph::{
            nodes::effects::delay::DelayNode, private_node_graph, AsNodeGraphPtr,
        },
        Binding, MaResult, MaudioError,
    };
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_delay_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_delay_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_delay_node,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_delay_node_init(
                private_node_graph::node_graph_ptr(node_graph),
                config,
                alloc_cb,
                node,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_delay_node_uninit(node: &mut DelayNode) {
        unsafe { sys::ma_delay_node_uninit(node.to_raw(), node.alloc_cb_ptr()) }
    }

    #[inline]
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
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

/// Builder for creating a [`DelayNode`]
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
    pub fn wet(&mut self, wet: f32) -> &mut Self {
        self.inner.delay.wet = wet;
        self
    }

    /// Sets the gain of the *dry* (unprocessed) signal.
    ///
    /// The dry signal is the original input audio before any delay is applied.
    /// Higher values preserve more of the original sound in the final output.
    /// Values are not clamped.
    pub fn dry(&mut self, dry: f32) -> &mut Self {
        self.inner.delay.dry = dry;
        self
    }

    /// Sets the balance between the dry and wet signals.
    ///
    /// `0.0` is fully dry (no delay audible), and `1.0` is fully wet
    /// (only the delayed signal). Values are clamped to `0.0..=1.0`.
    ///
    /// This overwrites both the wet and dry gains.
    pub fn mix(&mut self, mix: f32) -> &mut Self {
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
    pub fn decay(&mut self, decay: f32) -> &mut Self {
        self.inner.delay.decay = decay;
        self
    }

    /// Emables or disables a delayed start
    pub fn delay_start(&mut self, yes: bool) -> &mut Self {
        let delay_start = yes as u32;
        self.inner.delay.delayStart = delay_start;
        self
    }

    /// Sets the frame at which the delay starts.
    ///
    /// This offsets when the delay begins relative to the input signal.
    pub fn start_frame(&mut self, frame: u32) -> &mut Self {
        self.inner.delay.delayInFrames = frame;
        self
    }

    /// Sets the length of the delay in milliseconds.
    ///
    /// This is a convenience wrapper around `delay_start` that converts
    /// milliseconds to frames using the configured sample rate.
    pub fn delay_milli(&mut self, millis: u32) -> &mut Self {
        self.inner.delay.delayInFrames = self.millis_to_frames(millis);
        self
    }

    /// Sets the delay start offset in milliseconds.
    ///
    /// This is a convenience wrapper around `start_frame` that converts
    /// millisseconds to frames using the configured sample rate.
    pub fn start_milli(&mut self, millis: u32) -> &mut Self {
        self.inner.delay.delayStart = self.millis_to_frames(millis);
        self
    }

    pub fn build(&self) -> MaResult<DelayNode<'a>> {
        DelayNode::new_with_cfg_alloc_internal(self.node_graph, self, None)
    }

    #[inline]
    fn millis_to_frames(&self, millis: u32) -> u32 {
        let sr = self.inner.delay.sampleRate;
        (millis * sr + 500) / 1000
    }
}

#[cfg(test)]
mod test {
    use crate::engine::{node_graph::nodes::private_node, Engine, EngineOps};

    use super::*;

    fn assert_approx_eq(a: f32, b: f32, eps: f32) {
        assert!(
            (a - b).abs() <= eps,
            "expected {a} ≈ {b} (eps={eps}), diff={}",
            (a - b).abs()
        );
    }

    #[test]
    fn test_delay_node_test_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
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
        let engine = Engine::new_for_tests().unwrap();
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
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let mut delay = DelayNodeBuilder::new(&node_graph, 1, 441000, 0, 0.0)
            .build()
            .unwrap();

        delay.set_dry(0.75);
        assert_approx_eq(delay.dry(), 0.75, 1e-6);

        delay.set_dry(-0.5);
        assert_approx_eq(delay.dry(), -0.5, 1e-6);
    }

    #[test]
    fn test_delay_node_test_set_get_decay_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let mut delay = DelayNodeBuilder::new(&node_graph, 1, 441000, 0, 0.0)
            .build()
            .unwrap();

        delay.set_decay_frames(0.0);
        assert_approx_eq(delay.decay_frames(), 0.0, 1e-6);

        delay.set_decay_frames(0.4);
        assert_approx_eq(delay.decay_frames(), 0.4, 1e-6);

        delay.set_decay_frames(1.1);
        assert_approx_eq(delay.decay_frames(), 1.1, 1e-6);
    }

    #[test]
    fn test_delay_node_test_as_node_is_non_null() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();
        let delay = DelayNodeBuilder::new(&node_graph, 1, 441000, 0, 0.0)
            .build()
            .unwrap();

        let node_ref = delay.as_node();
        assert!(!private_node::node_ptr(&node_ref).is_null());
        let _ = node_ref;
    }
}
