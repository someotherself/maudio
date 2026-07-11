//! Audio device abstraction and control.
//!
//! Provides safe wrappers around `ma_device` for playback and capture.
use std::{
    cell::Cell,
    marker::PhantomData,
    mem::MaybeUninit,
    sync::{atomic::AtomicBool, Arc},
};

use maudio_sys::ffi as sys;

use crate::{
    backend::Backend,
    context::{ContextBuilder, ContextRef},
    device::{
        device_builder::{private_device_b, AsDeviceBuilder},
        device_id::DeviceId,
        device_info::DeviceInfo,
        device_state::DeviceState,
        device_type::DeviceType,
    },
    pcm_frames::PcmFormat,
    util::{device_notif::DeviceStateNotifier, proc_notif::ProcFramesNotif},
    Binding, MaResult,
};

pub mod device_builder;
pub(crate) mod device_cb_notif;
pub mod device_id;
pub mod device_info;
pub mod device_state;
pub mod device_type;

/// Owned audio device.
///
/// Manages the lifetime of a `ma_device` and provides control over
/// playback, capture, and device state.
pub struct Device<F: PcmFormat> {
    pub(crate) inner: Arc<DeviceInner<F>>,
    // Device cannot be sync.
    _not_sync: PhantomData<Cell<()>>,
}

pub(crate) struct DeviceInner<F: PcmFormat> {
    inner: *mut sys::ma_device,
    _playback_device_id: Option<DeviceId>, // Ref count. Needs to be kept alive.
    _capture_device_id: Option<DeviceId>,  // Ref count. Needs to be kept alive.
    callback_user_data: *mut core::ffi::c_void, // userdata (self.inner.pUserData)
    callback_user_data_drop: fn(*mut core::ffi::c_void), // destructor for the callback_user_data
    callback_panic: Arc<AtomicBool>,       // true = callback panicked and is now poisoned
    callback_process_notifier: ProcFramesNotif,
    state_notifier: Option<DeviceStateNotifier>, // used by ma_device_notification
    _format: PhantomData<F>,
}

impl<F: PcmFormat> Binding for DeviceInner<F> {
    type Raw = *mut sys::ma_device;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

// Required for Arc<DeviceInner> to implement Send
unsafe impl<F: PcmFormat> Send for DeviceInner<F> {}
unsafe impl<F: PcmFormat> Sync for DeviceInner<F> {}

impl<F: PcmFormat> Binding for Device<F> {
    type Raw = *mut sys::ma_device;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner.inner
    }
}

/// Borrowed view of the a `Device`. Typically returned from the `Engine`. Always in `f32` format.
pub struct DeviceRef<'a> {
    inner: *mut sys::ma_device,
    _keep_alive: PhantomData<&'a ()>,
}

impl Binding for DeviceRef<'_> {
    type Raw = *mut sys::ma_device;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            _keep_alive: PhantomData,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
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

mod private_device {
    use maudio_sys::ffi as sys;

    use crate::{
        device::{AsDevicePtr, CallBackDevice, Device, DeviceRef},
        pcm_frames::PcmFormat,
        Binding,
    };

    // Controls the Device functions that can be called from the data callback
    pub trait DeviceControl {}
    impl<F: PcmFormat> DeviceControl for Device<F> {}
    impl DeviceControl for DeviceRef<'_> {}

    pub trait DevicePtrProvider<T: ?Sized> {
        fn as_device_ptr(t: &T) -> *mut sys::ma_device;
    }

    pub struct DeviceProvider;
    pub struct DeviceRefProvider;
    pub struct CallBackDeviceRefProvider;

    impl<F: PcmFormat> DevicePtrProvider<Device<F>> for DeviceProvider {
        fn as_device_ptr(t: &Device<F>) -> *mut sys::ma_device {
            t.to_raw()
        }
    }

    impl DevicePtrProvider<DeviceRef<'_>> for DeviceRefProvider {
        fn as_device_ptr(t: &DeviceRef) -> *mut sys::ma_device {
            t.to_raw()
        }
    }

    impl DevicePtrProvider<CallBackDevice> for CallBackDeviceRefProvider {
        fn as_device_ptr(t: &CallBackDevice) -> *mut sys::ma_device {
            t.to_raw()
        }
    }

    pub fn device_ptr<T: AsDevicePtr + ?Sized>(t: &T) -> *mut sys::ma_device {
        <T as AsDevicePtr>::__PtrProvider::as_device_ptr(t)
    }
}

pub trait AsDevicePtr {
    type __PtrProvider: private_device::DevicePtrProvider<Self>;
}

impl<F: PcmFormat> AsDevicePtr for Device<F> {
    type __PtrProvider = private_device::DeviceProvider;
}

impl<'a> AsDevicePtr for DeviceRef<'a> {
    type __PtrProvider = private_device::DeviceRefProvider;
}

impl<F: PcmFormat> DeviceOps for Device<F> {}
impl DeviceOps for DeviceRef<'_> {}
impl DeviceOps for CallBackDevice {}

/// Methods shared between Device, DeviceRef and CallBackDevice
pub trait DeviceOps: AsDevicePtr {
    /// Retrieve the playback channels count
    ///
    /// Returns 0 if device is not setup for playback
    fn channels_playback(&self) -> u32 {
        unsafe { (*private_device::device_ptr(self)).playback.channels }
    }

    /// Retrieve the playback channels count
    ///
    /// Returns 0 if device is not setup for playback
    fn channels_capture(&self) -> u32 {
        unsafe { (*private_device::device_ptr(self)).capture.channels }
    }

    /// Returns the associated context, if available.
    fn get_context(&self) -> Option<ContextRef<'_>>
    where
        Self: private_device::DeviceControl,
    {
        device_ffi::ma_device_get_context(self)
    }

    /// Retrieves device information for the given type.
    fn get_info(&self, device_type: DeviceType) -> MaResult<DeviceInfo>
    where
        Self: private_device::DeviceControl,
    {
        device_ffi::ma_device_get_info(self, device_type)
    }

    /// Retrieves the human-readable name of the device.
    fn get_name(&self, device_type: DeviceType) -> MaResult<String>
    where
        Self: private_device::DeviceControl,
    {
        device_ffi::ma_device_get_name(self, device_type)
    }

    /// Returns `true` if the device is currently started.
    fn is_started(&self) -> bool {
        device_ffi::ma_device_is_started(self)
    }

    /// Returns the current state of the device. See [`DeviceState`]
    fn get_state(&self) -> MaResult<DeviceState> {
        device_ffi::ma_device_get_state(self)
    }

    /// Sets the master volume.
    ///
    /// Volume is linear, where `1.0` is unchanged.
    fn set_master_volume(&self, volume: f32) -> MaResult<()> {
        device_ffi::ma_device_set_master_volume(self, volume)
    }

    /// Returns the current master volume (linear scale).
    fn master_volume(&self) -> MaResult<f32> {
        device_ffi::ma_device_get_master_volume(self)
    }

    /// Returns the current master volume in decibels.
    fn master_volume_db(&self) -> MaResult<f32> {
        device_ffi::ma_device_get_master_volume_db(self)
    }
}

// Device only methods
impl<F: PcmFormat> Device<F> {
    /// Starts the device.
    ///
    /// Begins audio processing.
    pub fn device_start(&mut self) -> MaResult<()> {
        device_ffi::ma_device_start(self)
    }

    /// Stops the device.
    ///
    /// Halts audio processing.
    pub fn device_stop(&mut self) -> MaResult<()> {
        device_ffi::ma_device_stop(self)
    }

    /// Returns `true` if the data callback previously panicked.
    ///
    /// When this happens, the callback is considered poisoned and will no longer run.
    pub fn data_callback_panicked(&self) -> bool {
        self.inner
            .callback_panic
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Retrieves a [`ProcFramesNotif`] that fires when frames are processed inside the data callback
    ///
    /// `ProcFramesNotif` is cheap to clone, and this function can be safely called multiple times
    pub fn get_callback_notifier(&self) -> ProcFramesNotif {
        self.inner.callback_process_notifier.clone()
    }

    /// Retrieves a [`DeviceStateNotifier`] if one is present, that fires when the state of the device is changed
    ///
    /// `DeviceStateNotifier` is cheap to clone, and this function can be safely called multiple times
    pub fn get_state_notifier(&self) -> Option<DeviceStateNotifier> {
        self.inner.state_notifier.clone()
    }
}

// Private methods
impl<F: PcmFormat> Device<F> {
    pub(crate) fn new_with_config<'a, B: AsDeviceBuilder<'a> + ?Sized>(
        config: &B,
        context_cfg: Option<&ContextBuilder>,
        backends: Option<&[Backend]>,
        data_notif: ProcFramesNotif,
        playback_device_id: Option<DeviceId>,
        capture_device_id: Option<DeviceId>,
    ) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_device>> = Box::new(MaybeUninit::uninit());

        device_ffi::ma_device_init_ex(backends, context_cfg, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_device = Box::into_raw(mem) as *mut sys::ma_device;
        let Some(cb_info) = private_device_b::get_data_callback_info(config) else {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        };

        Ok(Self {
            inner: Arc::new(DeviceInner {
                inner,
                _playback_device_id: playback_device_id,
                _capture_device_id: capture_device_id,
                callback_user_data: cb_info.data_callback,
                callback_user_data_drop: cb_info.data_callback_drop,
                callback_panic: cb_info.data_callback_panic,
                callback_process_notifier: data_notif,
                state_notifier: Some(cb_info.state_notif.clone()),
                _format: PhantomData,
            }),
            _not_sync: PhantomData,
        })
    }
}

pub(crate) mod device_ffi {
    use std::mem::MaybeUninit;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::{performance::PerformanceProfile, sample_rate::SampleRate},
        backend::Backend,
        context::{Context, ContextBuilder, ContextRef},
        device::{
            device_builder::{private_device_b, AsDeviceBuilder},
            device_info::DeviceInfo,
            device_state::DeviceState,
            device_type::DeviceType,
            private_device, AsDevicePtr, Device, DeviceInner,
        },
        pcm_frames::PcmFormat,
        AsRawRef, Binding, MaResult, MaudioError,
    };

    #[allow(dead_code)]
    pub fn ma_device_init<'a, B: AsDeviceBuilder<'a>>(
        context: &mut Context,
        config: &B,
        device: *mut sys::ma_device,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_device_init(
                context.to_raw(),
                private_device_b::as_raw_ptr(config),
                device,
            )
        };
        MaudioError::check(res)
    }

    pub fn ma_device_init_ex<'a, B: AsDeviceBuilder<'a> + ?Sized>(
        backends: Option<&[Backend]>,
        context_cfg: Option<&ContextBuilder>,
        config: &B,
        device: *mut sys::ma_device,
    ) -> MaResult<()> {
        let (backends_ptr, length): (*const sys::ma_backend, u32) = if let Some(b) = backends {
            (b.as_ptr() as *const _, b.len() as u32)
        } else {
            (core::ptr::null(), 0)
        };

        let context_config = context_cfg.map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let res = unsafe {
            sys::ma_device_init_ex(
                backends_ptr,
                length,
                context_config,
                private_device_b::as_raw_ptr(config),
                device,
            )
        };
        MaudioError::check(res)
    }

    pub fn ma_device_uninit<F: PcmFormat>(device: &mut DeviceInner<F>) {
        unsafe { sys::ma_device_uninit(device.to_raw()) };
    }

    // Callback: not safe
    // Theadsafe: not safe
    pub fn ma_device_get_context<'a, D: AsDevicePtr + ?Sized>(
        device: &'a D,
    ) -> Option<ContextRef<'a>> {
        let ptr = unsafe { sys::ma_device_get_context(private_device::device_ptr(device)) };
        if ptr.is_null() {
            None
        } else {
            Some(ContextRef::from_ptr(ptr))
        }
    }

    // TODO: Implement log
    #[inline]
    #[allow(dead_code)]
    pub fn ma_device_get_log<D: AsDevicePtr + ?Sized>(context: &D) -> Option<*mut sys::ma_log> {
        let ptr = unsafe { sys::ma_device_get_log(private_device::device_ptr(context)) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    // Callback: not safe
    // Theadsafe: not safe
    #[inline]
    pub fn ma_device_get_info<D: AsDevicePtr + ?Sized>(
        device: &D,
        device_type: DeviceType,
    ) -> MaResult<DeviceInfo> {
        let mut info: MaybeUninit<sys::ma_device_info> = MaybeUninit::uninit();
        let res = unsafe {
            sys::ma_device_get_info(
                private_device::device_ptr(device),
                device_type.into(),
                info.as_mut_ptr(),
            )
        };
        MaudioError::check(res)?;

        Ok(DeviceInfo::new(unsafe { info.assume_init() }))
    }

    // Callback: not safe
    // Theadsafe: not safe
    // TODO: Add loop to check if name fits inside buffer
    #[inline]
    pub fn ma_device_get_name<D: AsDevicePtr + ?Sized>(
        device: &D,
        device_type: DeviceType,
    ) -> MaResult<String> {
        let cap: usize = 256;
        let mut len: usize = 0;

        let mut buf = vec![0u8; cap];

        let res = unsafe {
            sys::ma_device_get_name(
                private_device::device_ptr(device),
                device_type.into(),
                buf.as_mut_ptr() as *mut _,
                cap,
                &mut len as *mut _,
            )
        };
        MaudioError::check(res)?;
        Ok(String::from_utf8_lossy(&buf[..len]).into_owned())
    }

    // Callback: not safe
    // Theadsafe: SAFE
    #[inline]
    pub fn ma_device_start<F: PcmFormat>(device: &mut Device<F>) -> MaResult<()> {
        let res = unsafe { sys::ma_device_start(device.to_raw()) };
        MaudioError::check(res)
    }

    // Callback: not safe
    // Theadsafe: SAFE
    #[inline]
    pub fn ma_device_stop<F: PcmFormat>(device: &mut Device<F>) -> MaResult<()> {
        let res = unsafe { sys::ma_device_stop(device.to_raw()) };
        MaudioError::check(res)
    }

    // Callback: SAFE
    // Theadsafe: SAFE
    #[inline]
    pub fn ma_device_is_started<D: AsDevicePtr + ?Sized>(device: &D) -> bool {
        let res = unsafe { sys::ma_device_is_started(private_device::device_ptr(device)) };
        res == 1
    }

    // Callback: SAFE
    // Theadsafe: SAFE
    #[inline]
    pub fn ma_device_get_state<D: AsDevicePtr + ?Sized>(device: &D) -> MaResult<DeviceState> {
        let res = unsafe { sys::ma_device_get_state(private_device::device_ptr(device)) };
        res.try_into()
    }

    // Callback: not safe
    // Theadsafe: not safe
    // Not implemented. Only used for custom backends
    #[inline]
    #[allow(dead_code)]
    pub fn ma_device_post_init<D: AsDevicePtr + ?Sized>(
        device: &D,
        device_type: DeviceType,
        playback_descriptor: *const sys::ma_device_descriptor,
        capture_descriptor: *const sys::ma_device_descriptor,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_device_post_init(
                private_device::device_ptr(device),
                device_type.into(),
                playback_descriptor,
                capture_descriptor,
            )
        };
        MaudioError::check(res)
    }

    // Callback: SAFE
    // Theadsafe: SAFE
    #[inline]
    pub fn ma_device_set_master_volume<D: AsDevicePtr + ?Sized>(
        device: &D,
        volume: f32,
    ) -> MaResult<()> {
        let res =
            unsafe { sys::ma_device_set_master_volume(private_device::device_ptr(device), volume) };
        MaudioError::check(res)
    }

    // Callback: SAFE
    // Theadsafe: SAFE
    #[inline]
    pub fn ma_device_get_master_volume<D: AsDevicePtr + ?Sized>(device: &D) -> MaResult<f32> {
        let mut volume: f32 = 0.0;
        let res = unsafe {
            sys::ma_device_get_master_volume(private_device::device_ptr(device), &mut volume)
        };
        MaudioError::check(res)?;
        Ok(volume)
    }

    // Callback: SAFE
    // Theadsafe: SAFE
    #[inline]
    pub fn ma_device_get_master_volume_db<D: AsDevicePtr + ?Sized>(device: &D) -> MaResult<f32> {
        let mut volume: f32 = 0.0;
        let res = unsafe {
            sys::ma_device_get_master_volume_db(private_device::device_ptr(device), &mut volume)
        };
        MaudioError::check(res)?;
        Ok(volume)
    }

    // Callback: called by miniaudio
    // Theadsafe: called by miniaudio
    // Not implemented. Only used for custom backends
    #[inline]
    #[allow(dead_code)]
    pub fn ma_device_handle_backend_data_callback<D: AsDevicePtr + ?Sized>(
        device: &D,
        output: *mut core::ffi::c_void,
        input: *const core::ffi::c_void,
        frame_count: u32,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_device_handle_backend_data_callback(
                private_device::device_ptr(device),
                output,
                input,
                frame_count,
            )
        };
        MaudioError::check(res)
    }

    // Callback: called by miniaudio
    // Theadsafe: called by miniaudio
    // Not implemented. Only used for custom backends
    #[inline]
    #[allow(dead_code)]
    pub fn ma_calculate_buffer_size_in_frames_from_descriptor(
        descriptor: *const sys::ma_device_descriptor,
        native_sample_rate: SampleRate,
        performance_profile: PerformanceProfile,
    ) -> u32 {
        unsafe {
            sys::ma_calculate_buffer_size_in_frames_from_descriptor(
                descriptor,
                native_sample_rate.into(),
                performance_profile.into(),
            )
        }
    }
}

impl<F: PcmFormat> Drop for DeviceInner<F> {
    fn drop(&mut self) {
        device_ffi::ma_device_uninit(self);
        (self.callback_user_data_drop)(self.callback_user_data);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}
