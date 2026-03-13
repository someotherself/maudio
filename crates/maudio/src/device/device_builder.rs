//! Builder for constructing a [`Device`]
//!
//! This module provides [`DeviceBuilder`] and related builder types for
//! constructing playback, capture, duplex, and loopback audio devices.
//!
//! Devices are configured through a typed builder API. A sample format must be
//! selected first, after which mode-specific configuration and callback
//! installation become available.
//!
//! ## Latency and period size
//!
//! Audio devices process audio in fixed-size blocks called *periods*. Each
//! period corresponds to one invocation of the device callback.
//!
//! The **period size** determines:
//!
//! - how many frames are processed per callback
//! - how often the callback runs
//! - the latency of the audio device
//!
//! Smaller periods reduce latency but require the callback to run more often.
//! Larger periods increase latency but reduce CPU overhead and improve
//! stability on slower systems.
//!
//! The period size typically determines the number of frames passed to the callback,
//! although some backends may request different sizes if variable callback sizes are enabled.
//!
//! The period size can be configured with
//! [`DeviceBuilderOps::period_size_frames`] or
//! [`DeviceBuilderOps::period_size_millis`]. If neither is specified,
//! miniaudio selects a default based on the selected performance profile.
//!
//! ## Example
//!
//! ```no_run
//!# use maudio::device::device_builder::DeviceBuilder;
//!# use crate::maudio::device::device_builder::DeviceBuilderOps;
//!# use maudio::audio::sample_rate::SampleRate;
//!
//! # fn main() -> maudio::MaResult<()> {
//! let mut device = DeviceBuilder::playback()
//!     .f32()
//!     .playback_channels(2)
//!     .sample_rate(SampleRate::Sr48000)
//!     .with_callback(|_device, output, _frame_count| {
//!         output.fill(0.0);
//!     })?;
//!
//! device.device_start()?;
//! # Ok(())
//! # }
//! ```
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
    context::ContextBuilder,
    device::{
        device_cb_notif::{
            device_notification_capture_callback, device_notification_duplex_callback,
            device_notification_loopback_callback, device_notification_playback_callback,
        },
        device_id::DeviceId,
        device_type::{DeviceShareMode, DeviceType},
        private_device, AsDevicePtr, Device,
    },
    pcm_frames::{PcmFormat, S24Packed},
    util::{device_notif::DeviceStateNotifier, prof_notif::ProcFramesNotif},
    AsRawRef, Binding, MaResult,
};

/// Entry point for constructing audio devices.
///
/// `DeviceBuilder` creates typed builders for playback, capture, duplex, and
/// loopback devices.
///
/// Each builder starts in an unconfigured format state. A sample format must be
/// selected first (for example with `.f32()`, `.i16()`, etc.) before most other
/// configuration methods become available. This mirrors miniaudio's device
/// configuration model while making the format requirement explicit in the type
/// system.
///
/// ## Device types
///
/// - [`DeviceBuilder::playback()`] creates a playback device builder.
/// - [`DeviceBuilder::capture()`] creates a capture device builder.
/// - [`DeviceBuilder::duplex()`] creates a full-duplex device builder.
/// - [`DeviceBuilder::loopback()`] creates a loopback device builder.
///
/// In miniaudio, playback callbacks write to the output buffer, capture and
/// loopback callbacks read from the input buffer, and duplex callbacks do both.
///
/// ## Defaults
///
/// These builders are initialized from `ma_device_config_init()`, which means
/// miniaudio's default device configuration is used as the starting point.
/// Leaving channel count, or sample rate at their defaults allows
/// miniaudio to use the device's native configuration.
///
/// ## Contexts and backends
///
/// A device can be initialized either with an explicitly configured context or
/// with an internal context created by miniaudio. Use [`DeviceBuilderOps::context`]
/// when you need device enumeration, explicit backend selection, logging, or
/// other context-level customization.
///
/// ## Callback model
///
/// Devices created through these builders are callback-driven. The callback runs
/// asynchronously on the audio thread and must obey real-time audio constraints:
/// avoid blocking, allocation, I/O, or calling device lifecycle functions from
/// inside the callback.
pub struct DeviceBuilder {}

/// Marker type for a device builder whose sample format has not been selected yet.
///
/// Builders start in this state to force the sample format to be chosen before
/// callback installation or most device configuration is allowed.
///
/// This type is only used at the type level.
pub struct Unknown {}

/// Builder for a playback device.
///
/// A playback device requests audio from the application through a data callback.
/// The callback receives a mutable output buffer and must write at most the
/// requested number of frames.
///
/// The sample type is determined by the selected [`PcmFormat`].
///
/// Construct this builder with [`DeviceBuilder::playback()`].
pub struct PlayBackDeviceBuilder<'a, F = Unknown> {
    inner: sys::ma_device_config,
    context: Option<&'a ContextBuilder<'a>>,
    backends: Option<&'a [Backend]>,
    data_callback_info: Option<DeviceBuilderDataCallBack>,
    state_notifier: bool,
    _format: PhantomData<F>,
}

/// Builder for a capture device.
///
/// A capture device delivers recorded audio to the application through a data
/// callback. The callback receives an immutable input buffer containing the
/// captured frames.
///
/// The sample type is determined by the selected [`PcmFormat`].
///
/// Construct this builder with [`DeviceBuilder::capture()`].
pub struct CaptureDeviceBuilder<'a, F = Unknown> {
    inner: sys::ma_device_config,
    context: Option<&'a ContextBuilder<'a>>,
    backends: Option<&'a [Backend]>,
    data_callback_info: Option<DeviceBuilderDataCallBack>,
    state_notifier: bool,
    _format: PhantomData<F>,
}

/// Builder for a full-duplex device.
///
/// A duplex device exposes both playback and capture streams in the same
/// callback. The callback receives a mutable output buffer and an immutable
/// input buffer for the same callback cycle.
///
/// Playback and capture share the device sample rate.
///
/// Construct this builder with [`DeviceBuilder::duplex()`].
pub struct DuplexDeviceBuilder<'a, F = Unknown> {
    inner: sys::ma_device_config,
    context: Option<&'a ContextBuilder<'a>>,
    backends: Option<&'a [Backend]>,
    data_callback_info: Option<DeviceBuilderDataCallBack>,
    state_notifier: bool,
    _format: PhantomData<F>,
}

/// Builder for a loopback device.
///
/// A loopback device captures audio being played by the system rather than audio
/// from a physical microphone. Support depends on the active backend.
///
/// The callback receives an immutable input buffer containing captured frames.
///
/// Construct this builder with [`DeviceBuilder::loopback()`].
pub struct LoopBackDeviceBuilder<'a, F = Unknown> {
    inner: sys::ma_device_config,
    context: Option<&'a ContextBuilder<'a>>,
    backends: Option<&'a [Backend]>,
    data_callback_info: Option<DeviceBuilderDataCallBack>,
    state_notifier: bool,
    _format: PhantomData<F>,
}

#[derive(Clone)]
pub(crate) struct DeviceBuilderDataCallBack {
    pub(crate) data_callback: *mut core::ffi::c_void, // type erased for each State (ex: LoopBackDeviceState)
    pub(crate) data_callback_drop: fn(*mut core::ffi::c_void),
    pub(crate) data_callback_panic: Arc<AtomicBool>,
    pub(crate) state_notif: DeviceStateNotifier,
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
        fn set_context<'s>(t: &'s mut T, context: &'a ContextBuilder);
        fn get_callback_info(t: &T) -> Option<DeviceBuilderDataCallBack>;
        fn set_state_cb_info(t: &mut T);
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

        fn set_context<'s>(t: &'s mut PlayBackDeviceBuilder<'a, F>, context: &'a ContextBuilder) {
            t.context = Some(context);
        }

        fn get_callback_info(
            t: &PlayBackDeviceBuilder<'a, F>,
        ) -> Option<DeviceBuilderDataCallBack> {
            t.data_callback_info.clone()
        }

        fn set_state_cb_info(t: &mut PlayBackDeviceBuilder<'a, F>) {
            t.state_notifier = true;
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

        fn set_context<'s>(t: &'s mut CaptureDeviceBuilder<'a, F>, context: &'a ContextBuilder) {
            t.context = Some(context);
        }

        fn get_callback_info(t: &CaptureDeviceBuilder<'a, F>) -> Option<DeviceBuilderDataCallBack> {
            t.data_callback_info.clone()
        }

        fn set_state_cb_info(t: &mut CaptureDeviceBuilder<'a, F>) {
            t.state_notifier = true;
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

        fn set_context<'s>(t: &'s mut DuplexDeviceBuilder<'a, F>, context: &'a ContextBuilder) {
            t.context = Some(context);
        }

        fn get_callback_info(t: &DuplexDeviceBuilder<'a, F>) -> Option<DeviceBuilderDataCallBack> {
            t.data_callback_info.clone()
        }

        fn set_state_cb_info(t: &mut DuplexDeviceBuilder<'a, F>) {
            t.state_notifier = true;
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

        fn set_context<'s>(t: &'s mut LoopBackDeviceBuilder<'a, F>, context: &'a ContextBuilder) {
            t.context = Some(context);
        }

        fn get_callback_info(
            t: &LoopBackDeviceBuilder<'a, F>,
        ) -> Option<DeviceBuilderDataCallBack> {
            t.data_callback_info.clone()
        }

        fn set_state_cb_info(t: &mut LoopBackDeviceBuilder<'a, F>) {
            t.state_notifier = true;
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

    pub fn set_context<'a, 's, T: AsDeviceBuilder<'a> + ?Sized>(
        t: &'s mut T,
        context: &'a ContextBuilder,
    ) {
        <T as AsDeviceBuilder>::_DeviceBuilderProvider::set_context(t, context);
    }

    pub fn get_data_callback_info<'a, T: AsDeviceBuilder<'a> + ?Sized>(
        t: &T,
    ) -> Option<DeviceBuilderDataCallBack> {
        <T as AsDeviceBuilder>::_DeviceBuilderProvider::get_callback_info(t)
    }

    pub fn set_state_cb_info<'a, 's, T: AsDeviceBuilder<'a> + ?Sized>(t: &'s mut T) {
        <T as AsDeviceBuilder>::_DeviceBuilderProvider::set_state_cb_info(t);
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
            context: None,
            backends: self.backends,
            data_callback_info: None,
            state_notifier: false,
            _format: PhantomData,
        }
    }

    pub fn u8(&mut self) -> PlayBackDeviceBuilder<'a, u8> {
        self.inner.playback.format = sys::ma_format_ma_format_u8;
        self.new_inner::<u8>()
    }

    pub fn i16(&mut self) -> PlayBackDeviceBuilder<'a, i16> {
        self.inner.playback.format = sys::ma_format_ma_format_s16;
        self.new_inner::<i16>()
    }

    pub fn i32(&mut self) -> PlayBackDeviceBuilder<'a, i32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<i32>()
    }

    pub fn s24_packed(&mut self) -> PlayBackDeviceBuilder<'a, S24Packed> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<S24Packed>()
    }

    pub fn f32(&mut self) -> PlayBackDeviceBuilder<'a, f32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.new_inner::<f32>()
    }
}

impl<'a> CaptureDeviceBuilder<'a, Unknown> {
    fn new_inner<F: PcmFormat>(&self) -> CaptureDeviceBuilder<'a, F> {
        CaptureDeviceBuilder {
            inner: self.inner,
            context: None,
            backends: self.backends,
            data_callback_info: None,
            state_notifier: false,
            _format: PhantomData,
        }
    }

    pub fn u8(&mut self) -> CaptureDeviceBuilder<'a, u8> {
        self.inner.capture.format = sys::ma_format_ma_format_u8;
        self.new_inner::<u8>()
    }

    pub fn i16(&mut self) -> CaptureDeviceBuilder<'a, i16> {
        self.inner.capture.format = sys::ma_format_ma_format_s16;
        self.new_inner::<i16>()
    }

    pub fn i32(&mut self) -> CaptureDeviceBuilder<'a, i32> {
        self.inner.capture.format = sys::ma_format_ma_format_s32;
        self.new_inner::<i32>()
    }

    pub fn s24_packed(&mut self) -> CaptureDeviceBuilder<'a, S24Packed> {
        self.inner.capture.format = sys::ma_format_ma_format_s32;
        self.new_inner::<S24Packed>()
    }

    pub fn f32(&mut self) -> CaptureDeviceBuilder<'a, f32> {
        self.inner.capture.format = sys::ma_format_ma_format_s32;
        self.new_inner::<f32>()
    }
}

impl<'a> DuplexDeviceBuilder<'a, Unknown> {
    fn new_inner<F: PcmFormat>(&self) -> DuplexDeviceBuilder<'a, F> {
        DuplexDeviceBuilder {
            inner: self.inner,
            context: None,
            backends: self.backends,
            data_callback_info: None,
            state_notifier: false,
            _format: PhantomData,
        }
    }

    pub fn u8(&mut self) -> DuplexDeviceBuilder<'a, u8> {
        self.inner.playback.format = sys::ma_format_ma_format_u8;
        self.inner.capture.format = sys::ma_format_ma_format_u8;
        self.new_inner::<u8>()
    }

    pub fn i16(&mut self) -> DuplexDeviceBuilder<'a, i16> {
        self.inner.playback.format = sys::ma_format_ma_format_s16;
        self.inner.capture.format = sys::ma_format_ma_format_s16;
        self.new_inner::<i16>()
    }

    pub fn i32(&mut self) -> DuplexDeviceBuilder<'a, i32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.inner.capture.format = sys::ma_format_ma_format_s32;
        self.new_inner::<i32>()
    }

    pub fn s24_packed(&mut self) -> DuplexDeviceBuilder<'a, S24Packed> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.inner.capture.format = sys::ma_format_ma_format_s32;
        self.new_inner::<S24Packed>()
    }

    pub fn f32(&mut self) -> DuplexDeviceBuilder<'a, f32> {
        self.inner.playback.format = sys::ma_format_ma_format_s32;
        self.inner.capture.format = sys::ma_format_ma_format_s32;
        self.new_inner::<f32>()
    }
}

impl<'a> LoopBackDeviceBuilder<'a, Unknown> {
    fn new_inner<F: PcmFormat>(&self) -> LoopBackDeviceBuilder<'a, F> {
        LoopBackDeviceBuilder {
            inner: self.inner,
            context: None,
            backends: self.backends,
            data_callback_info: None,
            state_notifier: false,
            _format: PhantomData,
        }
    }

    pub fn u8(&mut self) -> LoopBackDeviceBuilder<'a, u8> {
        self.inner.capture.format = sys::ma_format_ma_format_u8;
        self.new_inner::<u8>()
    }

    pub fn i16(&mut self) -> LoopBackDeviceBuilder<'a, i16> {
        self.inner.capture.format = sys::ma_format_ma_format_s16;
        self.new_inner::<i16>()
    }

    pub fn i32(&mut self) -> LoopBackDeviceBuilder<'a, i32> {
        self.inner.capture.format = sys::ma_format_ma_format_s32;
        self.new_inner::<i32>()
    }

    pub fn s24_packed(&mut self) -> LoopBackDeviceBuilder<'a, S24Packed> {
        self.inner.capture.format = sys::ma_format_ma_format_s32;
        self.new_inner::<S24Packed>()
    }

    pub fn f32(&mut self) -> LoopBackDeviceBuilder<'a, f32> {
        self.inner.capture.format = sys::ma_format_ma_format_s32;
        self.new_inner::<f32>()
    }
}

impl<'a, F: PcmFormat> DeviceBuilderOps<'a> for PlayBackDeviceBuilder<'a, F> {}
impl<'a, F: PcmFormat> DeviceBuilderOps<'a> for CaptureDeviceBuilder<'a, F> {}
impl<'a, F: PcmFormat> DeviceBuilderOps<'a> for DuplexDeviceBuilder<'a, F> {}
impl<'a, F: PcmFormat> DeviceBuilderOps<'a> for LoopBackDeviceBuilder<'a, F> {}

/// Shared configuration methods for all device builders.
///
/// These methods modify the underlying miniaudio device configuration before the
/// device is initialized.
///
/// Some setters are only available for builders whose device mode supports the
/// corresponding stream:
///
/// - playback setters require playback support
/// - capture setters require capture support
///
/// ## Borrowed configuration
///
/// Some configuration methods store borrowed data inside the builder until the
/// device is created, such as backend lists, channel maps, device IDs, or an
/// explicit context. Those borrowed values must remain valid until device
/// initialization completes.
///
/// ## Defaults
///
/// Unless explicitly overridden, options use the defaults established by
/// `ma_device_config_init()`.
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

    /// The preferred share mode to use for playback. Can be either [`DeviceShareMode::Shared`] (default) or [`DeviceShareMode::Exclusive`] .
    /// Note that if you specify exclusive mode, but it's not supported by the backend,
    /// initialization will fail. You can then fall back to shared mode if desired by changing
    /// this to [`DeviceShareMode`] and reinitializing.
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

    /// The preferred share mode to use for capture. Can be either [`DeviceShareMode::Shared`] (default) or [`DeviceShareMode::Exclusive`] .
    /// Note that if you specify exclusive mode, but it's not supported by the backend,
    /// initialization will fail. You can then fall back to shared mode if desired by changing
    /// this to [`DeviceShareMode`] and reinitializing.
    fn capture_share_mode(&mut self, mode: DeviceShareMode) -> &mut Self
    where
        Self: private_device_b::SupportsCapture,
    {
        private_device_b::inner(self).capture.shareMode = mode.into();
        self
    }

    /// See [`PerformanceProfile`]
    fn performance_profile(&mut self, profile: PerformanceProfile) -> &mut Self {
        private_device_b::inner(self).performanceProfile = profile.into();
        self
    }

    /// Enables unclipped floating-point playback output.
    ///
    /// When enabled, miniaudio will not clip the playback output buffer after the
    /// callback returns. This only applies when the playback sample format is `f32`.
    ///
    /// Disabled by default.
    fn clipping(&mut self, yes: bool) -> &mut Self {
        private_device_b::inner(self).noClip = yes as u8;
        self
    }

    /// Controls whether the playback output buffer is pre-silenced before the callback.
    ///
    /// When enabled, the output buffer passed to the callback is initialized to
    /// silence before your callback runs.
    ///
    /// When disabled, the output buffer contents are left undefined and your
    /// callback is expected to fully write the requested output.
    ///
    /// Enabled by default.
    fn pre_silenced_output(&mut self, yes: bool) -> &mut Self {
        private_device_b::inner(self).noPreSilencedOutputBuffer = (!yes) as u8;
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

    /// Sets the desired period size in frames.
    ///
    /// A *period* is the number of frames processed in one device callback.
    /// This effectively controls the callback buffer size and therefore the
    /// latency of the device.
    ///
    /// Smaller values result in:
    /// - lower latency
    /// - more frequent callbacks
    ///
    /// Larger values result in:
    /// - higher latency
    /// - fewer callbacks
    ///
    /// If this value is non-zero, it takes priority over
    /// [`period_size_millis`](Self::period_size_millis).
    ///
    /// If both period size settings are left at `0`, miniaudio will choose a
    /// default buffer size based on the selected [`performance_profile`](Self::performance_profile).
    ///
    /// This value is treated as a hint and may be adjusted by the backend.
    fn period_size_frames(&mut self, frames: u32) -> &mut Self {
        private_device_b::inner(self).periodSizeInFrames = frames;
        self
    }

    /// Sets the desired period size in milliseconds.
    ///
    /// This is an alternative way of specifying the device period size.
    /// The value will be converted to frames using the device sample rate.
    ///
    /// If [`period_size_frames`](Self::period_size_frames) is also set,
    /// the frame value takes priority.
    ///
    /// If both period size settings are left at `0`, miniaudio will choose
    /// a default buffer size based on the selected
    /// [`performance_profile`](Self::performance_profile).
    ///
    /// Smaller values reduce latency but increase callback frequency.
    ///
    /// This value is treated as a hint and may be adjusted by the backend.
    fn period_size_millis(&mut self, millis: u32) -> &mut Self {
        private_device_b::inner(self).periodSizeInMilliseconds = millis;
        self
    }

    /// Controls whether miniaudio may vary the callback frame count.
    ///
    /// By default, miniaudio attempts to call the data callback with a consistent
    /// frame count based on the configured period size.
    ///
    /// When enabled, miniaudio may invoke the callback with whatever frame count
    /// the backend requests.
    ///
    /// Disabled by default.
    fn fixed_callback_size(&mut self, yes: bool) -> &mut Self {
        private_device_b::inner(self).noFixedSizedCallback = yes as u8;
        self
    }

    /// Enables device state notifications. See [`DeviceStateNotifier`].
    ///
    /// When enabled, the device installs a miniaudio notification callback and
    /// records state changes such as start, stop, reroute, or interruptions when
    /// the active backend reports them.
    ///
    /// Use [`Device::get_state_notifier()`] to access it after the device is created.
    fn state_notifier(&mut self) -> &mut Self {
        private_device_b::set_state_cb_info(self);
        self
    }

    // TODO: Does the context get coppied by miniaudio?
    fn context(&mut self, ctx: &'a ContextBuilder) -> &mut Self {
        private_device_b::set_context(self, ctx);
        self
    }
}

impl<'a> DeviceBuilder {
    pub fn playback() -> PlayBackDeviceBuilder<'a, Unknown> {
        let ptr = unsafe { sys::ma_device_config_init(DeviceType::PlayBack.into()) };
        PlayBackDeviceBuilder {
            inner: ptr,
            context: None,
            backends: None,
            data_callback_info: None,
            state_notifier: false,
            _format: PhantomData,
        }
    }

    pub fn capture() -> CaptureDeviceBuilder<'a, Unknown> {
        let ptr = unsafe { sys::ma_device_config_init(DeviceType::Capture.into()) };
        CaptureDeviceBuilder {
            inner: ptr,
            context: None,
            backends: None,
            data_callback_info: None,
            state_notifier: false,
            _format: PhantomData,
        }
    }

    pub fn duplex() -> DuplexDeviceBuilder<'a, Unknown> {
        let ptr = unsafe { sys::ma_device_config_init(DeviceType::Duplex.into()) };
        DuplexDeviceBuilder {
            inner: ptr,
            context: None,
            backends: None,
            data_callback_info: None,
            state_notifier: false,
            _format: PhantomData,
        }
    }

    pub fn loopback() -> LoopBackDeviceBuilder<'a, Unknown> {
        let ptr = unsafe { sys::ma_device_config_init(DeviceType::LoopBack.into()) };
        LoopBackDeviceBuilder {
            inner: ptr,
            context: None,
            backends: None,
            data_callback_info: None,
            state_notifier: false,
            _format: PhantomData,
        }
    }
}

impl<'a, F: PcmFormat> PlayBackDeviceBuilder<'a, F> {
    /// Builds the device and installs a playback callback.
    ///
    /// The callback is invoked on miniaudio's audio thread whenever the device
    /// needs more playback data.
    ///
    /// Unlike raw miniaudio, this callback does not expose both input and output
    /// buffers. Playback callbacks only receive the playback buffer, which is
    /// provided as a mutable interleaved slice of `F::StorageUnit`.
    ///
    /// ## Callback parameters
    ///
    /// The callback receives the following parameters:
    ///
    /// 1. **`CallBackDevice`**
    ///    A restricted device handle that exposes only operations that are safe to
    ///    call from inside the audio callback.
    ///
    /// 2. **`output`**
    ///    A mutable interleaved buffer that must be filled with playback samples.
    ///
    /// 3. **`frame_count`**
    ///    The number of frames available in the buffer.  
    ///    One frame contains one sample for each channel.
    ///
    /// The [`CallBackDevice`] a restricted device handle exposing only
    /// operations that are safe to use from inside the audio callback.
    ///
    /// The slice length is `frame_count * playback_channels`.
    ///
    /// ## Callback contract
    ///
    /// - Write playback samples into `output`.
    /// - Do not read from uninitialized parts of `output`.
    /// - Do not write more than `frame_count` frames.
    ///
    /// ## Real-time behavior
    ///
    /// This callback runs asynchronously on the device's audio thread. It should
    /// avoid blocking, sleeping, allocation, file I/O, or other work that may
    /// cause underruns.
    ///
    /// ## Panics
    ///
    /// Panics are trapped to avoid unwinding across the FFI boundary. After a
    /// panic, the callback is poisoned and will no longer run user code.
    pub fn with_callback<C>(&mut self, f: C) -> MaResult<Device>
    where
        C: FnMut(CallBackDevice, &mut [F::StorageUnit], u32) + Send + 'static,
    {
        let panic_flag = Arc::new(AtomicBool::new(false));
        let state_notif = DeviceStateNotifier::default();
        let state: PlayBackDeviceState<F, C> = PlayBackDeviceState {
            f: UnsafeCell::new(f),
            frames_processed: ProcFramesNotif::default(),
            panic_flag: panic_flag.clone(),
            // If state notif was not set in `set_state_cb_info`, it will never get fired
            state_notif: state_notif.clone(),
            _format: PhantomData,
        };

        let callback_process_notifier = state.frames_processed.clone();

        let state_box = Box::new(state);
        let state_ptr: *mut PlayBackDeviceState<F, C> = Box::into_raw(state_box);
        let callback_info: DeviceBuilderDataCallBack = DeviceBuilderDataCallBack {
            data_callback: state_ptr.cast(),
            data_callback_drop: drop_playback_device_state::<F, C>,
            data_callback_panic: panic_flag,
            state_notif: state_notif.clone(),
        };

        self.data_callback_info = Some(callback_info);

        // Set all the callbacks and user data in the config
        self.inner.dataCallback = Some(device_data_playback_callback::<F, C>);
        if self.state_notifier {
            self.inner.notificationCallback = Some(device_notification_playback_callback::<F, C>);
        }
        self.inner.pUserData = state_ptr as *mut core::ffi::c_void;

        Device::new_with_config(
            self,
            self.context,
            private_device_b::get_backends(self),
            callback_process_notifier,
        )
    }
}

impl<'a, F: PcmFormat> CaptureDeviceBuilder<'a, F> {
    /// Builds the device and installs a capture callback.
    ///
    /// The callback is invoked on miniaudio's audio thread whenever captured
    /// audio is available.
    ///
    /// Unlike raw miniaudio, this callback only exposes the input side of the
    /// stream. Capture callbacks receive an immutable interleaved slice of
    /// `F::StorageUnit` containing the captured frames.
    ///
    /// ## Callback parameters
    ///
    /// The callback receives the following parameters:
    ///
    /// 1. **`CallBackDevice`**
    ///    A restricted device handle that exposes only operations that are safe to
    ///    call from inside the audio callback.
    ///
    /// 2. **`input`**
    ///    Immutable interleaved capture buffer.
    ///
    /// 3. **`frame_count`**
    ///    The number of frames available in the buffer.  
    ///    One frame contains one sample for each channel.
    ///
    /// The [`CallBackDevice`] a restricted device handle exposing only
    /// operations that are safe to use from inside the audio callback.
    ///
    /// The slice length is `frame_count * capture_channels`.
    ///
    /// ## Callback contract
    ///
    /// - Read captured samples from `input`.
    /// - Treat the slice as read-only captured audio for this callback cycle.
    /// - Do not assume the data remains valid after the callback returns.
    ///
    /// ## Real-time behavior
    ///
    /// This callback runs asynchronously on the device's audio thread. It should
    /// avoid blocking, sleeping, allocation, file I/O, or other work that may
    /// cause drop-outs.
    ///
    /// ## Panics
    ///
    /// Panics are trapped to avoid unwinding across the FFI boundary. After a
    /// panic, the callback is poisoned and will no longer run user code.
    pub fn with_callback<C>(&mut self, f: C) -> MaResult<Device>
    where
        C: FnMut(CallBackDevice, &[F::StorageUnit], u32) + Send + 'static,
    {
        let panic_flag = Arc::new(AtomicBool::new(false));
        let state_notif = DeviceStateNotifier::default();

        let state: CaptureDeviceState<F, C> = CaptureDeviceState {
            f: UnsafeCell::new(f),
            frames_processed: ProcFramesNotif::default(),
            panic_flag: panic_flag.clone(),
            // If state notif was not set in `set_state_cb_info`, it will never get fired
            state_notif: state_notif.clone(),
            _format: PhantomData,
        };

        let callback_process_notifier = state.frames_processed.clone();

        let state_box = Box::new(state);
        let state_ptr: *mut CaptureDeviceState<F, C> = Box::into_raw(state_box);
        let callback_info: DeviceBuilderDataCallBack = DeviceBuilderDataCallBack {
            data_callback: state_ptr.cast(),
            data_callback_drop: drop_capture_device_state::<F, C>,
            data_callback_panic: panic_flag,
            state_notif: state_notif.clone(),
        };

        self.data_callback_info = Some(callback_info);

        // Set all the callbacks and user data in the config
        self.inner.dataCallback = Some(device_data_capture_callback::<F, C>);
        if self.state_notifier {
            self.inner.notificationCallback = Some(device_notification_capture_callback::<F, C>);
        }
        self.inner.pUserData = state_ptr as *mut core::ffi::c_void;

        Device::new_with_config(
            self,
            self.context,
            private_device_b::get_backends(self),
            callback_process_notifier,
        )
    }
}

impl<'a, F: PcmFormat> DuplexDeviceBuilder<'a, F> {
    /// Builds the device and installs a duplex callback.
    ///
    /// The callback is invoked on miniaudio's audio thread for full-duplex
    /// processing. It receives both the playback output buffer and the captured
    /// input buffer for the current callback cycle.
    ///
    /// Unlike raw miniaudio, the callback signature is specialized for duplex
    /// mode and exposes only correctly typed slices.
    ///
    /// ## Callback parameters
    ///
    /// The callback receives:
    ///
    /// 1. **`CallBackDevice`**
    ///    A callback-safe device handle.
    ///
    /// 2. **`output`**
    ///    Mutable interleaved playback buffer.
    ///
    /// 3. **`input`**
    ///    Immutable interleaved capture buffer.
    ///
    /// 4. **`frame_count`**
    ///    The number of frames available for this callback cycle.
    ///
    /// The [`CallBackDevice`] a restricted device handle exposing only
    /// operations that are safe to use from inside the audio callback.
    ///
    /// The output slice length is `frame_count * playback_channels`.
    /// The input slice length is `frame_count * capture_channels`.
    ///
    /// ## Callback contract
    ///
    /// - Read captured samples from `input`.
    /// - Write playback samples into `output`.
    /// - Do not read from uninitialized parts of `output`.
    /// - Do not process more than `frame_count` frames.
    ///
    /// ## Real-time behavior
    ///
    /// This callback runs asynchronously on the device's audio thread. It should
    /// avoid blocking, sleeping, allocation, file I/O, or other work that may
    /// cause underruns or overruns.
    ///
    /// ## Panics
    ///
    /// Panics are trapped to avoid unwinding across the FFI boundary. After a
    /// panic, the callback is poisoned and will no longer run user code.
    pub fn with_callback<C>(&mut self, f: C) -> MaResult<Device>
    where
        C: FnMut(CallBackDevice, &mut [F::StorageUnit], &[F::StorageUnit], u32) + Send + 'static,
    {
        let panic_flag = Arc::new(AtomicBool::new(false));
        let state_notif = DeviceStateNotifier::default();
        let state: DuplexDeviceState<F, C> = DuplexDeviceState {
            f: UnsafeCell::new(f),
            frames_processed: ProcFramesNotif::default(),
            panic_flag: panic_flag.clone(),
            // If state notif was not set in `set_state_cb_info`, it will never get fired
            state_notif: state_notif.clone(),
            _format: PhantomData,
        };

        let callback_process_notifier = state.frames_processed.clone();

        let state_box = Box::new(state);
        let state_ptr: *mut DuplexDeviceState<F, C> = Box::into_raw(state_box);
        let callback_info: DeviceBuilderDataCallBack = DeviceBuilderDataCallBack {
            data_callback: state_ptr.cast(),
            data_callback_drop: drop_duplex_device_state::<F, C>,
            data_callback_panic: panic_flag,
            state_notif: state_notif.clone(),
        };

        self.data_callback_info = Some(callback_info);

        // Set all the callbacks and user data in the config
        self.inner.dataCallback = Some(device_data_duplex_callback::<F, C>);
        if self.state_notifier {
            self.inner.notificationCallback = Some(device_notification_duplex_callback::<F, C>);
        }
        self.inner.pUserData = state_ptr as *mut core::ffi::c_void;

        Device::new_with_config(
            self,
            self.context,
            private_device_b::get_backends(self),
            callback_process_notifier,
        )
    }
}

impl<'a, F: PcmFormat> LoopBackDeviceBuilder<'a, F> {
    /// Builds the device and installs a loopback callback.
    ///
    /// The callback is invoked on miniaudio's audio thread whenever loopback
    /// data is available.
    ///
    /// Loopback mode captures audio being played by the system. Like capture
    /// mode, the callback only receives the input side of the stream as an
    /// immutable interleaved slice of `F::StorageUnit`.
    ///
    /// ## Callback parameters
    ///
    /// The callback receives the following parameters:
    ///
    /// 1. **`CallBackDevice`**
    ///    A restricted device handle that exposes only operations that are safe to
    ///    call from inside the audio callback.
    ///
    /// 2. **`input`**
    ///    Immutable interleaved capture buffer.
    ///
    /// 3. **`frame_count`**
    ///    The number of frames available in the buffer.  
    ///    One frame contains one sample for each channel.
    ///
    /// The [`CallBackDevice`] a restricted device handle exposing only
    /// operations that are safe to use from inside the audio callback.
    ///
    /// The slice length is `frame_count * capture_channels`.
    ///
    /// ## Callback contract
    ///
    /// - Read captured loopback samples from `input`.
    /// - Treat the slice as read-only audio for this callback cycle.
    /// - Do not assume the data remains valid after the callback returns.
    ///
    /// ## Real-time behavior
    ///
    /// This callback runs asynchronously on the device's audio thread. It should
    /// avoid blocking, sleeping, allocation, file I/O, or other work that may
    /// cause drop-outs.
    ///
    /// ## Panics
    ///
    /// Panics are trapped to avoid unwinding across the FFI boundary. After a
    /// panic, the callback is poisoned and will no longer run user code.
    pub fn with_callback<C>(&mut self, f: C) -> MaResult<Device>
    where
        C: FnMut(CallBackDevice, &[F::StorageUnit], u32) + Send + 'static,
    {
        let panic_flag = Arc::new(AtomicBool::new(false));
        let state_notif = DeviceStateNotifier::default();
        let state: LoopBackDeviceState<F, C> = LoopBackDeviceState {
            f: UnsafeCell::new(f),
            frames_processed: ProcFramesNotif::default(),
            panic_flag: panic_flag.clone(),
            // If state notif was not set in `set_state_cb_info`, it will never get fired
            state_notif: state_notif.clone(),
            _format: PhantomData,
        };

        let callback_process_notifier = state.frames_processed.clone();

        let state_box = Box::new(state);
        let state_ptr: *mut LoopBackDeviceState<F, C> = Box::into_raw(state_box);
        let callback_info: DeviceBuilderDataCallBack = DeviceBuilderDataCallBack {
            data_callback: state_ptr.cast(),
            data_callback_drop: drop_loopback_device_state::<F, C>,
            data_callback_panic: panic_flag,
            state_notif: state_notif.clone(),
        };

        self.data_callback_info = Some(callback_info);

        // Set all the callbacks and user data in the config
        self.inner.dataCallback = Some(device_data_loopback_callback::<F, C>);
        if self.state_notifier {
            self.inner.notificationCallback = Some(device_notification_loopback_callback::<F, C>);
        }
        self.inner.pUserData = state_ptr as *mut core::ffi::c_void;

        Device::new_with_config(
            self,
            self.context,
            private_device_b::get_backends(self),
            callback_process_notifier,
        )
    }
}

/// Device that lives inside the data callback
///
/// Provides limited access only to functions safe to call from inside the audio callback
pub struct CallBackDevice {
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

pub(crate) struct PlayBackDeviceState<F: PcmFormat, C> {
    f: UnsafeCell<C>,
    frames_processed: ProcFramesNotif,
    panic_flag: Arc<AtomicBool>,
    pub(crate) state_notif: DeviceStateNotifier,
    _format: PhantomData<F>,
}

pub(crate) struct CaptureDeviceState<F: PcmFormat, C> {
    f: UnsafeCell<C>,
    frames_processed: ProcFramesNotif,
    panic_flag: Arc<AtomicBool>,
    pub(crate) state_notif: DeviceStateNotifier,
    _format: PhantomData<F>,
}

pub(crate) struct DuplexDeviceState<F: PcmFormat, C> {
    f: UnsafeCell<C>,
    frames_processed: ProcFramesNotif,
    panic_flag: Arc<AtomicBool>,
    pub(crate) state_notif: DeviceStateNotifier,
    _format: PhantomData<F>,
}

pub(crate) struct LoopBackDeviceState<F: PcmFormat, C> {
    f: UnsafeCell<C>,
    frames_processed: ProcFramesNotif,
    panic_flag: Arc<AtomicBool>,
    pub(crate) state_notif: DeviceStateNotifier,
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

    // Build state from user data
    let cb_device = CallBackDevice::from_ptr(device);
    let state = &*((*device).pUserData as *const PlayBackDeviceState<F, C>);

    // Register processed frames in the flag
    state.frames_processed.add_frames(frame_count as u64);

    // Build the slice
    let channels = (*device).playback.channels;
    let Some(slice_len) = (frame_count as usize).checked_mul(channels as usize) else {
        return;
    };
    let Some(slice_len) = slice_len.checked_mul(<F as PcmFormat>::VEC_STORE_UNITS_PER_FRAME) else {
        return;
    };
    let slice = unsafe { slice::from_raw_parts_mut(output.cast::<F::StorageUnit>(), slice_len) };

    if state.panic_flag.load(Ordering::Relaxed) {
        // The callback is now poisoned
        slice.fill(F::SILENCE);
        return;
    }

    // Run the callback
    let cb = &mut *state.f.get();
    let res = catch_unwind(AssertUnwindSafe(|| (cb)(cb_device, slice, frame_count)));
    if res.is_err() {
        // The callback is now poisoned
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

    // Build state from user data
    let cb_device = CallBackDevice::from_ptr(device);
    let state = &*((*device).pUserData as *const CaptureDeviceState<F, C>);

    // Register processed frames in the flag
    state.frames_processed.add_frames(frame_count as u64);

    if state.panic_flag.load(Ordering::Relaxed) {
        // The callback is poisoned
        return;
    }

    // Build the slice
    let channels = (*device).capture.channels;
    let Some(slice_len) = (frame_count as usize).checked_mul(channels as usize) else {
        return;
    };
    let Some(slice_len) = slice_len.checked_mul(<F as PcmFormat>::VEC_STORE_UNITS_PER_FRAME) else {
        return;
    };
    let slice = unsafe { slice::from_raw_parts(input.cast::<F::StorageUnit>(), slice_len) };

    // Run the callback
    let cb = &mut *state.f.get();
    let res = catch_unwind(AssertUnwindSafe(|| (cb)(cb_device, slice, frame_count)));
    if res.is_err() {
        // The callback is now poisoned
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

    // Build state from user data
    let cb_device = CallBackDevice::from_ptr(device);
    let state = &*((*device).pUserData as *const DuplexDeviceState<F, C>);

    // Register processed frames in the flag
    state.frames_processed.add_frames(frame_count as u64);

    // Build the slice
    let channels = (*device).playback.channels;
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
        // The callback is poisoned
        out_slice.fill(F::SILENCE);
        return;
    }

    // Run the callback
    let cb = &mut *state.f.get();
    let res = catch_unwind(AssertUnwindSafe(|| {
        (cb)(cb_device, out_slice, in_slice, frame_count)
    }));
    if res.is_err() {
        // The callback is now poisoned
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

    // Build state from user data
    let cb_device = CallBackDevice::from_ptr(device);
    let state = &*((*device).pUserData as *const LoopBackDeviceState<F, C>);

    // Register processed frames in the flag
    state.frames_processed.add_frames(frame_count as u64);

    if state.panic_flag.load(Ordering::Relaxed) {
        // The callback is poisoned
        return;
    }

    // Build the slice
    let channels = (*device).capture.channels;
    let Some(slice_len) = (frame_count as usize).checked_mul(channels as usize) else {
        return;
    };
    let Some(slice_len) = slice_len.checked_mul(<F as PcmFormat>::VEC_STORE_UNITS_PER_FRAME) else {
        return;
    };
    let slice = unsafe { slice::from_raw_parts(input.cast::<F::StorageUnit>(), slice_len) };

    // Run the callback
    let cb = &mut *state.f.get();
    let res = catch_unwind(AssertUnwindSafe(|| (cb)(cb_device, slice, frame_count)));
    if res.is_err() {
        // The callback is now poisoned
        state.panic_flag.store(true, Ordering::Release);
    }
}

// Functions to de-allocate the user data for data callback
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
    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn test_device_builder_basic_playback_init() {
        use crate::device::device_builder::{DeviceBuilder, DeviceBuilderOps};
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
        use crate::device::device_builder::{DeviceBuilder, DeviceBuilderOps};
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
        use crate::device::device_builder::{DeviceBuilder, DeviceBuilderOps};
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
        use crate::device::device_builder::{DeviceBuilder, DeviceBuilderOps};
        let mut device = DeviceBuilder::loopback()
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
    fn test_device_builder_playback_state_notifier() {
        use crate::{
            device::device_builder::{DeviceBuilder, DeviceBuilderOps},
            util::device_notif::DeviceNotificationType,
        };
        let mut device = DeviceBuilder::playback()
            .f32()
            .playback_channels(2)
            .state_notifier()
            .with_callback(|_a, _b, _c| {})
            .unwrap();
        let notif = device.get_state_notifier().unwrap();
        assert!(!notif.contains(DeviceNotificationType::Started));
        device.device_start().unwrap();
        std::thread::sleep(std::time::Duration::from_micros(10));
        assert!(notif.contains(DeviceNotificationType::Started));
        device.device_stop().unwrap();
    }

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn test_device_builder_capture_state_notifier() {
        use crate::{
            device::device_builder::{DeviceBuilder, DeviceBuilderOps},
            util::device_notif::DeviceNotificationType,
        };
        let mut device = DeviceBuilder::capture()
            .f32()
            .capture_channels(2)
            .state_notifier()
            .with_callback(|_a, _b, _c| {})
            .unwrap();
        let notif = device.get_state_notifier().unwrap();
        assert!(!notif.contains(DeviceNotificationType::Started));
        device.device_start().unwrap();
        std::thread::sleep(std::time::Duration::from_micros(10));
        assert!(notif.contains(DeviceNotificationType::Started));
        device.device_stop().unwrap();
    }

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn test_device_builder_duplex_state_notifier() {
        use crate::{
            device::device_builder::{DeviceBuilder, DeviceBuilderOps},
            util::device_notif::DeviceNotificationType,
        };
        let mut device = DeviceBuilder::duplex()
            .f32()
            .playback_channels(2)
            .capture_channels(2)
            .state_notifier()
            .with_callback(|_a, _b, _c, _d| {})
            .unwrap();
        let notif = device.get_state_notifier().unwrap();
        assert!(!notif.contains(DeviceNotificationType::Started));
        device.device_start().unwrap();
        std::thread::sleep(std::time::Duration::from_micros(10));
        assert!(notif.contains(DeviceNotificationType::Started));
        device.device_stop().unwrap();
    }

    #[cfg(not(feature = "ci-tests"))]
    #[cfg(windows)]
    #[test]
    fn test_device_builder_lookback_state_notifier() {
        use crate::{
            device::device_builder::{DeviceBuilder, DeviceBuilderOps},
            util::device_notif::DeviceNotificationType,
        };
        let mut device = DeviceBuilder::loopback()
            .f32()
            .capture_channels(2)
            .state_notifier()
            .with_callback(|_a, _b, _c| {})
            .unwrap();
        let notif = device.get_state_notifier().unwrap();
        assert!(!notif.contains(DeviceNotificationType::Started));
        device.device_start().unwrap();
        std::thread::sleep(std::time::Duration::from_micros(10));
        assert!(notif.contains(DeviceNotificationType::Started));
        device.device_stop().unwrap();
    }
}
