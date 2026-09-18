use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::MaybeUninit,
    sync::{atomic::AtomicBool, Arc, OnceLock},
};

use maudio_sys::ffi as sys;

use crate::{
    backend::{
        custom_backend::CustomBackend,
        custom_context::{CustomContext, CustomContextInner},
    },
    device::{
        device_builder::{private_device_b, AsDeviceBuilder},
        device_ffi,
        device_id::DeviceId,
        private_device, AsDevicePtr,
    },
    logging::{LogInner, StoredLogs},
    pcm_frames::PcmFormat,
    util::{device_notif::DeviceStateNotifier, proc_notif::ProcFramesNotif},
    Binding, MaResult,
};

pub struct CustomDevice<F: PcmFormat, B: CustomBackend> {
    pub(crate) inner: Arc<CustomDeviceInner<B>>,
    format: PhantomData<F>,
}

#[repr(C)]
pub(crate) struct CustomDeviceInner<B: CustomBackend> {
    pub(crate) inner: UnsafeCell<sys::ma_device>,
    pub(crate) context: Arc<CustomContextInner<B>>,
    pub(crate) backend_device: OnceLock<B::Device>,
    pub(super) log: Option<Arc<LogInner>>,
    _playback_device_id: Option<DeviceId>, // Ref count. Needs to be kept alive.
    _capture_device_id: Option<DeviceId>,  // Ref count. Needs to be kept alive.
    callback_user_data: *mut core::ffi::c_void, // userdata (self.inner.pUserData)
    callback_user_data_drop: fn(*mut core::ffi::c_void), // destructor for the callback_user_data
    callback_panic: Arc<AtomicBool>,       // true = callback panicked and is now poisoned
    callback_process_notifier: ProcFramesNotif,
    state_notifier: Option<DeviceStateNotifier>, // used by ma_device_notification
    pub(crate) logs: StoredLogs,                 // TODO: Add it in the LogOwner
    backend: PhantomData<B>,
}

impl<F: PcmFormat, B: CustomBackend> Binding for CustomDevice<F, B> {
    type Raw = *mut sys::ma_device;

    fn to_raw(&self) -> Self::Raw {
        self.inner.inner.get()
    }
}

impl<F: PcmFormat, B: CustomBackend> AsDevicePtr for CustomDevice<F, B> {
    type __PtrProvider = private_device::CustomDeviceProvider;
}

// Private methods
impl<'a, F: PcmFormat, B: CustomBackend> CustomDevice<F, B> {
    pub(crate) fn new_with_config<D: AsDeviceBuilder<'a> + ?Sized>(
        config: &D,
        context: &'a CustomContext<B>,
        data_notif: ProcFramesNotif,
        playback_device_id: Option<DeviceId>,
        capture_device_id: Option<DeviceId>,
    ) -> MaResult<CustomDevice<F, B>> {
        let Some(cb_info) = private_device_b::get_data_callback_info(config) else {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        };

        let inner: Arc<CustomDeviceInner<B>> = Arc::new(CustomDeviceInner {
            inner: unsafe { MaybeUninit::zeroed().assume_init() },
            context: context.0.clone(),
            backend_device: OnceLock::new(),
            log: context.0.log.clone(),
            _playback_device_id: playback_device_id,
            _capture_device_id: capture_device_id,
            callback_user_data: cb_info.data_callback,
            callback_user_data_drop: cb_info.data_callback_drop,
            callback_panic: cb_info.data_callback_panic,
            callback_process_notifier: data_notif,
            state_notifier: Some(cb_info.state_notif.clone()),
            logs: StoredLogs::default(),
            backend: PhantomData,
        });

        let base_ptr = core::ptr::addr_of!(inner.inner);

        device_ffi::ma_device_init(context.to_raw(), config, base_ptr as *mut _)?;

        let inner_ptr = Arc::as_ptr(&inner) as *mut CustomDeviceInner<B>;

        debug_assert_eq!(
            unsafe { core::ptr::addr_of_mut!((*inner_ptr).inner) }.cast::<u8>(),
            inner_ptr.cast::<u8>(),
        );

        Ok(CustomDevice {
            inner,
            format: PhantomData,
        })
    }
}

// Device only methods
impl<F: PcmFormat, B: CustomBackend> CustomDevice<F, B> {
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

    // TODO
    // pub fn log(&self) -> LogRef {
    //     device_ffi::ma_device_get_log(self)
    // }

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

impl<B: CustomBackend> Drop for CustomDeviceInner<B> {
    fn drop(&mut self) {
        device_ffi::ma_device_uninit(self.inner.get());
        (self.callback_user_data_drop)(self.callback_user_data);
    }
}

#[derive(Copy)]
pub struct BackendDeviceHandle<B: CustomBackend>(pub(crate) *mut CustomDeviceInner<B>);

impl<B: CustomBackend> Clone for BackendDeviceHandle<B> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<B: CustomBackend> Binding for BackendDeviceHandle<B> {
    type Raw = *mut sys::ma_device;

    fn to_raw(&self) -> Self::Raw {
        unsafe { &*self.0 }.inner.get()
    }
}

unsafe impl<B: CustomBackend> Send for BackendDeviceHandle<B> {}

impl<B: CustomBackend> BackendDeviceHandle<B> {
    pub fn user_device(&self) -> Option<&B::Device> {
        let device = unsafe { &*self.0 };
        device.backend_device.get()
    }

    pub fn handle_backend_data_callback<F: PcmFormat, R: PcmFormat>(
        &self,
        output: Option<&mut [F::StorageUnit]>,
        input: Option<&[R::StorageUnit]>,
    ) -> MaResult<()> {
        device_ffi::ma_device_handle_backend_data_callback::<F, R, B>(self, output, input)
    }

    pub fn with_context<F, R>(&self, f: F) -> MaResult<R>
    where
        for<'b> F: FnOnce(&'b CustomContext<B>) -> MaResult<R>,
    {
        let ctx = unsafe { &*self.0 }.context.clone();
        let ctx = CustomContext(ctx);
        f(&ctx)
    }

    pub fn backend_context(&self) -> &B::Context {
        let inner_ptr = unsafe { &*self.0 };
        let ptr: *mut CustomContextInner<B> =
            unsafe { &*inner_ptr.inner.get() }.pContext as *mut CustomContextInner<B>;
        unsafe { &*ptr }.backend_context.get().unwrap()
    }
}
