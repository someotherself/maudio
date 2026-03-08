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
        Device,
    },
    engine::AllocationCallbacks,
    AsRawRef, Binding, MaResult, MaudioError,
};

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

pub struct ContextRef<'a> {
    inner: *mut sys::ma_context,
    _keep_alive: PhantomData<&'a Device>,
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

pub trait ContextOps: AsContextPtr {
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

    fn device_info(&self, device_type: DeviceType, device_id: &DeviceId) -> MaResult<DeviceInfo> {
        context_ffi::ma_context_get_device_info(self, device_type, device_id)
    }

    fn is_loopback_supported(&self) -> bool {
        context_ffi::ma_context_is_loopback_supported(self)
    }

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

impl<'a> ContextBuilder<'a> {
    pub fn new() -> Self {
        let inner = unsafe { sys::ma_context_config_init() };
        Self {
            inner,
            backends: None,
            alloc_cb: None,
        }
    }

    pub fn thread_priority(&mut self, priority: sys::ma_thread_priority) -> &mut Self {
        self.inner.threadPriority = priority;
        self
    }

    pub fn preferred_backends(&mut self, backends: &'a [Backend]) -> &mut Self {
        self.backends = Some(backends);
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

pub enum EnumerateControl {
    Continue,
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
