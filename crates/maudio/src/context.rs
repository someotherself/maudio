//! Audio backend context and device discovery.
//!
//! A [`Context`] represents an initialized miniaudio backend context. It is primarily used
//! for device enumeration, device information queries, backend selection, and capability checks.
//!
//! In most applications, a single shared `Context` is enough.
//!
//! # Context vs. Device
//!
//! [`Context`] and [`Device`](crate::device::Device) serve different roles:
//!
//! - [`Context`] manages backend-level operations such as initialization and device discovery
//! - [`Device`](crate::device::Device) represents an opened audio stream used for playback,
//!   capture, or duplex I/O
//!
//! A `Context` is commonly used to discover devices and choose a [`DeviceId`]
//! before creating a `Device`.
//!
//! # Enumerating devices
//!
//! Device enumeration is available in three forms:
//!
//! - [`ContextOps::get_devices`] for an owned snapshot
//! - [`ContextOps::with_devices`] for temporary borrowed slices
//! - [`ContextOps::enumerate_devices`] for lightweight callback-based iteration
//!
//! # Ownership
//!
//! [`Context`] is the owning handle to the backend context, while [`ContextRef`] is a borrowed
//! view. Both implement [`ContextOps`].
use core::slice;
use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::Arc,
};

use maudio_sys::ffi as sys;

use crate::{
    backend::Backend,
    device::{
        device_id::DeviceId,
        device_info::{DeviceBasicInfo, DeviceInfo, Devices},
        device_type::DeviceType,
    },
    engine::AllocationCallbacks,
    AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
};

/// An owning handle to a miniaudio context.
///
/// A context is the entry point for backend-level audio operations such as:
///
/// - device enumeration
/// - querying device information
/// - checking backend capabilities
/// - creating devices against a specific backend context
///
/// For temporary borrowed access, see [`ContextRef`].
#[derive(Clone)]
pub struct Context {
    inner: Arc<ContextInner>,
}

pub(crate) struct ContextInner {
    inner: *mut sys::ma_context,
}

impl Binding for ContextInner {
    type Raw = *mut sys::ma_context;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

unsafe impl Send for ContextInner {}
unsafe impl Sync for ContextInner {}

impl Binding for Context {
    type Raw = *mut sys::ma_context;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner.inner
    }
}

/// A borrowed reference to a miniaudio context.
///
/// `ContextRef` does not own the underlying context. It is mainly useful for APIs that need
/// to expose temporary context access without cloning or transferring ownership.
///
/// The lifetime `'a` ties this reference to the source that produced it.
pub struct ContextRef<'a> {
    inner: *mut sys::ma_context,
    _keep_alive: PhantomData<&'a ()>,
}

impl Binding for ContextRef<'_> {
    type Raw = *mut sys::ma_context;

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

mod private_context {
    use maudio_sys::ffi as sys;

    use crate::{
        context::{AsContextPtr, Context, ContextRef},
        Binding,
    };

    pub trait ContextPtrProvider<T: ?Sized> {
        fn as_context_ptr(t: &T) -> *mut sys::ma_context;
    }

    pub struct ContextProvider;
    pub struct ContextRefProvider;

    impl ContextPtrProvider<Context> for ContextProvider {
        fn as_context_ptr(t: &Context) -> *mut sys::ma_context {
            t.to_raw()
        }
    }

    impl ContextPtrProvider<ContextRef<'_>> for ContextRefProvider {
        fn as_context_ptr(t: &ContextRef) -> *mut sys::ma_context {
            t.to_raw()
        }
    }

    pub fn context_ptr<T: AsContextPtr + ?Sized>(t: &T) -> *mut sys::ma_context {
        <T as AsContextPtr>::__PtrProvider::as_context_ptr(t)
    }
}

pub trait AsContextPtr {
    type __PtrProvider: private_context::ContextPtrProvider<Self>;
}

impl AsContextPtr for Context {
    type __PtrProvider = private_context::ContextProvider;
}

impl<'a> AsContextPtr for ContextRef<'a> {
    type __PtrProvider = private_context::ContextRefProvider;
}

impl ContextOps for Context {}
impl ContextOps for ContextRef<'_> {}

/// Common operations available on both owned and borrowed context handles.
///
/// This trait is implemented for [`Context`] and [`ContextRef`], allowing the same
/// enumeration and query APIs to be used regardless of ownership.
pub trait ContextOps: AsContextPtr {
    /// Returns the currently available playback and capture devices.
    ///
    /// This method copies the device information reported by miniaudio into owned Rust values.
    /// The returned [`Devices`] collection is therefore independent of the temporary buffers used
    /// by the backend during enumeration.
    ///
    /// Use this when you want to keep device information around after the call returns.
    fn get_devices(&self) -> MaResult<Devices> {
        let (playback_info, playback_count, capture_info, capture_count) =
            context_ffi::ma_context_get_devices(self)?;

        let playback = if playback_count != 0 && !playback_info.is_null() {
            let tmp_playback =
                unsafe { slice::from_raw_parts(playback_info, playback_count as usize) };
            tmp_playback.iter().cloned().map(DeviceInfo::new).collect()
        } else {
            Vec::new()
        };

        let capture = if capture_count != 0 && !capture_info.is_null() {
            let tmp_capture =
                unsafe { slice::from_raw_parts(capture_info, capture_count as usize) };
            tmp_capture.iter().cloned().map(DeviceInfo::new).collect()
        } else {
            Vec::new()
        };

        Ok(Devices::from_owned(playback, capture))
    }

    /// Exposes the currently available playback and capture devices as borrowed slices.
    ///
    /// The closure `f` receives two slices:
    ///
    /// - `playback`: all available playback devices
    /// - `capture`: all available capture devices
    ///
    /// The slices passed to the closure are only valid for the duration of `f`. They borrow
    /// the temporary device list returned by miniaudio and must not be stored.
    ///
    /// Compared to [`get_devices`](Self::get_devices), this avoids allocating and copying the
    /// device list into owned Rust values.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use maudio::context::{ContextBuilder, ContextOps};
    /// let ctx = ContextBuilder::new().build().unwrap();
    ///
    /// ctx.with_devices(|playback, capture| {
    ///     for device in playback {
    ///         println!("Playback: {}", device.device_name());
    ///     }
    ///
    ///     for device in capture {
    ///         println!("Capture: {}", device.device_name());
    ///     }
    /// }).unwrap();
    /// ```
    fn with_devices<F>(&self, f: F) -> MaResult<()>
    where
        F: FnOnce(&[DeviceInfo], &[DeviceInfo]),
    {
        let (playback_info, playback_count, capture_info, capture_count) =
            context_ffi::ma_context_get_devices(self)?;

        let playback = if playback_count == 0 {
            &[]
        } else if playback_info.is_null() {
            return Err(crate::MaudioError::from_ma_result(sys::ma_result_MA_ERROR));
        } else {
            unsafe {
                slice::from_raw_parts(playback_info as *const DeviceInfo, playback_count as usize)
            }
        };

        let capture = if capture_count == 0 {
            &[]
        } else if capture_info.is_null() {
            return Err(crate::MaudioError::from_ma_result(sys::ma_result_MA_ERROR));
        } else {
            unsafe {
                slice::from_raw_parts(capture_info as *const DeviceInfo, capture_count as usize)
            }
        };

        f(playback, capture);

        Ok(())
    }

    /// Queries detailed information for a specific device ID.
    ///
    /// This is typically used after obtaining a [`DeviceId`] from enumeration to fetch the
    /// latest backend-reported details for that device.
    fn device_info(&self, device_type: DeviceType, device_id: &DeviceId) -> MaResult<DeviceInfo> {
        context_ffi::ma_context_get_device_info(self, device_type, device_id)
    }

    /// Returns whether the active backend configuration supports loopback devices.
    ///
    /// Loopback support is backend and platform specific.
    fn is_loopback_supported(&self) -> bool {
        context_ffi::ma_context_is_loopback_supported(self)
    }

    /// Enumerates devices one by one via a callback.
    ///
    /// This is the most lightweight device enumeration API. Unlike
    /// [`get_devices`](Self::get_devices), it does not build and return owned device lists.
    /// Instead, each device is reported immediately to the callback as it is enumerated.
    ///
    /// The callback receives:
    ///
    /// - the [`DeviceType`] of the current device
    /// - a borrowed [`DeviceBasicInfo`] describing that device
    ///
    /// This method is useful when you only need lightweight information such as the
    /// device name, ID, or default-device status, or when you want to stop enumeration
    /// early after finding a matching device.
    ///
    /// Return [`EnumerateControl::Continue`] to keep enumerating, or
    /// [`EnumerateControl::Stop`] to stop early.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use maudio::context::{ContextBuilder, ContextOps, EnumerateControl};
    /// let ctx = ContextBuilder::new().build().unwrap();
    ///
    /// ctx.enumerate_devices(|device_type, info| {
    ///     println!("{}: {}", device_type, info.name());
    ///
    ///     if info.is_default() {
    ///         println!("found default device");
    ///         return EnumerateControl::Stop;
    ///     }
    ///
    ///     EnumerateControl::Continue
    /// })?;
    /// # Ok::<(), maudio::MaudioError>(())
    /// ```
    fn enumerate_devices<F>(&self, mut f: F) -> MaResult<()>
    where
        F: FnMut(DeviceType, DeviceBasicInfo<'_>) -> EnumerateControl,
    {
        let mut state = EnumerateState {
            f: &mut f,
            err: None,
        };

        context_ffi::ma_context_enumerate_devices(
            self,
            Some(enumerate_devices_callback::<F>),
            (&mut state as *mut EnumerateState<'_, F>).cast::<core::ffi::c_void>(),
        )?;

        if let Some(err) = state.err {
            return Err(err);
        }
        Ok(())
    }
}

// Private methods
impl Context {
    fn new_with_config(config: &ContextBuilder) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_context>> = Box::new(MaybeUninit::uninit());

        context_ffi::ma_context_init(config.backends, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_context = Box::into_raw(mem) as *mut sys::ma_context;

        Ok(Self {
            inner: Arc::new(ContextInner { inner }),
        })
    }
}

pub(crate) mod context_ffi {
    use std::mem::MaybeUninit;

    use maudio_sys::ffi as sys;

    use crate::{
        backend::Backend,
        context::{private_context, AsContextPtr, Context, ContextBuilder, ContextInner},
        device::{device_id::DeviceId, device_info::DeviceInfo, device_type::DeviceType},
        AsRawRef, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_context_init(
        backends: Option<&[Backend]>,
        config: &ContextBuilder,
        context: *mut sys::ma_context,
    ) -> MaResult<()> {
        let (backend, backend_count): (*const sys::ma_backend, u32) =
            backends.map_or((core::ptr::null(), 0), |list| {
                if !list.is_empty() {
                    (list.as_ptr() as *const sys::ma_backend, list.len() as u32)
                } else {
                    (core::ptr::null(), 0)
                }
            });
        let res =
            unsafe { sys::ma_context_init(backend, backend_count, config.as_raw_ptr(), context) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_context_uninit(context: &mut ContextInner) -> MaResult<()> {
        let res = unsafe { sys::ma_context_uninit(context.to_raw()) };
        MaudioError::check(res)
    }

    /// Not needed
    #[inline]
    pub fn ma_context_sizeof() -> usize {
        unsafe { sys::ma_context_sizeof() }
    }

    // TODO: Implement log
    #[inline]
    pub fn ma_context_get_log(context: &Context) -> Option<*mut sys::ma_log> {
        let ptr = unsafe { sys::ma_context_get_log(context.to_raw()) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    #[inline]
    pub fn ma_context_enumerate_devices<C: AsContextPtr + ?Sized>(
        context: &C,
        callback: sys::ma_enum_devices_callback_proc,
        user_data: *mut core::ffi::c_void,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_context_enumerate_devices(
                private_context::context_ptr(context),
                callback,
                user_data,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_context_get_devices<C: AsContextPtr + ?Sized>(
        context: &C,
    ) -> MaResult<(*mut sys::ma_device_info, u32, *mut sys::ma_device_info, u32)> {
        let mut playback_device_info: *mut sys::ma_device_info = core::ptr::null_mut();
        let mut playback_device_count: u32 = 0;
        let mut capture_device_info: *mut sys::ma_device_info = core::ptr::null_mut();
        let mut capture_device_count: u32 = 0;

        let res = unsafe {
            sys::ma_context_get_devices(
                private_context::context_ptr(context),
                &mut playback_device_info,
                &mut playback_device_count,
                &mut capture_device_info,
                &mut capture_device_count,
            )
        };
        MaudioError::check(res)?;

        Ok((
            playback_device_info,
            playback_device_count,
            capture_device_info,
            capture_device_count,
        ))
    }

    #[inline]
    pub fn ma_context_get_device_info<C: AsContextPtr + ?Sized>(
        context: &C,
        device_type: DeviceType,
        device_id: &DeviceId,
    ) -> MaResult<DeviceInfo> {
        let mut device_info: MaybeUninit<sys::ma_device_info> = MaybeUninit::uninit();
        let res = unsafe {
            sys::ma_context_get_device_info(
                private_context::context_ptr(context),
                device_type.into(),
                device_id.as_raw_ptr(),
                device_info.as_mut_ptr(),
            )
        };
        MaudioError::check(res)?;
        Ok(DeviceInfo::new(unsafe { device_info.assume_init() }))
    }

    #[inline]
    pub fn ma_context_is_loopback_supported<C: AsContextPtr + ?Sized>(context: &C) -> bool {
        let res =
            unsafe { sys::ma_context_is_loopback_supported(private_context::context_ptr(context)) };
        res == 1
    }
}

impl Drop for ContextInner {
    fn drop(&mut self) {
        let _ = context_ffi::ma_context_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

pub struct ContextBuilder<'a> {
    inner: sys::ma_context_config,
    backends: Option<&'a [Backend]>,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
}

impl AsRawRef for ContextBuilder<'_> {
    type Raw = sys::ma_context_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

/// Builder for creating a [`Context`].
///
/// A context controls backend selection and backend-wide behavior such as thread settings
/// used internally by miniaudio.
///
/// In most applications, a single shared context is enough.
impl<'a> ContextBuilder<'a> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let inner = unsafe { sys::ma_context_config_init() };
        Self {
            inner,
            backends: None,
            alloc_cb: None,
        }
    }

    /// Sets the thread priority used by internal context threads, if applicable.
    ///
    /// The exact effect is backend and platform dependent.
    pub fn thread_priority(&mut self, priority: ThreadPriority) -> &mut Self {
        self.inner.threadPriority = priority.into();
        self
    }

    /// Sets the preferred backend order for context initialization.
    ///
    /// Miniaudio will try the provided backends in order until one succeeds.
    /// If not set, miniaudio uses its default backend selection logic.
    pub fn preferred_backends(&mut self, backends: &'a [Backend]) -> &mut Self {
        self.backends = Some(backends);
        self
    }

    /// Sets the stack size, in bytes, for internal threads created by the context.
    ///
    /// This is an advanced option and usually does not need to be changed.
    fn stack_size(&mut self, bytes: usize) -> &mut Self {
        self.inner.threadStackSize = bytes;
        self
    }

    pub fn build(&self) -> MaResult<Context> {
        let ctx = Context::new_with_config(self)?;
        Ok(ctx)
    }
}

struct EnumerateState<'a, F>
where
    F: FnMut(DeviceType, DeviceBasicInfo<'_>) -> EnumerateControl,
{
    f: &'a mut F,
    err: Option<MaudioError>,
}

/// Controls whether device enumeration should continue.
pub enum EnumerateControl {
    /// Continue enumerating remaining devices.
    Continue,
    /// Stop enumeration early.
    Stop,
}

unsafe extern "C" fn enumerate_devices_callback<F>(
    _context: *mut sys::ma_context,
    device_type: sys::ma_device_type,
    device_info: *const sys::ma_device_info,
    user_data: *mut core::ffi::c_void,
) -> sys::ma_bool32
where
    F: FnMut(DeviceType, DeviceBasicInfo<'_>) -> EnumerateControl,
{
    if user_data.is_null() {
        return sys::MA_FALSE;
    }

    let state = &mut *(user_data as *mut EnumerateState<'_, F>);

    if state.err.is_some() {
        return sys::MA_FALSE;
    }

    if device_info.is_null() {
        state.err = Some(MaudioError::from_ma_result(sys::ma_result_MA_ERROR));
        return sys::MA_FALSE;
    }

    let info = &*device_info;
    let name = core::ffi::CStr::from_ptr(info.name.as_ptr());

    let basic = DeviceBasicInfo::new(&info.id, name, info.isDefault);

    let Ok(device_type): Result<DeviceType, _> = device_type.try_into() else {
        state.err = Some(MaudioError::from_ma_result(sys::ma_result_MA_ERROR));
        return sys::MA_FALSE;
    };

    let decision = catch_unwind(AssertUnwindSafe(|| (state.f)(device_type, basic)));

    match decision {
        Ok(EnumerateControl::Continue) => sys::MA_TRUE,
        Ok(EnumerateControl::Stop) => sys::MA_FALSE,
        Err(_) => {
            state.err = Some(MaudioError::from_ma_result(sys::ma_result_MA_ERROR));
            sys::MA_FALSE
        }
    }
}

pub enum ThreadPriority {
    Default,
    Idle,
    Lowest,
    Low,
    Normal,
    High,
    Highest,
    Realtime,
}

impl From<ThreadPriority> for sys::ma_thread_priority {
    fn from(value: ThreadPriority) -> Self {
        match value {
            ThreadPriority::Default => sys::ma_thread_priority_ma_thread_priority_default,
            ThreadPriority::Idle => sys::ma_thread_priority_ma_thread_priority_idle,
            ThreadPriority::Lowest => sys::ma_thread_priority_ma_thread_priority_lowest,
            ThreadPriority::Low => sys::ma_thread_priority_ma_thread_priority_low,
            ThreadPriority::Normal => sys::ma_thread_priority_ma_thread_priority_normal,
            ThreadPriority::High => sys::ma_thread_priority_ma_thread_priority_high,
            ThreadPriority::Highest => sys::ma_thread_priority_ma_thread_priority_highest,
            ThreadPriority::Realtime => sys::ma_thread_priority_ma_thread_priority_realtime,
        }
    }
}

impl TryFrom<sys::ma_thread_priority> for ThreadPriority {
    type Error = MaudioError;

    fn try_from(value: sys::ma_thread_priority) -> Result<Self, Self::Error> {
        match value {
            sys::ma_thread_priority_ma_thread_priority_idle => Ok(ThreadPriority::Idle),
            sys::ma_thread_priority_ma_thread_priority_lowest => Ok(ThreadPriority::Lowest),
            sys::ma_thread_priority_ma_thread_priority_low => Ok(ThreadPriority::Low),
            sys::ma_thread_priority_ma_thread_priority_normal => Ok(ThreadPriority::Normal),
            sys::ma_thread_priority_ma_thread_priority_high => Ok(ThreadPriority::High),
            sys::ma_thread_priority_ma_thread_priority_highest => Ok(ThreadPriority::Highest),
            sys::ma_thread_priority_ma_thread_priority_realtime => Ok(ThreadPriority::Realtime),
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                ThreadPriority,
            >(other as i64))),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::context::{ContextBuilder, ContextOps, EnumerateControl};

    #[test]
    fn test_context_basic_init() {
        let ctx = ContextBuilder::new().build().unwrap();
        drop(ctx);
    }

    #[test]
    fn test_context_get_device_info_owned() {
        let ctx = ContextBuilder::new().build().unwrap();
        let devices = ctx.get_devices().unwrap();
        println!("{}", devices.playback.len());
        println!("{}", devices.capture.len());
    }

    #[test]
    fn test_context_get_device_info_cb() {
        let ctx = ContextBuilder::new().build().unwrap();
        ctx.with_devices(|play, capture| {
            println!("{}", play.len());
            println!("{}", capture.len());
        })
        .unwrap();
    }

    #[test]
    fn test_context_get_device_info_compare() {
        let ctx = ContextBuilder::new().build().unwrap();
        let devices = ctx.get_devices().unwrap();
        let mut cb_play: usize = 0;
        let mut cb_capt: usize = 0;
        ctx.with_devices(|play, capture| {
            cb_play = play.len();
            cb_capt = capture.len();
        })
        .unwrap();
        assert_eq!(devices.playback.len(), cb_play);
        assert_eq!(devices.capture.len(), cb_capt);
    }

    #[test]
    fn test_context_get_device_enumerate() {
        let ctx = ContextBuilder::new().build().unwrap();
        ctx.enumerate_devices(|device_type, info| {
            println!("{:?}", device_type);
            println!("{:?}", info.name());
            EnumerateControl::Continue
        })
        .unwrap();
    }

    #[test]
    fn text_context_send_to_thread() {
        let ctx = ContextBuilder::new().build().unwrap();

        let ctx_clone = ctx.clone();
        let join = std::thread::spawn(move || {
            let _devices = ctx_clone.get_devices().unwrap();
        });

        let _devices = ctx.get_devices().unwrap();
        let _ = join.join();
    }
}
