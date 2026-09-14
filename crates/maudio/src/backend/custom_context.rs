use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::MaybeUninit,
    sync::{Arc, OnceLock},
};

use maudio_sys::ffi as sys;

use crate::{
    audio::{
        channels::Channel, formats::Format, performance::PerformanceProfile,
        sample_rate::SampleRate,
    },
    backend::custom_backend::CustomBackend,
    context::{context_ffi, private_context, AsContextPtr, ContextBuilder},
    device::{
        device_id::DeviceId,
        device_type::{DeviceShareMode, DeviceType},
    },
    logging::LogInner,
    Binding, MaResult, MaudioError,
};

pub struct CustomContext<B: CustomBackend>(pub(crate) Arc<CustomContextInner<B>>);

#[repr(C)]
pub(crate) struct CustomContextInner<B: CustomBackend> {
    pub(crate) inner: UnsafeCell<sys::ma_context>,
    pub(crate) user_context: OnceLock<B::Context>,
    pub(super) log: Option<Arc<LogInner>>,
    pub(crate) backend: PhantomData<B>,
}

impl<B: CustomBackend> Binding for CustomContext<B> {
    type Raw = *mut sys::ma_context;

    fn to_raw(&self) -> Self::Raw {
        self.0.inner.get()
    }
}

impl<B: CustomBackend> AsContextPtr for CustomContext<B> {
    type __PtrProvider = private_context::CustomContextProvider;
}

// Private methods
impl<B: CustomBackend> CustomContext<B> {
    pub(crate) fn new_with_config(config: &mut ContextBuilder) -> MaResult<Self> {
        let inner = Arc::new(CustomContextInner {
            inner: unsafe { MaybeUninit::zeroed().assume_init() },
            user_context: OnceLock::new(),
            log: config.log.take(),
            backend: PhantomData,
        });

        let base_ptr = core::ptr::addr_of!(inner.inner);

        context_ffi::ma_context_init(
            config.backends.as_deref(),
            config,
            base_ptr as *mut sys::ma_context,
        )?;

        Ok(CustomContext(inner))
    }
}

impl<B: CustomBackend> Drop for CustomContextInner<B> {
    fn drop(&mut self) {
        let _ = context_ffi::ma_context_uninit(self.inner.get());
    }
}

pub struct UserDeviceConfig {
    pub device_type: DeviceType,
    pub sample_rate: SampleRate,
    pub playback_channels: u32,
    pub capture_channels: u32,
    pub period_size_frames: u32,
    pub period_size_millis: u32,
    pub period_count: u32,
    pub performance_profile: PerformanceProfile,
    pub no_pre_silenced_output_buffer: bool,
    pub no_clip: bool,
    pub no_fixed_size_callback: bool,
}

impl TryFrom<sys::ma_device_config> for UserDeviceConfig {
    type Error = MaudioError;

    fn try_from(value: sys::ma_device_config) -> Result<Self, Self::Error> {
        Ok(Self {
            device_type: value.deviceType.try_into()?,
            sample_rate: value.sampleRate.try_into()?,
            playback_channels: value.playback.channels,
            capture_channels: value.capture.channels,
            period_size_frames: value.periodSizeInFrames,
            period_size_millis: value.periodSizeInMilliseconds,
            period_count: value.periods,
            performance_profile: value.performanceProfile.try_into()?,
            no_pre_silenced_output_buffer: value.noPreSilencedOutputBuffer == 1,
            no_clip: value.noClip == 1,
            no_fixed_size_callback: value.noFixedSizedCallback == 1,
        })
    }
}

pub struct DeviceDescriptor {
    pub device_id: DeviceId,
    pub share_mode: DeviceShareMode,
    pub format: Format,
    pub channels: u32,
    pub sample_rate: SampleRate,
    pub channel_map: Vec<Channel>,
    pub period_size_frames: u32,
    pub period_size_millis: u32,
    pub period_count: u32,
}

impl DeviceDescriptor {
    // SAFETY: sys::ma_device_descriptor cannot outlive self
    unsafe fn to_raw(&self) -> sys::ma_device_descriptor {
        let mut channel_map = [0 as sys::ma_channel; 254];
        for (id, &c) in self
            .channel_map
            .iter()
            .enumerate()
            .take(self.channels as usize)
        {
            channel_map[id] = c.into();
        }

        sys::ma_device_descriptor {
            pDeviceID: &self.device_id.inner.id as *const _,
            shareMode: self.share_mode.into(),
            format: self.format.into(),
            channels: self.channels,
            sampleRate: self.sample_rate.into(),
            channelMap: channel_map,
            periodSizeInFrames: self.period_size_frames,
            periodSizeInMilliseconds: self.period_size_millis,
            periodCount: self.period_count,
        }
    }

    pub fn calculate_buffer_size_in_frames(
        &self,
        native_sample_rate: u32,
        performance_profile: PerformanceProfile,
    ) -> u32 {
        let descriptor: sys::ma_device_descriptor = unsafe { self.to_raw() };
        unsafe {
            sys::ma_calculate_buffer_size_in_frames_from_descriptor(
                &descriptor as *const _,
                native_sample_rate,
                performance_profile.into(),
            )
        }
    }
}

impl TryFrom<sys::ma_device_descriptor> for DeviceDescriptor {
    type Error = MaudioError;

    fn try_from(value: sys::ma_device_descriptor) -> Result<Self, Self::Error> {
        let mut channel_map: Vec<Channel> = vec![Channel::None; value.channels as usize];
        for &c in value.channelMap.iter().take(value.channels as usize) {
            channel_map.push(c.try_into()?);
        }

        Ok(Self {
            device_id: DeviceId::from_raw(unsafe { &*value.pDeviceID }),
            share_mode: value.shareMode.try_into()?,
            format: value.format.try_into()?,
            channels: value.channels,
            sample_rate: value.sampleRate.try_into()?,
            channel_map,
            period_size_frames: value.periodSizeInFrames,
            period_size_millis: value.periodSizeInMilliseconds,
            period_count: value.periodCount,
        })
    }
}
