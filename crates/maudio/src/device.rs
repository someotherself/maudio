use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    sync::{atomic::AtomicBool, Arc},
};

use maudio_sys::ffi as sys;

use crate::{
    backend::Backend,
    context::{ContextBuilder, ContextRef},
    device::{
        device_builder::{private_device_b, AsDeviceBuilder, CallBackDevice},
        device_info::DeviceInfo,
        device_state::DeviceState,
        device_type::DeviceType,
    },
    Binding, MaResult,
};

pub mod device_builder;
pub mod device_id;
pub mod device_info;
pub mod device_state;
pub mod device_type;

pub struct Device {
    inner: Arc<DeviceInner>,
}

pub(crate) struct DeviceInner {
    inner: *mut sys::ma_device,
    callback_user_data: *mut core::ffi::c_void, // userdata (self.inner.pUserData)
    callback_user_data_drop: fn(*mut core::ffi::c_void), // destructor for the callback_user_data
    callback_panic: Arc<AtomicBool>,            // true = callback panicked and is now poisoned
}

impl Binding for DeviceInner {
    type Raw = *mut sys::ma_device;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

unsafe impl Send for DeviceInner {}
unsafe impl Sync for DeviceInner {}

impl Binding for Device {
    type Raw = *mut sys::ma_device;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner.inner
    }
}

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

mod private_device {
    use maudio_sys::ffi as sys;

    use crate::{
        device::{device_builder::CallBackDevice, AsDevicePtr, Device, DeviceRef},
        Binding,
    };

    // Controls the Device functions that can be called from the data callback
    pub trait DeviceControl {}
    impl DeviceControl for Device {}
    impl DeviceControl for DeviceRef<'_> {}

    pub trait DevicePtrProvider<T: ?Sized> {
        fn as_device_ptr(t: &T) -> *mut sys::ma_device;
    }

    pub struct DeviceProvider;
    pub struct DeviceRefProvider;
    pub struct CallBackDeviceRefProvider;

    impl DevicePtrProvider<Device> for DeviceProvider {
        fn as_device_ptr(t: &Device) -> *mut sys::ma_device {
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

impl AsDevicePtr for Device {
    type __PtrProvider = private_device::DeviceProvider;
}

impl<'a> AsDevicePtr for DeviceRef<'a> {
    type __PtrProvider = private_device::DeviceRefProvider;
}

impl DeviceOps for Device {}
impl DeviceOps for DeviceRef<'_> {}
impl DeviceOps for CallBackDevice {}

/// Methods shared between Device, DeviceRef and CallBackDevice
pub trait DeviceOps: AsDevicePtr {
    fn get_context(&self) -> Option<ContextRef<'_>>
    where
        Self: private_device::DeviceControl,
    {
        device_ffi::ma_device_get_context(self)
    }

    fn get_info(&self, device_type: DeviceType) -> MaResult<DeviceInfo>
    where
        Self: private_device::DeviceControl,
    {
        device_ffi::ma_device_get_info(self, device_type)
    }

    fn get_name(&self, device_type: DeviceType) -> MaResult<String>
    where
        Self: private_device::DeviceControl,
    {
        device_ffi::ma_device_get_name(self, device_type)
    }

    fn is_started(&self) -> bool {
        device_ffi::ma_device_is_started(self)
    }

    fn get_state(&self) -> MaResult<DeviceState> {
        device_ffi::ma_device_get_state(self)
    }

    fn set_master_volume(&self, volume: f32) -> MaResult<()> {
        device_ffi::ma_device_set_master_volume(self, volume)
    }

    fn master_volume(&self) -> MaResult<f32> {
        device_ffi::ma_device_get_master_volume(self)
    }

    fn master_volume_db(&self) -> MaResult<f32> {
        device_ffi::ma_device_get_master_volume_db(self)
    }
}

// Device only methods
impl Device {
    pub fn device_start(&mut self) -> MaResult<()> {
        device_ffi::ma_device_start(self)
    }

    pub fn device_stop(&mut self) -> MaResult<()> {
        device_ffi::ma_device_stop(self)
    }

    pub fn data_callback_panicked(&self) -> bool {
        self.inner
            .callback_panic
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

// Private methods
impl Device {
    pub(crate) fn new_with_config<'a, B: AsDeviceBuilder<'a> + ?Sized>(
        config: &B,
        context_cfg: Option<&ContextBuilder>,
        backends: Option<&[Backend]>,
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
                callback_user_data: cb_info.data_callback,
                callback_user_data_drop: cb_info.data_callback_drop,
                callback_panic: cb_info.data_callback_panic,
            }),
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
        AsRawRef, Binding, MaResult, MaudioError,
    };

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

    pub fn ma_device_uninit(device: &mut DeviceInner) {
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
    pub fn ma_device_start(device: &mut Device) -> MaResult<()> {
        let res = unsafe { sys::ma_device_start(device.to_raw()) };
        MaudioError::check(res)
    }

    // Callback: not safe
    // Theadsafe: SAFE
    #[inline]
    pub fn ma_device_stop(device: &mut Device) -> MaResult<()> {
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
    // TODO
    // Only used for custom backends
    #[inline]
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
    // TODO
    #[inline]
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
    // TODO
    #[inline]
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

impl Drop for DeviceInner {
    fn drop(&mut self) {
        device_ffi::ma_device_uninit(self);
        (self.callback_user_data_drop)(self.callback_user_data);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}
