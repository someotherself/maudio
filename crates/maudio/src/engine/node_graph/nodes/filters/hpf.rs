use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    engine::{
        node_graph::{
            nodes::{private_node::HpfNodeProvider, AsNodePtr, NodeRef},
            AsNodeGraphPtr, NodeGraph,
        },
        AllocationCallbacks,
    },
    Binding, MaResult,
};

/// A node that applies a **high-pass filter (HPF)** to an audio signal.
///
/// A high-pass filter attenuates frequencies **below** a cutoff frequency,
/// allowing higher frequencies to pass through mostly unchanged. This is commonly
/// used to remove DC offset, rumble, handling noise, or excessive low-end buildup.
///
/// `HpfNode` is a node-graph wrapper around miniaudio's HPF implementation.
/// It is designed for real-time use inside a [`NodeGraph`], and maintains internal filter state
/// across parameter changes.
///
/// ## Parameters
/// - **cutoff_frequency**: The cutoff frequency (Hz) of the high-pass filter.
/// - **order**: The filter order (slope/steepness). Higher values produce a steeper cutoff
///   at the cost of more processing and potentially more phase shift.
///
/// ## Notes
/// After creating the filter, use [`Self::reinit`] and [`HpfNodeParams`] to change the filter parameters.
/// This reinitializes the filter coefficients without clearing the internal state.
/// This allows filter parameters to be updated in real time without causing
/// audible artifacts such as clicks or pops.
///
/// Use [`HpfNodeBuilder`] to initialize
pub struct HpfNode<'a> {
    inner: *mut sys::ma_hpf_node,
    alloc_cb: Option<&'a AllocationCallbacks>,
    _marker: PhantomData<&'a NodeGraph<'a>>,
    // Below is needed during a reinit
    channels: u32,
    // format is hard coded as ma_format_f32 in miniaudio `sys::ma_hpf_node_config_init()`
    // but use value in inner.hpf.format anyway inside new_with_cfg_alloc_internal()
    format: Format,
    order: u32,
}

impl Binding for HpfNode<'_> {
    type Raw = *mut sys::ma_hpf_node;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[doc(hidden)]
impl AsNodePtr for HpfNode<'_> {
    type __PtrProvider = HpfNodeProvider;
}

impl<'a> HpfNode<'a> {
    fn new_with_cfg_alloc_internal<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: &HpfNodeBuilder<'_, N>,
        alloc: Option<&'a AllocationCallbacks>,
    ) -> MaResult<Self> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_hpf_node>> = Box::new(MaybeUninit::uninit());

        n_hpf_ffi::ma_hpf_node_init(
            node_graph,
            &config.inner as *const _,
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_hpf_node = Box::into_raw(mem) as *mut sys::ma_hpf_node;

        Ok(Self {
            inner,
            alloc_cb: alloc,
            _marker: PhantomData,
            channels: config.inner.hpf.channels,
            format: config.inner.hpf.format.try_into().unwrap_or(Format::F32),
            order: config.inner.hpf.order,
        })
    }

    /// See [`HpfNodeParams`] for creating a config
    pub fn reinit(&mut self, config: &HpfNodeParams) -> MaResult<()> {
        n_hpf_ffi::ma_hpf_node_reinit(&config.inner as *const _, self)
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

pub(crate) mod n_hpf_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        engine::node_graph::{nodes::filters::hpf::HpfNode, private_node_graph, AsNodeGraphPtr},
        Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_hpf_node_init<N: AsNodeGraphPtr + ?Sized>(
        node_graph: &N,
        config: *const sys::ma_hpf_node_config,
        alloc_cb: *const sys::ma_allocation_callbacks,
        node: *mut sys::ma_hpf_node,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_hpf_node_init(
                private_node_graph::node_graph_ptr(node_graph),
                config,
                alloc_cb,
                node,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_hpf_node_uninit(node: &mut HpfNode) {
        unsafe {
            sys::ma_hpf_node_uninit(node.to_raw(), node.alloc_cb_ptr());
        }
    }

    #[inline]
    pub fn ma_hpf_node_reinit(
        config: *const sys::ma_hpf_config,
        node: &mut HpfNode,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_hpf_node_reinit(config, node.to_raw()) };
        MaudioError::check(res)
    }
}

impl<'a> Drop for HpfNode<'a> {
    fn drop(&mut self) {
        n_hpf_ffi::ma_hpf_node_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

/// Builder for creating a [`HpfNode`]
pub struct HpfNodeBuilder<'a, N: AsNodeGraphPtr + ?Sized> {
    inner: sys::ma_hpf_node_config,
    node_graph: &'a N,
}

impl<'a, N: AsNodeGraphPtr + ?Sized> HpfNodeBuilder<'a, N> {
    // TODO: Create an enum for order???
    pub fn new(
        node_graph: &'a N,
        channels: u32,
        sample_rate: SampleRate,
        cutoff_freq: f64,
        order: u32,
    ) -> Self {
        let ptr = unsafe {
            sys::ma_hpf_node_config_init(channels, sample_rate.into(), cutoff_freq, order)
        };
        HpfNodeBuilder {
            inner: ptr,
            node_graph,
        }
    }

    pub fn build(&self) -> MaResult<HpfNode<'a>> {
        HpfNode::new_with_cfg_alloc_internal(self.node_graph, self, None)
    }
}

pub struct HpfNodeParams {
    inner: sys::ma_hpf_config,
}

impl HpfNodeParams {
    pub fn new(node: &HpfNode, sample_rate: SampleRate, cutoff_freq: f64) -> Self {
        let ptr = unsafe {
            sys::ma_hpf_config_init(
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
            node_graph::nodes::filters::hpf::{HpfNodeBuilder, HpfNodeParams},
            Engine, EngineOps,
        },
    };

    #[test]
    fn test_hpf_builder_basic_init() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = HpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr44100, 1000.0, 1)
            .build()
            .unwrap();
        let config = HpfNodeParams::new(&node, SampleRate::Sr44100, 1200.0);
        node.reinit(&config).unwrap();
    }

    #[test]
    fn test_hpf_reinit_sample_rate_change_ok() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = HpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 200.0, 2)
            .build()
            .unwrap();

        let cfg = HpfNodeParams::new(&node, SampleRate::Sr44100, 200.0);
        node.reinit(&cfg).unwrap();
    }

    #[test]
    fn test_hpf_reinit_invalid_cutoff_zero_ok_or_error() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = HpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 200.0, 2)
            .build()
            .unwrap();

        let cfg = HpfNodeParams::new(&node, SampleRate::Sr48000, 0.0);

        assert!(node.reinit(&cfg).is_ok());
    }

    #[test]
    fn test_hpf_reinit_negative_cutoff_ok_or_error() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        let mut node = HpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 200.0, 2)
            .build()
            .unwrap();

        // Negative cutoff is nonsensical, but may not be checked.
        let cfg = HpfNodeParams::new(&node, SampleRate::Sr48000, -1.0);
        assert!(node.reinit(&cfg).is_ok());
    }

    #[test]
    fn test_hpf_builder_extreme_order_init_ok_or_error() {
        let engine = Engine::new_for_tests().unwrap();
        let node_graph = engine.as_node_graph().unwrap();

        // Very high order: may succeed or may fail depending on internal allocation limits.
        let node = HpfNodeBuilder::new(&node_graph, 1, SampleRate::Sr48000, 200.0, 32).build();

        // Keep this non-strict unless you decide to clamp/validate.
        assert!(node.is_ok());
    }
}
