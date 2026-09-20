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
    audio::channels::Channel,
    backend::Backend,
    context::{ContextBuilder, ContextInner, ContextRef},
    device::{
        device_builder::{private_device_b, AsDeviceBuilder, DeviceContextStore},
        device_id::DeviceId,
        device_info::DeviceInfo,
        device_state::DeviceState,
        device_type::DeviceType,
    },
    logging::{LogRef, StoredLogs},
    pcm_frames::PcmFormat,
    util::{device_notif::DeviceStateNotifier, proc_notif::ProcFramesNotif},
    AllocationCallbacks, Binding, MaResult,
};

pub mod custom_device;
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
    pub(crate) inner: Arc<DeviceInner>,
    _format: PhantomData<F>,
    // Device cannot be sync.
    _not_sync: PhantomData<Cell<()>>,
}

#[doc(hidden)]
pub struct DeviceInner {
    inner: *mut sys::ma_device,
    _playback_device_id: Option<DeviceId>, // Ref count. Needs to be kept alive.
    _capture_device_id: Option<DeviceId>,  // Ref count. Needs to be kept alive.
    _context: Option<Arc<ContextInner>>,   // keep alive
    callback_user_data: *mut core::ffi::c_void, // userdata (self.inner.pUserData - ErasedBackendState)
    callback_user_data_drop: fn(*mut core::ffi::c_void), // destructor for the callback_user_data
    callback_panic: Arc<AtomicBool>,            // true = callback panicked and is now poisoned
    callback_process_notifier: ProcFramesNotif,
    state_notifier: Option<DeviceStateNotifier>, // used by ma_device_notification
    pub(crate) logs: StoredLogs,
}

impl Binding for DeviceInner {
    type Raw = *mut sys::ma_device;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

// Required for Arc<DeviceInner> to implement Send
unsafe impl Send for DeviceInner {}
unsafe impl Sync for DeviceInner {}

impl<F: PcmFormat> Binding for Device<F> {
    type Raw = *mut sys::ma_device;

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

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a> DeviceRef<'a> {
    #[allow(unused)]
    pub(crate) fn from_ptr(ptr: *mut sys::ma_device) -> Self {
        Self {
            inner: ptr,
            _keep_alive: PhantomData,
        }
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

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl CallBackDevice {
    pub(crate) fn from_ptr(ptr: *mut sys::ma_device) -> Self {
        Self { inner: ptr }
    }
}

impl AsDevicePtr for CallBackDevice {
    type __PtrProvider = private_device::CallBackDeviceRefProvider;
}

pub(crate) mod private_device {
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

    /// Retrieve the playback channel map
    fn channel_map_playback(&self) -> MaResult<Vec<Channel>> {
        let raw_map = unsafe { (*private_device::device_ptr(self)).playback.channelMap };
        let channels = unsafe { (*private_device::device_ptr(self)).playback.channels };

        let mut map = Vec::new();
        for &entry in raw_map.iter().take(channels as usize) {
            map.push(Channel::try_from(entry)?);
        }
        Ok(map)
    }

    /// Retrieve the capture channel map
    fn channel_map_capture(&self) -> MaResult<Vec<Channel>> {
        let raw_map = unsafe { (*private_device::device_ptr(self)).capture.channelMap };
        let channels = unsafe { (*private_device::device_ptr(self)).capture.channels };

        let mut map = Vec::new();
        for &entry in raw_map.iter().take(channels as usize) {
            map.push(Channel::try_from(entry)?);
        }
        Ok(map)
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
        device_ffi::ma_device_start(self.to_raw())
    }

    /// Stops the device.
    ///
    /// Halts audio processing.
    pub fn device_stop(&mut self) -> MaResult<()> {
        device_ffi::ma_device_stop(self.to_raw())
    }

    pub fn log(&self) -> LogRef {
        device_ffi::ma_device_get_log(self)
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
    pub(crate) fn new_with_config<B: AsDeviceBuilder + ?Sized>(
        config: &B,
        context: Option<DeviceContextStore>,
        data_notif: ProcFramesNotif,
        playback_device_id: Option<DeviceId>,
        capture_device_id: Option<DeviceId>,
    ) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_device>> = Box::new(MaybeUninit::uninit());

        let mut std_ctx = None;
        let ctx = if let Some(ctx) = context {
            match ctx {
                DeviceContextStore::Ctx(inner) => {
                    std_ctx = Some(inner.clone());
                    Some(inner.to_raw())
                }
                DeviceContextStore::Custom(p) => Some(p),
            }
        } else {
            None
        };
        device_ffi::ma_device_init(ctx, config, mem.as_mut_ptr())?;

        println!("device init returned");

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
                _context: std_ctx,
                callback_user_data: cb_info.data_callback,
                callback_user_data_drop: cb_info.data_callback_drop,
                callback_panic: cb_info.data_callback_panic,
                callback_process_notifier: data_notif,
                state_notifier: Some(cb_info.state_notif.clone()),
                logs: StoredLogs::default(),
            }),
            _format: PhantomData,
            _not_sync: PhantomData,
        })
    }

    #[allow(unused)]
    pub(crate) fn new_ex_with_config<B: AsDeviceBuilder + ?Sized>(
        config: &B,
        context_cfg: Option<&ContextBuilder>,
        backends: Option<&[Backend]>,
        data_notif: ProcFramesNotif,
        playback_device_id: Option<DeviceId>,
        capture_device_id: Option<DeviceId>,
    ) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_device>> = Box::new(MaybeUninit::uninit());

        // Miniaudio passes in the alloc cb to the device via the context
        // If user does not create a context, but uses the global alloc feature, we need to create a context
        let owned_context = (context_cfg.is_none()
            && AllocationCallbacks::clone_callbacks().is_some())
        .then(ContextBuilder::new);

        let context_cfg = context_cfg.or(owned_context.as_ref());

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
                _context: None,
                callback_user_data: cb_info.data_callback,
                callback_user_data_drop: cb_info.data_callback_drop,
                callback_panic: cb_info.data_callback_panic,
                callback_process_notifier: data_notif,
                state_notifier: Some(cb_info.state_notif.clone()),
                logs: StoredLogs::default(),
            }),
            _format: PhantomData,
            _not_sync: PhantomData,
        })
    }
}

pub(crate) mod device_ffi {
    use std::mem::MaybeUninit;

    use maudio_sys::ffi as sys;

    use crate::{
        backend::Backend,
        context::{ContextBuilder, ContextRef},
        device::{
            device_builder::{private_device_b, AsDeviceBuilder},
            device_info::DeviceInfo,
            device_state::DeviceState,
            device_type::DeviceType,
            private_device, AsDevicePtr, Device,
        },
        logging::{LogOwner, LogRef},
        pcm_frames::PcmFormat,
        AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
    };

    pub fn ma_device_init<B: AsDeviceBuilder + ?Sized>(
        context: Option<*mut sys::ma_context>,
        config: &B,
        device: *mut sys::ma_device,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_device_init(
                context.unwrap_or(std::ptr::null_mut()),
                private_device_b::as_raw_ptr(config),
                device,
            )
        };
        MaudioError::check(res)
    }

    pub fn ma_device_init_ex<B: AsDeviceBuilder + ?Sized>(
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

    pub fn ma_device_uninit(device: *mut sys::ma_device) {
        unsafe { sys::ma_device_uninit(device) };
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

    // Callback: not safe
    // Theadsafe: not safe
    #[inline]
    pub fn ma_device_get_log<F: PcmFormat>(device: &Device<F>) -> LogRef {
        let ptr = unsafe { sys::ma_device_get_log(device.to_raw()) };

        LogRef {
            inner: ptr,
            _owner: LogOwner::Device(device.inner.clone()),
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
    pub fn ma_device_start(device: *mut sys::ma_device) -> MaResult<()> {
        let res = unsafe { sys::ma_device_start(device) };
        MaudioError::check(res)
    }

    // Callback: not safe
    // Theadsafe: SAFE
    #[inline]
    pub fn ma_device_stop(device: *mut sys::ma_device) -> MaResult<()> {
        let res = unsafe { sys::ma_device_stop(device) };
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

    // TODO: Can this API be improved?
    // Callback: called by miniaudio
    // Theadsafe: called by miniaudio
    #[inline]
    pub fn ma_device_handle_backend_data_callback<F: PcmFormat, R: PcmFormat>(
        device: *mut sys::ma_device,
        output: Option<&mut [F::StorageUnit]>,
        input: Option<&[R::StorageUnit]>,
    ) -> MaResult<()> {
        if output.is_none() && input.is_none() {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "At least one buffer must be valid",
            )));
        }

        let invalid = |message| MaudioError::new_ma_error(ErrorKinds::InvalidOperation(message));

        let count_frames = |len: usize, units_per_frame: usize| -> MaResult<u32> {
            if units_per_frame == 0 || len % units_per_frame != 0 {
                return Err(invalid("Buffer must contain a whole number of frames"));
            }

            u32::try_from(len / units_per_frame)
                .map_err(|_| invalid("Frame count exceeds u32::MAX"))
        };

        let output_frames = output
            .as_ref()
            .map(|buffer| {
                let channels = unsafe { (*device).playback.internalChannels } as usize;

                let units_per_frame = F::VEC_STORE_UNITS_PER_FRAME
                    .checked_mul(channels)
                    .ok_or_else(|| invalid("Output frame size overflow"))?;

                count_frames(buffer.len(), units_per_frame)
            })
            .transpose()?;

        let input_frames = input
            .as_ref()
            .map(|buffer| {
                let channels = unsafe { (*device).capture.internalChannels } as usize;

                let units_per_frame = R::VEC_STORE_UNITS_PER_FRAME
                    .checked_mul(channels)
                    .ok_or_else(|| invalid("Input frame size overflow"))?;

                count_frames(buffer.len(), units_per_frame)
            })
            .transpose()?;

        let frame_count = match (output_frames, input_frames) {
            (Some(output), Some(input)) => {
                if output != input {
                    return Err(invalid("Input and output frame counts must match"));
                }
                output
            }
            (Some(count), None) | (None, Some(count)) => count,
            (None, None) => {
                return Err(invalid("At least one buffer must be valid"));
            }
        };

        let output = output.map_or(std::ptr::null_mut(), |o| o.as_mut_ptr());
        let input = input.map_or(std::ptr::null(), |i| i.as_ptr());

        let res = unsafe {
            sys::ma_device_handle_backend_data_callback(
                device,
                output as *mut _,
                input as *const _,
                frame_count,
            )
        };
        MaudioError::check(res)
    }
}

impl Drop for DeviceInner {
    fn drop(&mut self) {
        device_ffi::ma_device_uninit(self.to_raw());
        (self.callback_user_data_drop)(self.callback_user_data);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}
