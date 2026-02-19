use std::{marker::PhantomData, mem::MaybeUninit, path::Path};

use maudio_sys::ffi as sys;

use crate::{engine::resource::rm_builder::ResourceManagerBuilder, AsRawRef, Binding, MaResult};

pub mod rm_builder;
pub mod rm_flags;

pub struct ResourceManager {
    inner: *mut sys::ma_resource_manager,
}

// ma_resource_manager is intended to be used from multiple threads
// it uses a multi-producer/multi-consumer job queue and background job threads
// NOTE: Everything else added to the Rust struct needs to be Send and Sync!!!
unsafe impl Send for ResourceManager {}
unsafe impl Sync for ResourceManager {}

impl Binding for ResourceManager {
    type Raw = *mut sys::ma_resource_manager;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

pub struct ResourceManagerRef<'a> {
    inner: *mut sys::ma_resource_manager,
    _marker: PhantomData<&'a ()>,
}

impl Binding for ResourceManagerRef<'_> {
    type Raw = *mut sys::ma_resource_manager;

    /// !!! unimplemented !!!
    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            _marker: PhantomData,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

mod private_rm {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait RmPtrProvider<T: ?Sized> {
        fn as_rm_ptr(t: &T) -> *mut sys::ma_resource_manager;
    }

    pub struct RmProvider;
    pub struct RmRefProvider;

    impl RmPtrProvider<ResourceManager> for RmProvider {
        fn as_rm_ptr(t: &ResourceManager) -> *mut sys::ma_resource_manager {
            t.to_raw()
        }
    }

    impl RmPtrProvider<ResourceManagerRef<'_>> for RmRefProvider {
        fn as_rm_ptr(t: &ResourceManagerRef<'_>) -> *mut sys::ma_resource_manager {
            t.to_raw()
        }
    }

    pub fn rm_ptr<T: AsRmPtr + ?Sized>(t: &T) -> *mut sys::ma_resource_manager {
        <T as AsRmPtr>::__PtrProvider::as_rm_ptr(t)
    }
}

#[doc(hidden)]
pub trait AsRmPtr {
    type __PtrProvider: private_rm::RmPtrProvider<Self>;
}

impl AsRmPtr for ResourceManager {
    type __PtrProvider = private_rm::RmProvider;
}

impl AsRmPtr for ResourceManagerRef<'_> {
    type __PtrProvider = private_rm::RmRefProvider;
}

impl RmOps for ResourceManager {}
impl RmOps for ResourceManagerRef<'_> {}

pub struct ResourceRegistration<'a> {
    rm: &'a ResourceManager,
}

pub trait RmOps: AsRmPtr {
    fn register(&self) {}

    fn register_file(&self, path: &Path, flags: u32) -> MaResult<()> {
        #[cfg(unix)]
        {
            use crate::engine::cstring_from_path;

            let path = cstring_from_path(path)?;
            resource_ffi::ma_resource_manager_register_file(self, path, flags)?;
            Ok(())
        }

        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated;

            let path = wide_null_terminated(path);

            resource_ffi::ma_resource_manager_register_file_w(self, &path, flags)?;
            Ok(())
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    // fn register_decoder_u8<'a>(
    //     &self,
    //     name: String,
    //     data: &'a [u8],
    //     channels: u32,
    //     sample_rate: SampleRate,
    // ) -> MaResult<()> {
    //     resource_ffi::ma_resource_manager_register_decoded_data_internal::<u8, Self>(
    //         self,
    //         &name,
    //         data,
    //         Format::U8,
    //         channels,
    //         sample_rate,
    //     )
    // }

    // fn register_decoder_i16<'a>(
    //     &self,
    //     _name: String,
    //     _data: &'a [i16],
    //     _channels: u32,
    //     _sample_rate: SampleRate,
    // ) {
    // }

    // fn register_decoder_i32<'a>(
    //     &mut self,
    //     _name: String,
    //     _data: &'a [i32],
    //     _channels: u32,
    //     _sample_rate: SampleRate,
    // ) {
    // }

    // TODO: register_decoded_data does not copy, so the converted data needs to be stored somewhere else
    // fn register_decoder_s24() {}

    // fn register_decoder_s24_packed<'a>(
    //     &self,
    //     _name: String,
    //     _data: &'a [u8],
    //     _channels: u32,
    //     _sample_rate: SampleRate,
    // ) {
    // }

    // fn register_decoder_f32<'a>(
    //     &self,
    //     _name: String,
    //     _data: &'a [f32],
    //     _channels: u32,
    //     _sample_rate: SampleRate,
    // ) -> MaResult<()> {
    //     todo!()
    // }

    // fn register_decoded_data<'a, F: PcmFormat>(
    //     &mut self,
    //     name: String,
    //     data: &'a [F::PcmUnit],
    //     channels: u32,
    //     sample_rate: SampleRate
    // ) -> MaResult<()> {
    // }
}

impl ResourceManager {
    fn new_with_config(config: &ResourceManagerBuilder) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resource_manager>> = Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_init(config.as_raw_ptr(), mem.as_mut_ptr())?;

        let inner: *mut sys::ma_resource_manager =
            Box::into_raw(mem) as *mut sys::ma_resource_manager;

        Ok(Self { inner })
    }
}

pub(crate) mod resource_ffi {
    use crate::engine::resource::{private_rm, AsRmPtr};
    use maudio_sys::ffi as sys;

    #[cfg(unix)]
    use crate::engine::resource::RmOps;
    use crate::{
        audio::{
            formats::{Format, SampleBuffer},
            sample_rate::SampleRate,
        },
        engine::resource::ResourceManager,
        pcm_frames::PcmFormat,
        Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_resource_manager_init(
        config: *const sys::ma_resource_manager_config,
        rm: *mut sys::ma_resource_manager,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_init(config, rm) };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_resource_manager_uninit(rm: &mut ResourceManager) {
        unsafe {
            sys::ma_resource_manager_uninit(rm.to_raw());
        }
    }

    // TODO: Implement Log
    #[inline]
    pub fn ma_resource_manager_get_log(rm: &mut ResourceManager) -> Option<*mut sys::ma_log> {
        let ptr = unsafe { sys::ma_resource_manager_get_log(rm.to_raw()) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    // REGISTRATION

    #[inline]
    #[cfg(unix)]
    pub fn ma_resource_manager_register_file<R: RmOps + ?Sized>(
        rm: &R,
        path: std::ffi::CString,
        flags: u32,
    ) -> MaResult<()> {
        let res = unsafe {
            use crate::engine::resource::private_rm;
            sys::ma_resource_manager_register_file(private_rm::rm_ptr(rm), path.as_ptr(), flags)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_resource_manager_register_file_w<R: AsRmPtr + ?Sized>(
        rm: &R,
        path: &[u16],
        flags: u32,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_file_w(private_rm::rm_ptr(rm), path.as_ptr(), flags)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    fn ma_resource_manager_register_decoded_data_internal<F: PcmFormat, R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &str,
        data: &[F::PcmUnit],
        format: Format,
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            let name = std::ffi::CString::new(name)
                .map_err(|_| crate::MaudioError::new_ma_error(crate::ErrorKinds::InvalidCString))?;
            let frame_count = 0;
            ma_resource_manager_register_decoded_data(
                rm,
                name.as_ptr(),
                data.as_ptr() as *const _,
                frame_count,
                format,
                channels,
                sample_rate,
            )
        }
        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated_name;

            let name = wide_null_terminated_name(name);
            let frame_count = 0;
            ma_resource_manager_register_decoded_data_w(
                rm,
                &name,
                data.as_ptr() as *const _,
                frame_count,
                format,
                channels,
                sample_rate,
            )
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    #[inline]
    #[cfg(unix)]
    fn ma_resource_manager_register_decoded_data<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: *const core::ffi::c_char,
        data: *const core::ffi::c_void,
        frame_count: u64,
        format: Format,
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_decoded_data(
                private_rm::rm_ptr(rm),
                name,
                data,
                frame_count,
                format.into(),
                channels,
                sample_rate.into(),
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    fn ma_resource_manager_register_decoded_data_w<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &[u16],
        data: *const core::ffi::c_void,
        frame_count: u64,
        format: Format,
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_decoded_data_w(
                private_rm::rm_ptr(rm),
                name.as_ptr(),
                data,
                frame_count,
                format.into(),
                channels,
                sample_rate.into(),
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(unix)]
    pub fn ma_resource_manager_register_encoded_data(
        rm: &ResourceManager,
        name: *const core::ffi::c_char,
        data: *const core::ffi::c_void,
        size: usize,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_encoded_data(rm.to_raw(), name, data, size)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_resource_manager_register_encoded_data_w(
        rm: &mut ResourceManager,
        name: &[u16],
        data: *const core::ffi::c_void,
        size: usize,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_encoded_data_w(rm.to_raw(), name.as_ptr(), data, size)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(unix)]
    pub fn ma_resource_manager_unregister_file(
        rm: &mut ResourceManager,
        path: std::ffi::CString,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_unregister_file(rm.to_raw(), path.as_ptr()) };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_resource_manager_unregister_file_w(
        rm: &mut ResourceManager,
        path: &[u16],
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_unregister_file_w(rm.to_raw(), path.as_ptr()) };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(unix)]
    pub fn ma_resource_manager_unregister_data(
        rm: &mut ResourceManager,
        name: *const core::ffi::c_char,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_unregister_data(rm.to_raw(), name) };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_resource_manager_unregister_data_w(
        rm: &mut ResourceManager,
        name: &[u16],
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_unregister_data_w(rm.to_raw(), name.as_ptr()) };
        MaudioError::check(res)?;
        Ok(())
    }

    // DATA BUFFERS

    #[inline]
    pub fn ma_resource_manager_data_buffer_init_ex(
        rm: &mut ResourceManager,
        config: *const sys::ma_resource_manager_data_source_config,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init_ex(rm.to_raw(), config, data_buffer)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(unix)]
    pub fn ma_resource_manager_data_buffer_init(
        rm: &mut ResourceManager,
        path: std::ffi::CString,
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init(
                rm.to_raw(),
                path.as_ptr(),
                flags,
                notif,
                data_buffer,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_resource_manager_data_buffer_init_w(
        rm: &mut ResourceManager,
        path: &[u16],
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init_w(
                rm.to_raw(),
                path.as_ptr(),
                flags,
                notif,
                data_buffer,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_init_copy(
        rm: &mut ResourceManager,
        existing: *const sys::ma_resource_manager_data_buffer,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init_copy(rm.to_raw(), existing, data_buffer)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_uninit(
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_buffer_uninit(data_buffer) };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_read_pcm_frames_into() -> MaResult<usize> {
        todo!()
    }

    // TODO
    #[inline]
    pub fn ma_resource_manager_data_buffer_read_pcm_frames<F: PcmFormat>(
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<F>> {
        let mut frames_read = 0;
        let frames_out: *mut core::ffi::c_void = core::ptr::null_mut();
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_read_pcm_frames(
                data_buffer,
                frames_out,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        todo!()
    }

    // DATA SOURCES

    #[inline]
    pub fn ma_resource_manager_data_source_init_ex(
        rm: &mut ResourceManager,
        config: *const sys::ma_resource_manager_data_source_config,
        data_source: *mut sys::ma_resource_manager_data_source,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_source_init_ex(rm.to_raw(), config, data_source)
        };
        MaudioError::check(res)?;
        Ok(())
    }
}
