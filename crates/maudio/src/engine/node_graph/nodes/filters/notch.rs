use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    Binding, MaResult,
    audio::{formats::Format, sample_rate::SampleRate},
    engine::{
        AllocationCallbacks,
        node_graph::{
            AsNodeGraphPtr, NodeGraph,
            nodes::{AsNodePtr, NodeRef, private_node::NotchNodeProvider},
        },
    },
};

/// A node that applies a **notch filter (band-stop)** to an audio signal.
///
/// A notch filter attenuates a **narrow band of frequencies** centered around a target frequency,
/// while leaving frequencies below and above mostly unchanged. This is commonly used to remove
/// tonal noise such as **mains hum** (50/60 Hz), whistles, resonances, or feedback.
///
/// `NotchNode` is a node-graph wrapper around miniaudio's notch filter implementation.
/// It is designed for real-time use inside a [`NodeGraph`], and maintains internal filter state
/// across parameter changes.
///
/// ## Parameters
/// - **frequency**: The center frequency (Hz) to attenuate.
/// - **q**: The quality factor (bandwidth control).
///   Higher values produce a **narrower, more selective** notch,
///   while lower values produce a **wider** cut.
///
/// ## Notes
/// After creating the filter, use [`Self::reinit`] and [`NotchNodeParams`] to change the filter parameters.
/// This reinitializes the filter coefficients without clearing the internal state.
/// This allows filter parameters to be updated in real time without causing
/// audible artifacts such as clicks or pops.
///
/// Use [`NotchNodeBuilder`] to initialize.
pub struct NotchNode<'a> {
    inner: *mut sys::ma_notch_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
    // Below is needed during a reinit
    channels: u32,
    // format is hard coded as ma_format_f32 in miniaudio `sys::ma_hpf_node_config_init()`
    // but use value in inner.hpf.format anyway inside new_with_cfg_alloc_internal()
    format: Format,
    sample_rate: SampleRate,
}

impl Binding for NotchNode<'_> {
    type Raw = *mut sys::ma_notch_node;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[doc(hidden)]
impl AsNodePtr for NotchNode<'_> {
    type __PtrProvider = NotchNodeProvider;
}

impl<'a> NotchNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &NotchNodeBuilder<'_, N>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_notch_node>> = Box::new_uninit();

        n_notch_ffi::ma_notch_node_init(node_graph, config.to_raw(), alloc_cb, mem.as_mut_ptr())?;

        let ptr: Box<sys::ma_notch_node> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_notch_node = Box::into_raw(ptr);

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _marker: PhantomData,
            format: config.inner.notch.format.try_into().unwrap_or(Format::F32),
            channels: config.inner.notch.channels,
            sample_rate: config.inner.notch.sampleRate.try_into()?,
        })
    }

    /// See [`NotchNodeParams`] for creating a config
    pub fn reinit(&mut self, config: &NotchNodeParams) -> MaResult<()> {
        n_notch_ffi::ma_notch_node_reinit(config.to_raw(), self)
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

pub(crate) mod n_notch_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        Binding, MaRawResult, MaResult,
        engine::node_graph::{
            AsNodeGraphPtr, nodes::filters::notch::NotchNode, private_node_graph,
        },
    };

    #[inline]
    pub fn ma_notch_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_notch_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_notch_node,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_notch_node_init(
                private_node_graph::node_graph_ptr(node_graph),
                config,
                alloc_cb,
                node,
            )
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_notch_node_uninit(node: &mut NotchNode) {
        unsafe {
            sys::ma_notch_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }

    #[inline]
    pub fn ma_notch_node_reinit(
        config: *const sys::ma_notch_config,
        node: &mut NotchNode,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_notch_node_reinit(config, node.to_raw()) };
        MaRawResult::check(res)
    }
}

impl<'a> Drop for NotchNode<'a> {
    fn drop(&mut self) {
        n_notch_ffi::ma_notch_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub struct NotchNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_notch_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> Binding for NotchNodeBuilder<'_, N> {
    type Raw = *const sys::ma_notch_node_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}
impl<'a, N: AsNodeGraphPtr + ?Sized> NotchNodeBuilder<'a, N> {
    pub fn new(
        node_graph: &'a N,
        channels: u32,
        sample_rate: SampleRate,
        quality_factor: f64,
        frequency: f64,
    ) -> Self {
        let ptr = unsafe {
            sys::ma_notch_node_config_init(channels, sample_rate.into(), quality_factor, frequency)
        };
        NotchNodeBuilder {
            inner: ptr,
            node_graph,
        }
    }

    pub fn build(self) -> MaResult<NotchNode<'a>> {
        NotchNode::new_with_cfg_alloc_internal(self.node_graph, &self, None)
    }
}

pub struct NotchNodeParams {
    inner: sys::ma_notch_config,
}

impl Binding for NotchNodeParams {
    type Raw = *const sys::ma_notch_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

impl NotchNodeParams {
    pub fn new(node: &NotchNode, q: f64, frequency: f64) -> Self {
        let ptr = unsafe {
            sys::ma_notch2_config_init(
                node.format.into(),
                node.channels,
                node.sample_rate.into(),
                q,
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
            Engine, EngineOps,
            node_graph::nodes::filters::notch::{NotchNodeBuilder, NotchNodeParams},
        },
    };

    #[test]
    fn test_notch_builder_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = NotchNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1.0, 2000.0)
            .build()
            .unwrap();
        let config = NotchNodeParams::new(&node, 1.0, 3000.0);
        node.reinit(&config).unwrap();
    }

    #[test]
    fn test_notch_builder_stereo_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        // Just ensure init works for >1 channel.
        let _node = NotchNodeBuilder::new(&node_graph, 2, SampleRate::Sr44100, 1.0, 1000.0)
            .build()
            .unwrap();
    }

    #[test]
    fn test_notch_multiple_reinit_stability() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = NotchNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 2.0, 500.0)
            .build()
            .unwrap();

        // Sweep frequency and Q a bit. This should not error and should not reset state.
        for i in 0..50 {
            let f = 200.0 + (i as f64) * 40.0; // 200..2160 Hz
            let q = 0.7 + (i as f64) * 0.05; // 0.7..3.15
            let cfg = NotchNodeParams::new(&node, q, f);
            node.reinit(&cfg).unwrap();
        }
    }

    #[test]
    fn test_notch_reinit_frequency_zero_should_error() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = NotchNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1.0, 1000.0)
            .build()
            .unwrap();

        // TODO: zero frequency also valid?
        let bad_cfg = NotchNodeParams::new(&node, 1.0, 0.0);
        assert!(node.reinit(&bad_cfg).is_ok());
    }

    #[test]
    fn test_notch_reinit_negative_q_should_error() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = NotchNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1.0, 1000.0)
            .build()
            .unwrap();

        let bad_cfg = NotchNodeParams::new(&node, -1.0, 1000.0);
        // TODO: Negative quality factor?
        assert!(node.reinit(&bad_cfg).is_ok());
    }

    #[test]
    fn test_notch_builder_frequency_above_nyquist_should_error_or_fail_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        // Nyquist for 44100 is 22050. Try something clearly above it.
        // Depending on miniaudio, this may error at init or may clamp/behave oddly.
        // We accept either outcome, but it should not UB/panic.
        let res = NotchNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1.0, 50_000.0).build();

        // Prefer error, but allow Ok if miniaudio clamps internally.
        // If it clamps, this assert can be changed to `assert!(res.is_ok())` once confirmed.
        if let Ok(mut node) = res {
            let cfg = NotchNodeParams::new(&node, 1.0, 10_000.0);
            node.reinit(&cfg).unwrap();
        }
    }

    #[test]
    fn test_notch_builder_q_zero_should_error_or_fail_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        // Q=0 is maybe typically invalid (division by zero-ish in coefficient derivation).
        let res = NotchNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 0.0, 1000.0).build();

        // TODO: But is ok?
        assert!(res.is_ok());
    }
}
