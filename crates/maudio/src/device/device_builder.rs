use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    panic::{catch_unwind, AssertUnwindSafe},
    slice,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use maudio_sys::ffi as sys;

use crate::{
    audio::{
        channels::{Channel, ChannelMixMode},
        performance::PerformanceProfile,
        sample_rate::SampleRate,
    },
    backend::Backend,
    device::{
        device_id::DeviceId,
        device_type::{DeviceShareMode, DeviceType},
        private_device, AsDevicePtr, Device,
    },
    pcm_frames::{PcmFormat, S24Packed},
    AsRawRef, Binding, MaResult,
};

pub struct DeviceBuilder {}

struct Unknown {}

struct PlayBackDeviceBuilder<'a, F = Unknown> {
    inner: sys::ma_device_config,
    device_type: DeviceType,
    backends: Option<&'a [Backend]>,
    data_callback_info: Option<DeviceBuilderDataCallBack>,
    _format: PhantomData<F>,
}

struct CaptureDeviceBuilder<'a, F = Unknown> {
    inner: sys::ma_device_config,
    device_type: DeviceType,
    backends: Option<&'a [Backend]>,
    data_callback_info: Option<DeviceBuilderDataCallBack>,
    _format: PhantomData<F>,
}

struct DuplexDeviceBuilder<'a, F = Unknown> {
    inner: sys::ma_device_config,
    device_type: DeviceType,
    backends: Option<&'a [Backend]>,
    data_callback_info: Option<DeviceBuilderDataCallBack>,
    _format: PhantomData<F>,
}

struct LoopBackDeviceBuilder<'a, F = Unknown> {
    inner: sys::ma_device_config,
    device_type: DeviceType,
    backends: Option<&'a [Backend]>,
    data_callback_info: Option<DeviceBuilderDataCallBack>,
    _format: PhantomData<F>,
}

#[derive(Clone)]
pub(crate) struct DeviceBuilderDataCallBack {
    pub(crate) data_callback: *mut core::ffi::c_void, // example: *mut LoopBackDeviceState<F, C>
    pub(crate) data_callback_drop: fn(*mut core::ffi::c_void),
    pub(crate) data_callback_panic: Arc<AtomicBool>,
}

impl<F> AsRawRef for PlayBackDeviceBuilder<'_, F> {
    type Raw = sys::ma_device_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<F> AsRawRef for CaptureDeviceBuilder<'_, F> {
    type Raw = sys::ma_device_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<F> AsRawRef for DuplexDeviceBuilder<'_, F> {
    type Raw = sys::ma_device_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<F> AsRawRef for LoopBackDeviceBuilder<'_, F> {
    type Raw = sys::ma_device_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

pub trait AsDeviceBuilder<'a> {
    type _DeviceBuilderProvider: private_device_b::DeviceBulderProvider<'a, Self>;
}

impl<'a, F: PcmFormat> AsDeviceBuilder<'a> for PlayBackDeviceBuilder<'a, F> {
    type _DeviceBuilderProvider = private_device_b::PlayBackDeviceBuilderProvider;
}

impl<'a, F: PcmFormat> AsDeviceBuilder<'a> for CaptureDeviceBuilder<'a, F> {
    type _DeviceBuilderProvider = private_device_b::CaptureDeviceBuilderProvider;
}

impl<'a, F: PcmFormat> AsDeviceBuilder<'a> for DuplexDeviceBuilder<'a, F> {
    type _DeviceBuilderProvider = private_device_b::DuplexDeviceBuilderProvider;
}

impl<'a, F: PcmFormat> AsDeviceBuilder<'a> for LoopBackDeviceBuilder<'a, F> {
    type _DeviceBuilderProvider = private_device_b::LoopBackDeviceBuilderProvider;
}

pub(crate) mod private_device_b {
    use super::*;
    use maudio_sys::ffi as sys;

    // Will not allow playback or capture methods on types that don't support them.
    // The methods will still be available to the user, but they will get a compiler error
    // (tradeoff to limit code duplication and imports).
    pub trait SupportsPlayBack {}
    pub trait SupportsCapture {}

    impl<'a, F: PcmFormat> SupportsPlayBack for PlayBackDeviceBuilder<'a, F> {}
    impl<'a, F: PcmFormat> SupportsCapture for CaptureDeviceBuilder<'a, F> {}
    impl<'a, F: PcmFormat> SupportsPlayBack for DuplexDeviceBuilder<'a, F> {}
    impl<'a, F: PcmFormat> SupportsCapture for DuplexDeviceBuilder<'a, F> {}
    impl<'a, F: PcmFormat> SupportsCapture for LoopBackDeviceBuilder<'a, F> {}

    pub trait DeviceBulderProvider<'a, T: ?Sized> {
        fn set_backends<'s>(t: &'s mut T, backends: &'a [Backend]);
        fn get_backends(t: &T) -> Option<&[Backend]>;
        fn get_callback_info(t: &T) -> Option<DeviceBuilderDataCallBack>;
        fn inner(t: &mut T) -> &mut sys::ma_device_config;
        fn as_raw(t: &'a T) -> &'a sys::ma_device_config;
        fn as_raw_ptr(t: &T) -> *const sys::ma_device_config;
    }

    pub struct PlayBackDeviceBuilderProvider;
    pub struct CaptureDeviceBuilderProvider;
    pub struct DuplexDeviceBuilderProvider;
    pub struct LoopBackDeviceBuilderProvider;

    impl<'a, F: PcmFormat> DeviceBulderProvider<'a, PlayBackDeviceBuilder<'a, F>>
        for PlayBackDeviceBuilderProvider
    {
        fn set_backends<'s>(t: &'s mut PlayBackDeviceBuilder<'a, F>, backends: &'a [Backend]) {
            t.backends = Some(backends);
        }

        fn get_backends<'s>(t: &'s PlayBackDeviceBuilder<'a, F>) -> Option<&'s [Backend]> {
            t.backends
        }

        fn get_callback_info(
            t: &PlayBackDeviceBuilder<'a, F>,
        ) -> Option<DeviceBuilderDataCallBack> {
            t.data_callback_info.clone()
        }

        fn inner<'s>(t: &'s mut PlayBackDeviceBuilder<'a, F>) -> &'s mut sys::ma_device_config {
            &mut t.inner
        }

        fn as_raw(t: &'a PlayBackDeviceBuilder<'a, F>) -> &'a sys::ma_device_config {
            t.as_raw()
        }

        fn as_raw_ptr(t: &PlayBackDeviceBuilder<'a, F>) -> *const sys::ma_device_config {
            t.as_raw_ptr()
        }
    }

    impl<'a, F: PcmFormat> DeviceBulderProvider<'a, CaptureDeviceBuilder<'a, F>>
        for CaptureDeviceBuilderProvider
    {
        fn set_backends<'s>(t: &'s mut CaptureDeviceBuilder<'a, F>, backends: &'a [Backend]) {
            t.backends = Some(backends);
        }

        fn get_backends<'s>(t: &'s CaptureDeviceBuilder<'a, F>) -> Option<&'s [Backend]> {
            t.backends
        }

        fn get_callback_info(t: &CaptureDeviceBuilder<'a, F>) -> Option<DeviceBuilderDataCallBack> {
            t.data_callback_info.clone()
        }

        fn inner<'s>(t: &'s mut CaptureDeviceBuilder<'a, F>) -> &'s mut sys::ma_device_config {
            &mut t.inner
        }

        fn as_raw(t: &'a CaptureDeviceBuilder<'a, F>) -> &'a sys::ma_device_config {
            t.as_raw()
        }

        fn as_raw_ptr(t: &CaptureDeviceBuilder<'a, F>) -> *const sys::ma_device_config {
            t.as_raw_ptr()
        }
    }

    impl<'a, F: PcmFormat> DeviceBulderProvider<'a, DuplexDeviceBuilder<'a, F>>
        for DuplexDeviceBuilderProvider
    {
        fn set_backends<'s>(t: &'s mut DuplexDeviceBuilder<'a, F>, backends: &'a [Backend]) {
            t.backends = Some(backends);
        }

        fn get_backends<'s>(t: &'s DuplexDeviceBuilder<'a, F>) -> Option<&'s [Backend]> {
            t.backends
        }

        fn get_callback_info(t: &DuplexDeviceBuilder<'a, F>) -> Option<DeviceBuilderDataCallBack> {
            t.data_callback_info.clone()
        }

        fn inner<'s>(t: &'s mut DuplexDeviceBuilder<'a, F>) -> &'s mut sys::ma_device_config {
            &mut t.inner
        }

        fn as_raw(t: &'a DuplexDeviceBuilder<'a, F>) -> &'a sys::ma_device_config {
            t.as_raw()
        }

        fn as_raw_ptr(t: &DuplexDeviceBuilder<'a, F>) -> *const sys::ma_device_config {
            t.as_raw_ptr()
        }
    }

    impl<'a, F: PcmFormat> DeviceBulderProvider<'a, LoopBackDeviceBuilder<'a, F>>
        for LoopBackDeviceBuilderProvider
    {
        fn set_backends<'s>(t: &'s mut LoopBackDeviceBuilder<'a, F>, backends: &'a [Backend]) {
            t.backends = Some(backends);
        }

        fn get_backends<'s>(t: &'s LoopBackDeviceBuilder<'a, F>) -> Option<&'s [Backend]> {
            t.backends
        }

        fn get_callback_info(
            t: &LoopBackDeviceBuilder<'a, F>,
        ) -> Option<DeviceBuilderDataCallBack> {
            t.data_callback_info.clone()
        }

        fn inner<'s>(t: &'s mut LoopBackDeviceBuilder<'a, F>) -> &'s mut sys::ma_device_config {
            &mut t.inner
        }

        fn as_raw(t: &'a LoopBackDeviceBuilder<'a, F>) -> &'a sys::ma_device_config {
            t.as_raw()
        }

        fn as_raw_ptr(t: &LoopBackDeviceBuilder<'a, F>) -> *const sys::ma_device_config {
            t.as_raw_ptr()
        }
    }

    pub fn set_backends<'a, 's, T: AsDeviceBuilder<'a> + ?Sized>(
        t: &'s mut T,
        backends: &'a [Backend],
    ) {
        <T as AsDeviceBuilder>::_DeviceBuilderProvider::set_backends(t, backends);
    }

    pub fn get_backends<'a, 's, T: AsDeviceBuilder<'a> + ?Sized>(
        t: &'s T,
    ) -> Option<&'s [Backend]> {
        <T as AsDeviceBuilder>::_DeviceBuilderProvider::get_backends(t)
    }

    pub fn get_data_callback_info<'a, T: AsDeviceBuilder<'a> + ?Sized>(
        t: &T,
    ) -> Option<DeviceBuilderDataCallBack> {
        <T as AsDeviceBuilder>::_DeviceBuilderProvider::get_callback_info(t)
    }

    pub fn inner<'a, 's, T: AsDeviceBuilder<'a> + ?Sized>(
        t: &'s mut T,
    ) -> &'s mut sys::ma_device_config {
        <T as AsDeviceBuilder<'a>>::_DeviceBuilderProvider::inner(t)
    }

    pub fn as_raw<'a, T: AsDeviceBuilder<'a> + ?Sized>(t: &'a T) -> &'a sys::ma_device_config {
        <T as AsDeviceBuilder>::_DeviceBuilderProvider::as_raw(t)
    }

    pub fn as_raw_ptr<'a, T: AsDeviceBuilder<'a> + ?Sized>(t: &T) -> *const sys::ma_device_config {
        <T as AsDeviceBuilder>::_DeviceBuilderProvider::as_raw_ptr(t)
    }
}

impl<'a> PlayBackDeviceBuilder<'a, Unknown> {
    fn new_inner<F: PcmFormat>(&self) -> PlayBackDeviceBuilder<'a, F> {
        PlayBackDeviceBuilder {
            inner: self.inner,
            backends: self.backends,
            device_type: DeviceType::PlayBack,
            data_callback_info: None,
            _format: PhantomData,
        }
    }

    fn u8(&mut self) -> PlayBackDeviceBuilder<'a, u8> {
        self.inner.playback.format = sys::ma_format_ma_format_u8;
        self.new_inner::<u8>()
    }

    fn i16(&mut self) -> PlayBackDeviceBuilder<'a, i16> {
        self.inner.playback.format = sys::ma_format_ma_format_s16;
        self.new_inner::<i16>()
    }

    fn i32(&mut self) -> PlayBackDeviceBuilder<'a, i32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<i32>()
    }

    fn s24_packed(&mut self) -> PlayBackDeviceBuilder<'a, S24Packed> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<S24Packed>()
    }

    fn f32(&mut self) -> PlayBackDeviceBuilder<'a, f32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<f32>()
    }
}

impl<'a> CaptureDeviceBuilder<'a, Unknown> {
    fn new_inner<F: PcmFormat>(&self) -> CaptureDeviceBuilder<'a, F> {
        CaptureDeviceBuilder {
            inner: self.inner,
            backends: self.backends,
            device_type: DeviceType::Capture,
            data_callback_info: None,
            _format: PhantomData,
        }
    }

    fn u8(&mut self) -> CaptureDeviceBuilder<'a, u8> {
        self.inner.playback.format = sys::ma_format_ma_format_u8;
        self.new_inner::<u8>()
    }

    fn i16(&mut self) -> CaptureDeviceBuilder<'a, i16> {
        self.inner.playback.format = sys::ma_format_ma_format_s16;
        self.new_inner::<i16>()
    }

    fn i32(&mut self) -> CaptureDeviceBuilder<'a, i32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<i32>()
    }

    fn s24_packed(&mut self) -> CaptureDeviceBuilder<'a, S24Packed> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<S24Packed>()
    }

    fn f32(&mut self) -> CaptureDeviceBuilder<'a, f32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<f32>()
    }
}

impl<'a> DuplexDeviceBuilder<'a, Unknown> {
    fn new_inner<F: PcmFormat>(&self) -> DuplexDeviceBuilder<'a, F> {
        DuplexDeviceBuilder {
            inner: self.inner,
            backends: self.backends,
            device_type: DeviceType::Duplex,
            data_callback_info: None,
            _format: PhantomData,
        }
    }

    fn u8(&mut self) -> DuplexDeviceBuilder<'a, u8> {
        self.inner.playback.format = sys::ma_format_ma_format_u8;
        self.new_inner::<u8>()
    }

    fn i16(&mut self) -> DuplexDeviceBuilder<'a, i16> {
        self.inner.playback.format = sys::ma_format_ma_format_s16;
        self.new_inner::<i16>()
    }

    fn i32(&mut self) -> DuplexDeviceBuilder<'a, i32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<i32>()
    }

    fn s24_packed(&mut self) -> DuplexDeviceBuilder<'a, S24Packed> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<S24Packed>()
    }

    fn f32(&mut self) -> DuplexDeviceBuilder<'a, f32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<f32>()
    }
}

impl<'a> LoopBackDeviceBuilder<'a, Unknown> {
    fn new_inner<F: PcmFormat>(&self) -> LoopBackDeviceBuilder<'a, F> {
        LoopBackDeviceBuilder {
            inner: self.inner,
            backends: self.backends,
            device_type: DeviceType::LoopBack,
            data_callback_info: None,
            _format: PhantomData,
        }
    }

    fn u8(&mut self) -> LoopBackDeviceBuilder<'a, u8> {
        self.inner.playback.format = sys::ma_format_ma_format_u8;
        self.new_inner::<u8>()
    }

    fn i16(&mut self) -> LoopBackDeviceBuilder<'a, i16> {
        self.inner.playback.format = sys::ma_format_ma_format_s16;
        self.new_inner::<i16>()
    }

    fn i32(&mut self) -> LoopBackDeviceBuilder<'a, i32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<i32>()
    }

    fn s24_packed(&mut self) -> LoopBackDeviceBuilder<'a, S24Packed> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<S24Packed>()
    }

    fn f32(&mut self) -> LoopBackDeviceBuilder<'a, f32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<f32>()
    }
}

impl<'a, F: PcmFormat> DeviceBuilderOps<'a> for PlayBackDeviceBuilder<'a, F> {}
impl<'a, F: PcmFormat> DeviceBuilderOps<'a> for CaptureDeviceBuilder<'a, F> {}
impl<'a, F: PcmFormat> DeviceBuilderOps<'a> for DuplexDeviceBuilder<'a, F> {}
impl<'a, F: PcmFormat> DeviceBuilderOps<'a> for LoopBackDeviceBuilder<'a, F> {}

pub trait DeviceBuilderOps<'a>: AsDeviceBuilder<'a> {
    fn playback_device_id(&mut self, device_id: &DeviceId) -> &mut Self
    where
        Self: private_device_b::SupportsPlayBack,
    {
        private_device_b::inner(self).playback.pDeviceID = device_id.as_raw_ptr();
        self
    }

    fn playback_mix_mode(&mut self, mode: ChannelMixMode) -> &mut Self
    where
        Self: private_device_b::SupportsPlayBack,
    {
        private_device_b::inner(self).playback.channelMixMode = mode.into();
        self
    }

    fn playback_channels(&mut self, channels: u32) -> &mut Self
    where
        Self: private_device_b::SupportsPlayBack,
    {
        private_device_b::inner(self).playback.channels = channels;
        self
    }

    fn playback_channel_map(&mut self, map: &[Channel]) -> &mut Self
    where
        Self: private_device_b::SupportsPlayBack,
    {
        private_device_b::inner(self).playback.pChannelMap = map.as_ptr() as *mut _;
        private_device_b::inner(self).playback.channels = map.len() as _;
        self
    }

    fn playback_share_mode(&mut self, mode: DeviceShareMode) -> &mut Self
    where
        Self: private_device_b::SupportsPlayBack,
    {
        private_device_b::inner(self).playback.shareMode = mode.into();
        self
    }

    fn capture_device_id(&mut self, device_id: &DeviceId) -> &mut Self
    where
        Self: private_device_b::SupportsCapture,
    {
        private_device_b::inner(self).capture.pDeviceID = device_id.as_raw_ptr();
        self
    }

    fn capture_mix_mode(&mut self, mode: ChannelMixMode) -> &mut Self
    where
        Self: private_device_b::SupportsCapture,
    {
        private_device_b::inner(self).capture.channelMixMode = mode.into();
        self
    }

    fn capture_channels(&mut self, channels: u32) -> &mut Self
    where
        Self: private_device_b::SupportsCapture,
    {
        private_device_b::inner(self).capture.channels = channels;
        self
    }

    fn capture_channel_map(&mut self, map: &[Channel]) -> &mut Self
    where
        Self: private_device_b::SupportsCapture,
    {
        private_device_b::inner(self).capture.pChannelMap = map.as_ptr() as *mut _;
        private_device_b::inner(self).capture.channels = map.len() as _;
        self
    }

    fn capture_share_mode(&mut self, mode: DeviceShareMode) -> &mut Self
    where
        Self: private_device_b::SupportsCapture,
    {
        private_device_b::inner(self).capture.shareMode = mode.into();
        self
    }

    fn performance_profile(&mut self, profile: PerformanceProfile) -> &mut Self {
        private_device_b::inner(self).performanceProfile = profile.into();
        self
    }

    /// When set to true, the contents of the output buffer passed into the data callback will not be clipped after returning. Only applies when the playback sample format is f32.
    ///
    /// Set to false by default
    fn clipping(&mut self, yes: bool) -> &mut Self {
        private_device_b::inner(self).noClip = yes as u8;
        self
    }

    /// When set to true, the contents of the output buffer passed into the data callback will be left undefined rather than initialized to silence.
    ///
    /// Set to false by default
    fn pre_silenced_output(&mut self, yes: bool) -> &mut Self {
        private_device_b::inner(self).noPreSilencedOutputBuffer = yes as u8;
        self
    }

    fn sample_rate(&mut self, sample_rate: SampleRate) -> &mut Self {
        private_device_b::inner(self).sampleRate = sample_rate.into();
        self
    }

    fn backends(&mut self, backends: &'a [Backend]) -> &mut Self {
        private_device_b::set_backends(self, backends);
        self
    }
}

impl<'a> DeviceBuilder {
    fn playback() -> PlayBackDeviceBuilder<'a, Unknown> {
        let ptr = unsafe { sys::ma_device_config_init(DeviceType::PlayBack.into()) };
        PlayBackDeviceBuilder {
            inner: ptr,
            backends: None,
            device_type: DeviceType::PlayBack,
            data_callback_info: None,
            _format: PhantomData,
        }
    }

    fn capture() -> CaptureDeviceBuilder<'a, Unknown> {
        let ptr = unsafe { sys::ma_device_config_init(DeviceType::Capture.into()) };
        CaptureDeviceBuilder {
            inner: ptr,
            backends: None,
            device_type: DeviceType::Capture,
            data_callback_info: None,
            _format: PhantomData,
        }
    }

    fn duplex() -> DuplexDeviceBuilder<'a, Unknown> {
        let ptr = unsafe { sys::ma_device_config_init(DeviceType::Duplex.into()) };
        DuplexDeviceBuilder {
            inner: ptr,
            backends: None,
            device_type: DeviceType::Duplex,
            data_callback_info: None,
            _format: PhantomData,
        }
    }

    fn loopback() -> LoopBackDeviceBuilder<'a, Unknown> {
        let ptr = unsafe { sys::ma_device_config_init(DeviceType::LoopBack.into()) };
        LoopBackDeviceBuilder {
            inner: ptr,
            backends: None,
            device_type: DeviceType::LoopBack,
            data_callback_info: None,
            _format: PhantomData,
        }
    }
}

impl<'a, F: PcmFormat> PlayBackDeviceBuilder<'a, F> {
    fn with_callback<C>(&mut self, f: C) -> MaResult<Device>
    where
        C: FnMut(CallBackDevice, &mut [F::StorageUnit], u32) + Send + 'static,
    {
        let panic_flag = Arc::new(AtomicBool::new(false));
        let state: PlayBackDeviceState<F, C> = PlayBackDeviceState {
            f: UnsafeCell::new(f),
            panic_flag: panic_flag.clone(),
            _format: PhantomData,
        };

        let state_box = Box::new(state);
        let state_ptr: *mut PlayBackDeviceState<F, C> = Box::into_raw(state_box);
        let callback_info: DeviceBuilderDataCallBack = DeviceBuilderDataCallBack {
            data_callback: state_ptr.cast(),
            data_callback_drop: drop_playback_device_state::<F, C>,
            data_callback_panic: panic_flag,
        };

        self.data_callback_info = Some(callback_info);

        self.inner.dataCallback = Some(device_data_playback_callback::<F, C>);
        self.inner.pUserData = state_ptr as *mut core::ffi::c_void;

        Device::new_with_config(self, None, private_device_b::get_backends(self))
    }
}

impl<'a, F: PcmFormat> CaptureDeviceBuilder<'a, F> {
    fn with_callback<C>(&mut self, f: C) -> MaResult<Device>
    where
        C: FnMut(CallBackDevice, &[F::StorageUnit], u32) + Send + 'static,
    {
        let panic_flag = Arc::new(AtomicBool::new(false));
        let state: CaptureDeviceState<F, C> = CaptureDeviceState {
            f: UnsafeCell::new(f),
            panic_flag: panic_flag.clone(),
            _format: PhantomData,
        };
        let state_box = Box::new(state);
        let state_ptr: *mut CaptureDeviceState<F, C> = Box::into_raw(state_box);
        let callback_info: DeviceBuilderDataCallBack = DeviceBuilderDataCallBack {
            data_callback: state_ptr.cast(),
            data_callback_drop: drop_playback_device_state::<F, C>,
            data_callback_panic: panic_flag,
        };

        self.data_callback_info = Some(callback_info);

        self.inner.dataCallback = Some(device_data_capture_callback::<F, C>);
        self.inner.pUserData = state_ptr as *mut core::ffi::c_void;

        Device::new_with_config(self, None, private_device_b::get_backends(self))
    }
}

impl<'a, F: PcmFormat> DuplexDeviceBuilder<'a, F> {
    fn with_callback<C>(&mut self, f: C) -> MaResult<Device>
    where
        C: FnMut(CallBackDevice, &mut [F::StorageUnit], &[F::StorageUnit], u32) + Send + 'static,
    {
        let panic_flag = Arc::new(AtomicBool::new(false));
        let state: DuplexDeviceState<F, C> = DuplexDeviceState {
            f: UnsafeCell::new(f),
            panic_flag: panic_flag.clone(),
            _format: PhantomData,
        };
        let state_box = Box::new(state);
        let state_ptr: *mut DuplexDeviceState<F, C> = Box::into_raw(state_box);
        let callback_info: DeviceBuilderDataCallBack = DeviceBuilderDataCallBack {
            data_callback: state_ptr.cast(),
            data_callback_drop: drop_playback_device_state::<F, C>,
            data_callback_panic: panic_flag,
        };

        self.data_callback_info = Some(callback_info);

        self.inner.dataCallback = Some(device_data_duplex_callback::<F, C>);
        self.inner.pUserData = state_ptr as *mut core::ffi::c_void;

        Device::new_with_config(self, None, private_device_b::get_backends(self))
    }
}

impl<'a, F: PcmFormat> LoopBackDeviceBuilder<'a, F> {
    fn with_callback<C>(&mut self, f: C) -> MaResult<Device>
    where
        C: FnMut(CallBackDevice, &[F::StorageUnit], u32) + Send + 'static,
    {
        let panic_flag = Arc::new(AtomicBool::new(false));
        let state: LoopBackDeviceState<F, C> = LoopBackDeviceState {
            f: UnsafeCell::new(f),
            panic_flag: panic_flag.clone(),
            _format: PhantomData,
        };
        let state_box = Box::new(state);
        let state_ptr: *mut LoopBackDeviceState<F, C> = Box::into_raw(state_box);
        let callback_info: DeviceBuilderDataCallBack = DeviceBuilderDataCallBack {
            data_callback: state_ptr.cast(),
            data_callback_drop: drop_playback_device_state::<F, C>,
            data_callback_panic: panic_flag,
        };

        self.data_callback_info = Some(callback_info);
        println!("1");

        self.inner.dataCallback = Some(device_data_loopback_callback::<F, C>);
        self.inner.pUserData = state_ptr as *mut core::ffi::c_void;
        println!("2");

        Device::new_with_config(self, None, private_device_b::get_backends(self))
    }
}

/// Device that lives inside the data callback
///
/// Provides limited access only to functions safe to call from inside the audio callback
pub(crate) struct CallBackDevice {
    inner: *mut sys::ma_device,
}

impl Binding for CallBackDevice {
    type Raw = *mut sys::ma_device;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self { inner: raw }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl AsDevicePtr for CallBackDevice {
    type __PtrProvider = private_device::CallBackDeviceRefProvider;
}

struct PlayBackDeviceState<F: PcmFormat, C> {
    f: UnsafeCell<C>,
    panic_flag: Arc<AtomicBool>,
    _format: PhantomData<F>,
}

struct CaptureDeviceState<F: PcmFormat, C> {
    f: UnsafeCell<C>,
    panic_flag: Arc<AtomicBool>,
    _format: PhantomData<F>,
}

struct DuplexDeviceState<F: PcmFormat, C> {
    f: UnsafeCell<C>,
    panic_flag: Arc<AtomicBool>,
    _format: PhantomData<F>,
}

struct LoopBackDeviceState<F: PcmFormat, C> {
    f: UnsafeCell<C>,
    panic_flag: Arc<AtomicBool>,
    _format: PhantomData<F>,
}

unsafe extern "C" fn device_data_playback_callback<F: PcmFormat, C>(
    device: *mut sys::ma_device,
    output: *mut core::ffi::c_void,
    _input: *const core::ffi::c_void,
    frame_count: u32,
) where
    C: FnMut(CallBackDevice, &mut [F::StorageUnit], u32) + Send + 'static,
{
    if device.is_null() {
        return;
    }

    let user_data = (*device).pUserData;
    if user_data.is_null() {
        return;
    }

    if frame_count == 0 || output.is_null() {
        return;
    }

    let cb_device = CallBackDevice::from_ptr(device);
    let state = &*((*device).pUserData as *const PlayBackDeviceState<F, C>);

    let channels = (*device).playback.channels;
    let Some(slice_len) = (frame_count as usize).checked_mul(channels as usize) else {
        return;
    };
    let Some(slice_len) = slice_len.checked_mul(<F as PcmFormat>::VEC_STORE_UNITS_PER_FRAME) else {
        return;
    };
    let slice = unsafe { slice::from_raw_parts_mut(output.cast::<F::StorageUnit>(), slice_len) };

    if state.panic_flag.load(Ordering::Relaxed) {
        slice.fill(F::SILENCE);
        return;
    }

    let cb = &mut *state.f.get();
    let res = catch_unwind(AssertUnwindSafe(|| (cb)(cb_device, slice, frame_count)));
    if res.is_err() {
        state.panic_flag.store(true, Ordering::Release);
        slice.fill(F::SILENCE);
    }
}

unsafe extern "C" fn device_data_capture_callback<F: PcmFormat, C>(
    device: *mut sys::ma_device,
    _output: *mut core::ffi::c_void,
    input: *const core::ffi::c_void,
    frame_count: u32,
) where
    C: FnMut(CallBackDevice, &[F::StorageUnit], u32) + Send + 'static,
{
    if device.is_null() {
        return;
    }

    let user_data = (*device).pUserData;
    if user_data.is_null() {
        return;
    }

    if frame_count == 0 || input.is_null() {
        return;
    }

    let cb_device = CallBackDevice::from_ptr(device);
    let state = &*((*device).pUserData as *const CaptureDeviceState<F, C>);

    if state.panic_flag.load(Ordering::Relaxed) {
        return;
    }

    let channels = (*device).capture.channels;
    let Some(slice_len) = (frame_count as usize).checked_mul(channels as usize) else {
        return;
    };
    let Some(slice_len) = slice_len.checked_mul(<F as PcmFormat>::VEC_STORE_UNITS_PER_FRAME) else {
        return;
    };
    let slice = unsafe { slice::from_raw_parts(input.cast::<F::StorageUnit>(), slice_len) };

    let cb = &mut *state.f.get();
    let res = catch_unwind(AssertUnwindSafe(|| (cb)(cb_device, slice, frame_count)));
    if res.is_err() {
        state.panic_flag.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn device_data_duplex_callback<F: PcmFormat, C>(
    device: *mut sys::ma_device,
    output: *mut core::ffi::c_void,
    input: *const core::ffi::c_void,
    frame_count: u32,
) where
    C: FnMut(CallBackDevice, &mut [F::StorageUnit], &[F::StorageUnit], u32) + Send + 'static,
{
    if device.is_null() {
        return;
    }

    let user_data = (*device).pUserData;
    if user_data.is_null() {
        return;
    }

    if frame_count == 0 || input.is_null() || output.is_null() {
        return;
    }

    let cb_device = CallBackDevice::from_ptr(device);
    let state = &*((*device).pUserData as *const DuplexDeviceState<F, C>);

    let channels = (*device).playback.channels; // TODO
    let Some(slice_len) = (frame_count as usize).checked_mul(channels as usize) else {
        return;
    };
    let Some(slice_len) = slice_len.checked_mul(<F as PcmFormat>::VEC_STORE_UNITS_PER_FRAME) else {
        return;
    };
    let out_slice =
        unsafe { slice::from_raw_parts_mut(output.cast::<F::StorageUnit>(), slice_len) };
    let in_slice = unsafe { slice::from_raw_parts(input.cast::<F::StorageUnit>(), slice_len) };

    if state.panic_flag.load(Ordering::Relaxed) {
        out_slice.fill(F::SILENCE);
        return;
    }

    let cb = &mut *state.f.get();
    let res = catch_unwind(AssertUnwindSafe(|| {
        (cb)(cb_device, out_slice, in_slice, frame_count)
    }));
    if res.is_err() {
        state.panic_flag.store(true, Ordering::Release);
        out_slice.fill(F::SILENCE);
    }
}

unsafe extern "C" fn device_data_loopback_callback<F: PcmFormat, C>(
    device: *mut sys::ma_device,
    _output: *mut core::ffi::c_void,
    input: *const core::ffi::c_void,
    frame_count: u32,
) where
    C: FnMut(CallBackDevice, &[F::StorageUnit], u32) + Send + 'static,
{
    if device.is_null() {
        return;
    }

    let user_data = (*device).pUserData;
    if user_data.is_null() {
        return;
    }

    if frame_count == 0 || input.is_null() {
        return;
    }

    let cb_device = CallBackDevice::from_ptr(device);
    let state = &*((*device).pUserData as *const LoopBackDeviceState<F, C>);

    if state.panic_flag.load(Ordering::Relaxed) {
        return;
    }

    let channels = (*device).capture.channels;
    let Some(slice_len) = (frame_count as usize).checked_mul(channels as usize) else {
        return;
    };
    let Some(slice_len) = slice_len.checked_mul(<F as PcmFormat>::VEC_STORE_UNITS_PER_FRAME) else {
        return;
    };
    let slice = unsafe { slice::from_raw_parts(input.cast::<F::StorageUnit>(), slice_len) };

    let cb = &mut *state.f.get();
    let res = catch_unwind(AssertUnwindSafe(|| (cb)(cb_device, slice, frame_count)));
    if res.is_err() {
        state.panic_flag.store(true, Ordering::Release);
    }
}

fn drop_playback_device_state<F: PcmFormat, C>(ptr: *mut core::ffi::c_void) {
    let state: Box<PlayBackDeviceState<F, C>> =
        unsafe { Box::from_raw(ptr as *mut PlayBackDeviceState<F, C>) };
    drop(state);
}

fn drop_capture_device_state<F: PcmFormat, C>(ptr: *mut core::ffi::c_void) {
    let state: Box<CaptureDeviceState<F, C>> =
        unsafe { Box::from_raw(ptr as *mut CaptureDeviceState<F, C>) };
    drop(state);
}

fn drop_duplex_device_state<F: PcmFormat, C>(ptr: *mut core::ffi::c_void) {
    let state: Box<DuplexDeviceState<F, C>> =
        unsafe { Box::from_raw(ptr as *mut DuplexDeviceState<F, C>) };
    drop(state);
}

fn drop_loopback_device_state<F: PcmFormat, C>(ptr: *mut core::ffi::c_void) {
    let state: Box<LoopBackDeviceState<F, C>> =
        unsafe { Box::from_raw(ptr as *mut LoopBackDeviceState<F, C>) };
    drop(state);
}

#[cfg(test)]
mod test {
    use crate::device::device_builder::{DeviceBuilder, DeviceBuilderOps};

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn test_device_builder_basic_playback_init() {
        let mut device = DeviceBuilder::playback()
            .f32()
            .playback_channels(2)
            .with_callback(|_a, b, _c| {
                b.fill(f32::default());
            })
            .unwrap();
        device.device_start().unwrap();
        device.device_stop().unwrap();
        drop(device);
    }

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn test_device_builder_basic_capture_init() {
        let mut device = DeviceBuilder::capture()
            .f32()
            .capture_channels(2)
            .with_callback(|_a, _b, _c| return)
            .unwrap();
        device.device_start().unwrap();
        device.device_stop().unwrap();
        drop(device);
    }

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn test_device_builder_basic_duplex_init() {
        let mut device = DeviceBuilder::duplex()
            .f32()
            .playback_channels(2)
            .capture_channels(2)
            .with_callback(|_a, b, _c, _d| {
                b.fill(f32::default());
            })
            .unwrap();
        device.device_start().unwrap();
        device.device_stop().unwrap();
        drop(device);
    }

    #[cfg(not(feature = "ci-tests"))]
    #[cfg(windows)]
    #[test]
    fn test_device_builder_basic_loopback_init() {
        let mut device = DeviceBuilder::loopback()
            .f32()
            .capture_channels(2)
            .with_callback(|_a, _b, _c| return)
            .unwrap();
        device.device_start().unwrap();
        device.device_stop().unwrap();
        drop(device);
    }
}
