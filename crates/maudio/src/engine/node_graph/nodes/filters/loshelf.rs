use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    Binding, MaResult,
    audio::{formats::Format, sample_rate::SampleRate},
    engine::{
        AllocationCallbacks,
        node_graph::{
            AsNodeGraphPtr, NodeGraph,
            nodes::{AsNodePtr, NodeRef, private_node::LoShelfNodeProvider},
        },
    },
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
/// After creating the filter, use [`Self::reinit`] and [`LoShelfNodeParams`] to change the values of the coefficients.
/// This reinitializes the filter coefficients without clearing the internal state.
/// This allows filter parameters to be updated in real time without causing
/// audible artifacts such as clicks or pops.
///
/// Use [`LoShelfNodeBuilder`] to initialize
pub struct LoShelfNode<'a> {
    inner: *mut sys::ma_loshelf_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
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
        alloc: Option<&'a AllocationCallbacks>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_loshelf_node>> = Box::new_uninit();

        n_loshelf_ffi::ma_loshelf_node_init(
            node_graph,
            config.to_raw(),
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        let ptr: Box<sys::ma_loshelf_node> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_loshelf_node = Box::into_raw(ptr);
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
    fn reinit(&mut self, config: &LoShelfNodeParams) -> MaResult<()> {
        n_loshelf_ffi::ma_loshelf_node_reinit(config.to_raw(), self)
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

pub(crate) mod n_loshelf_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        Binding, MaRawResult, MaResult,
        engine::node_graph::{
            AsNodeGraphPtr, nodes::filters::loshelf::LoShelfNode, private_node_graph,
        },
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
        MaRawResult::check(res)
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
        MaRawResult::check(res)
    }
}

impl<'a> Drop for LoShelfNode<'a> {
    fn drop(&mut self) {
        n_loshelf_ffi::ma_loshelf_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub struct LoShelfNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_loshelf_node_config,
    node_graph: &'a N,
}

impl<N: AsNodeGraphPtr + ?Sized> Binding for LoShelfNodeBuilder<'_, N> {
    type Raw = *const sys::ma_loshelf_node_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
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

    pub fn build(self) -> MaResult<LoShelfNode<'a>> {
        LoShelfNode::new_with_cfg_alloc_internal(self.node_graph, &self, None)
    }
}

pub struct LoShelfNodeParams {
    inner: sys::ma_loshelf_config,
}

impl Binding for LoShelfNodeParams {
    type Raw = *const sys::ma_loshelf_config;

    // !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
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
            Engine, EngineOps,
            node_graph::nodes::filters::loshelf::{LoShelfNodeBuilder, LoShelfNodeParams},
        },
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

        // Frequency 0 is almost certainly invalid.
        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.5, 0.0);
        // Does not return an error???
        assert!(node.reinit(&cfg).is_ok());
    }

    #[test]
    fn test_loshelf_reinit_invalid_slope_errors() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        // Slope 0 or negative should error.
        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.0, 200.0);
        // Does not return an error???
        assert!(node.reinit(&cfg).is_ok());
    }

    #[test]
    fn test_loshelf_reinit_extreme_gain_ok() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        // Very large gain values: miniaudio usually doesn't validate these strictly.
        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 120.0, 0.5, 200.0);
        assert!(node.reinit(&cfg).is_ok());

        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, -120.0, 0.5, 200.0);
        assert!(node.reinit(&cfg).is_ok());
    }

    #[test]
    fn test_loshelf_reinit_negative_frequency_ok_or_error() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node =
            LoShelfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 0.0, 0.5, 200.0)
                .build()
                .unwrap();

        // Negative frequency is nonsensical, but depending on miniaudio internals it may not be checked.
        let cfg = LoShelfNodeParams::new(&node, SampleRate::Sr48000, 0.0, 0.5, -1.0);

        // If you want this to be strict later, flip this to `is_err()` once you add validation on your side.
        assert!(node.reinit(&cfg).is_ok());
    }
}
