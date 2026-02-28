use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    path::{Path, PathBuf},
    sync::Arc,
};

use maudio_sys::ffi as sys;

use crate::{
    audio::{
        formats::{Format, SampleBuffer},
        sample_rate::SampleRate,
    },
    engine::resource::{
        rm_buffer::{ResourceManagerBuffer, ResourceManagerBufferBuilder},
        rm_builder::ResourceManagerBuilder,
        rm_source::{ResourceManagerSource, ResourceManagerSourceBuilder},
        rm_source_flags::RmSourceFlags,
        rm_stream::{ResourceManagerStream, ResourceManagerStreamBuilder},
    },
    pcm_frames::{PcmFormat, PcmFormatInternal, S24Packed, S24},
    test_assets::wav_i16_le,
    AsRawRef, Binding, MaResult,
};

pub mod rm_buffer;
pub mod rm_builder;
pub mod rm_flags;
pub mod rm_source;
pub mod rm_source_flags;
pub mod rm_stream;

#[derive(Clone)]
pub struct ResourceManager<F: PcmFormat> {
    inner: Arc<InnerResourceManager<F>>,
}

pub(crate) struct InnerResourceManager<F: PcmFormat> {
    inner: *mut sys::ma_resource_manager,
    channels: Option<u32>,
    _format: PhantomData<F>,
}

// ma_resource_manager is intended to be used from multiple threads
// it uses a multi-producer/multi-consumer job queue and background job threads
// NOTE: Everything else added to the Rust struct needs to be Send and Sync!!!
unsafe impl<F: PcmFormat> Send for ResourceManager<F> {}
unsafe impl<F: PcmFormat> Sync for ResourceManager<F> {}

impl<F: PcmFormat> Binding for ResourceManager<F> {
    type Raw = *mut sys::ma_resource_manager;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner.inner
    }
}

pub struct ResourceManagerRef<'a, F: PcmFormat> {
    inner: *mut sys::ma_resource_manager,
    _format: PhantomData<F>,
    _marker: PhantomData<&'a ()>,
}

impl<F: PcmFormat> Binding for ResourceManagerRef<'_, F> {
    type Raw = *mut sys::ma_resource_manager;

    /// !!! unimplemented !!!
    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            _format: PhantomData,
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

    impl<F: PcmFormat> RmPtrProvider<ResourceManager<F>> for RmProvider {
        fn as_rm_ptr(t: &ResourceManager<F>) -> *mut sys::ma_resource_manager {
            t.to_raw()
        }
    }

    impl<F: PcmFormat> RmPtrProvider<ResourceManagerRef<'_, F>> for RmRefProvider {
        fn as_rm_ptr(t: &ResourceManagerRef<'_, F>) -> *mut sys::ma_resource_manager {
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
    type Format: PcmFormat;
}

impl<F: PcmFormat> AsRmPtr for ResourceManager<F> {
    type __PtrProvider = private_rm::RmProvider;
    type Format = F;
}

impl<F: PcmFormat> AsRmPtr for ResourceManagerRef<'_, F> {
    type __PtrProvider = private_rm::RmRefProvider;
    type Format = F;
}

pub struct ResourceGuard<'a, R: AsRmPtr + ?Sized> {
    rm: &'a R,
    data_name: RegisteredDataType,
    data_store: Option<Arc<[u8]>>,
    _data_marker: PhantomData<&'a [u8]>,
}

// Builders for registered resources
impl<'a, R: AsRmPtr + ?Sized> ResourceGuard<'a, R> {
    pub fn build_buffer(&'a self, flags: RmSourceFlags) -> MaResult<ResourceManagerBuffer<'a, R>> {
        let mut builder = ResourceManagerBufferBuilder::new(self.rm);
        builder.flags(flags); // TODO: Does it make sense to take flags here too?
        match &self.data_name {
            RegisteredDataType::RegisteredPath { path } => builder.file_path(path),
            RegisteredDataType::RegisteredData { name } => builder.file_path(Path::new(name)),
        };
        builder.build()
    }

    /// Will fail if the data is anything other than a file
    pub fn build_stream(&'a self, flags: RmSourceFlags) -> MaResult<ResourceManagerStream<'a, R>> {
        let mut builder = ResourceManagerStreamBuilder::new(self.rm);
        builder.flags(flags);
        match &self.data_name {
            RegisteredDataType::RegisteredPath { path } => builder.file_path(path),
            RegisteredDataType::RegisteredData { name } => builder.file_path(Path::new(name)),
        };
        builder.build()
    }

    pub fn build_source(&'a self, flags: RmSourceFlags) -> MaResult<ResourceManagerSource<'a, R>> {
        let mut builder = ResourceManagerSourceBuilder::new(self.rm);
        builder.flags(flags);
        match &self.data_name {
            RegisteredDataType::RegisteredPath { path } => builder.file_path(path),
            RegisteredDataType::RegisteredData { name } => builder.file_path(Path::new(name)),
        };
        builder.build()
    }
}

// Private methods
impl<'a, R: AsRmPtr + ?Sized> ResourceGuard<'a, R> {
    pub(crate) fn from_path(rm: &'a R, path: &Path) -> Self {
        Self {
            rm,
            data_name: RegisteredDataType::RegisteredPath {
                path: path.to_path_buf(),
            },
            data_store: None,
            _data_marker: PhantomData,
        }
    }

    pub(crate) fn from_data(rm: &'a R, name: &str, data: Option<Arc<[u8]>>) -> Self {
        Self {
            rm,
            data_name: RegisteredDataType::RegisteredData {
                name: name.to_string(),
            },
            data_store: data,
            _data_marker: PhantomData,
        }
    }
}

pub enum RegisteredDataType {
    RegisteredPath { path: PathBuf },
    RegisteredData { name: String },
}

impl<R: AsRmPtr + ?Sized> Drop for ResourceGuard<'_, R> {
    fn drop(&mut self) {
        match &self.data_name {
            RegisteredDataType::RegisteredData { name } => {
                let _ = resource_ffi::ma_resource_manager_unregister_data_internal(self.rm, name);
            }
            RegisteredDataType::RegisteredPath { path } => {
                let _ = resource_ffi::ma_resource_manager_unregister_file_internal(self.rm, path);
            }
        }
    }
}

impl<F: PcmFormat> RmOps for ResourceManager<F> {}
impl<F: PcmFormat> RmOps for ResourceManagerRef<'_, F> {}

pub trait RmOps: AsRmPtr {
    fn register_file<'a>(
        &'a self,
        path: &Path,
        flags: RmSourceFlags,
    ) -> MaResult<ResourceGuard<'a, Self>> {
        #[cfg(unix)]
        {
            use crate::engine::cstring_from_path;

            let c_path = cstring_from_path(path)?;
            resource_ffi::ma_resource_manager_register_file(self, c_path, flags)?;
            Ok(ResourceGuard::from_path(self, path))
        }

        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated;

            let c_path = wide_null_terminated(path);

            resource_ffi::ma_resource_manager_register_file_w(self, &c_path, flags)?;
            Ok(ResourceGuard::from_path(self, path))
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    fn register_decoded_u8<'a>(
        &'a self,
        name: &str,
        data: &'a [u8],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, Self>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<u8, Self>(
            self,
            name,
            data,
            Format::U8,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self, name, None))
    }

    fn register_decoded_i16<'a>(
        &'a self,
        name: &str,
        data: &'a [i16],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, Self>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<i16, Self>(
            self,
            name,
            data,
            Format::S16,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self, name, None))
    }

    fn register_decoded_i32<'a>(
        &'a self,
        name: &str,
        data: &'a [i32],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, Self>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<i32, Self>(
            self,
            name,
            data,
            Format::S32,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self, name, None))
    }

    fn register_decoded_s24_packed<'a>(
        &'a self,
        name: &str,
        data: &'a [u8],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, Self>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<S24Packed, Self>(
            self,
            name,
            data,
            Format::S24,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self, name, None))
    }

    fn register_decoded_s24<'a>(
        &'a self,
        name: &str,
        data: &[i32],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, Self>> {
        if channels == 0 {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        let data_len = data.len();
        if data_len % channels as usize != 0 {
            return Err(crate::MaudioError::new_ma_error(
                crate::ErrorKinds::InvalidDecodedDataLength,
            ));
        }

        let frames = data_len / channels as usize;
        let mut dst = SampleBuffer::<S24>::new_zeroed(frames, channels)?;
        <S24 as PcmFormatInternal>::write_to_storage_internal(
            &mut dst,
            data,
            frames,
            channels as usize,
        )?;
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<S24Packed, Self>(
            self,
            name,
            &dst,
            Format::S24,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self, name, Some(dst.into())))
    }

    fn register_decoded_f32<'a>(
        &'a self,
        name: &str,
        data: &'a [f32],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, Self>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<f32, Self>(
            self,
            name,
            data,
            Format::F32,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self, name, None))
    }

    fn encoded<'a>(&'a self, name: &str, data: &'a [u8]) -> MaResult<ResourceGuard<'a, Self>> {
        resource_ffi::ma_resource_manager_register_encoded_data_internal(self, name, data)?;
        Ok(ResourceGuard::from_data(self, name, None))
    }
}

impl<F: PcmFormat> ResourceManager<F> {
    fn new_with_config(config: &ResourceManagerBuilder) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resource_manager>> = Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_init(config.as_raw_ptr(), mem.as_mut_ptr())?;

        let inner: *mut sys::ma_resource_manager =
            Box::into_raw(mem) as *mut sys::ma_resource_manager;

        Ok(Self {
            inner: Arc::new(InnerResourceManager {
                inner,
                channels: None,
                _format: PhantomData,
            }),
        })
    }
}

pub(crate) mod resource_ffi {
    use std::path::Path;

    use crate::{
        audio::{
            channels::Channel,
            formats::{Format, SampleBuffer},
            sample_rate::SampleRate,
        },
        data_source::{DataFormat, DataSourceOps},
        engine::resource::{
            private_rm,
            rm_buffer::{ResourceManagerBuffer, ResourceManagerBufferBuilder},
            rm_source::{ResourceManagerSource, ResourceManagerSourceBuilder},
            rm_source_flags::RmSourceFlags,
            rm_stream::{ResourceManagerStream, ResourceManagerStreamBuilder},
            AsRmPtr, InnerResourceManager, ResourceManager,
        },
        pcm_frames::PcmFormat,
        Binding, MaResult, MaudioError,
    };
    use crate::{AsRawRef, ErrorKinds};

    use maudio_sys::ffi as sys;

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
    pub fn ma_resource_manager_uninit<F: PcmFormat>(rm: &mut InnerResourceManager<F>) {
        unsafe {
            sys::ma_resource_manager_uninit(rm.inner);
        }
    }

    // TODO: Implement Log
    #[inline]
    pub fn ma_resource_manager_get_log<F: PcmFormat>(
        rm: &mut ResourceManager<F>,
    ) -> Option<*mut sys::ma_log> {
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
    pub fn ma_resource_manager_register_file<R: AsRmPtr + ?Sized>(
        rm: &R,
        path: std::ffi::CString,
        flags: RmSourceFlags,
    ) -> MaResult<()> {
        let res = unsafe {
            use crate::engine::resource::private_rm;
            sys::ma_resource_manager_register_file(
                private_rm::rm_ptr(rm),
                path.as_ptr(),
                flags.bits(),
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_resource_manager_register_file_w<R: AsRmPtr + ?Sized>(
        rm: &R,
        path: &[u16],
        flags: RmSourceFlags,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_file_w(
                private_rm::rm_ptr(rm),
                path.as_ptr(),
                flags.bits(),
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    pub fn ma_resource_manager_register_decoded_data_internal<F: PcmFormat, R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &str,
        data: &[F::StorageUnit],
        format: Format,
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<()> {
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        let data_len = data.len();

        let units_per_frame = (channels as usize)
            .checked_mul(F::VEC_PCM_UNITS_PER_FRAME)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                op: "Units per frames",
                lhs: channels as u64,
                rhs: F::VEC_PCM_UNITS_PER_FRAME as u64,
            }))?;

        if data_len % units_per_frame != 0 {
            return Err(crate::MaudioError::new_ma_error(
                crate::ErrorKinds::InvalidDecodedDataLength,
            ));
        }

        let frame_count = (data_len / units_per_frame) as u64;

        #[cfg(unix)]
        {
            let name = std::ffi::CString::new(name)
                .map_err(|_| crate::MaudioError::new_ma_error(crate::ErrorKinds::InvalidCString))?;
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

    pub fn ma_resource_manager_register_encoded_data_internal<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &str,
        data: &[u8],
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            let name = std::ffi::CString::new(name)
                .map_err(|_| crate::MaudioError::new_ma_error(crate::ErrorKinds::InvalidCString))?;
            ma_resource_manager_register_encoded_data(
                rm,
                name.as_ptr(),
                data.as_ptr() as *const _,
                data.len(),
            )
        }
        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated_name;

            let name = wide_null_terminated_name(name);
            ma_resource_manager_register_encoded_data_w(
                rm,
                &name,
                data.as_ptr() as *const _,
                data.len(),
            )
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    #[inline]
    #[cfg(unix)]
    fn ma_resource_manager_register_encoded_data<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: *const core::ffi::c_char,
        data: *const core::ffi::c_void,
        size: usize,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_encoded_data(private_rm::rm_ptr(rm), name, data, size)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    fn ma_resource_manager_register_encoded_data_w<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &[u16],
        data: *const core::ffi::c_void,
        size: usize,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_encoded_data_w(
                private_rm::rm_ptr(rm),
                name.as_ptr(),
                data,
                size,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    pub fn ma_resource_manager_unregister_file_internal<R: AsRmPtr + ?Sized>(
        rm: &R,
        path: &Path,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            use crate::engine::cstring_from_path;

            let c_path = cstring_from_path(path)?;
            ma_resource_manager_unregister_file(rm, c_path)
        }
        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated;

            let c_path = wide_null_terminated(path);
            ma_resource_manager_unregister_file_w(rm, &c_path)
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    #[inline]
    #[cfg(unix)]
    fn ma_resource_manager_unregister_file<R: AsRmPtr + ?Sized>(
        rm: &R,
        path: std::ffi::CString,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_unregister_file(private_rm::rm_ptr(rm), path.as_ptr())
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    fn ma_resource_manager_unregister_file_w<R: AsRmPtr + ?Sized>(
        rm: &R,
        path: &[u16],
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_unregister_file_w(private_rm::rm_ptr(rm), path.as_ptr())
        };
        MaudioError::check(res)?;
        Ok(())
    }

    pub fn ma_resource_manager_unregister_data_internal<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &str,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            let name = std::ffi::CString::new(name)
                .map_err(|_| crate::MaudioError::new_ma_error(crate::ErrorKinds::InvalidCString))?;

            ma_resource_manager_unregister_data(rm, name.as_ptr())
        }
        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated_name;

            let name = wide_null_terminated_name(name);
            ma_resource_manager_unregister_data_w(rm, &name)
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    #[inline]
    #[cfg(unix)]
    fn ma_resource_manager_unregister_data<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: *const core::ffi::c_char,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_unregister_data(private_rm::rm_ptr(rm), name) };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    fn ma_resource_manager_unregister_data_w<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &[u16],
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_unregister_data_w(private_rm::rm_ptr(rm), name.as_ptr())
        };
        MaudioError::check(res)?;
        Ok(())
    }

    // DATA BUFFERS

    #[inline]
    pub fn ma_resource_manager_data_buffer_init_ex<R: AsRmPtr + ?Sized>(
        rm: &R,
        config: &ResourceManagerBufferBuilder<'_, R>,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init_ex(
                private_rm::rm_ptr(rm),
                config.as_raw_ptr(),
                data_buffer,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    #[cfg(unix)]
    pub fn ma_resource_manager_data_buffer_init<R: AsRmPtr + ?Sized>(
        rm: &R,
        path: std::ffi::CString,
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init(
                private_rm::rm_ptr(rm),
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
    pub fn ma_resource_manager_data_buffer_init_w<R: AsRmPtr + ?Sized>(
        rm: &R,
        path: &[u16],
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init_w(
                private_rm::rm_ptr(rm),
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
    pub fn ma_resource_manager_data_buffer_init_copy<R: AsRmPtr + ?Sized>(
        rm: &R,
        existing: &ResourceManagerBuffer<'_, R>,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init_copy(
                private_rm::rm_ptr(rm),
                existing.to_raw(),
                data_buffer,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_uninit<R: AsRmPtr + ?Sized>(
        data_buffer: &ResourceManagerBuffer<'_, R>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_buffer_uninit(data_buffer.to_raw()) };
        MaudioError::check(res)?;
        Ok(())
    }

    // Not used. Already available on DataSourceOps
    pub fn ma_resource_manager_data_buffer_read_pcm_frames<'a, R: AsRmPtr>(
        data_buffer: &mut ResourceManagerBuffer<'a, R>,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<R::Format>> {
        let channels = data_buffer.data_format()?.channels;
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        let mut buffer = SampleBuffer::<R::Format>::new_zeroed(frame_count as usize, channels)?;

        let frames_read = ma_resource_manager_data_buffer_read_pcm_frames_internal::<R>(
            data_buffer,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<R::Format>::from_storage(buffer, frames_read as usize, 2)
    }

    #[inline]
    fn ma_resource_manager_data_buffer_read_pcm_frames_internal<'a, R: AsRmPtr>(
        data_buffer: &mut ResourceManagerBuffer<'a, R>,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_read_pcm_frames(
                data_buffer.to_raw(),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_read)
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_seek_to_pcm_frame<R: AsRmPtr>(
        data_buffer: &ResourceManagerBuffer<'_, R>,
        frame_index: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_seek_to_pcm_frame(
                data_buffer.to_raw(),
                frame_index,
            )
        };
        MaudioError::check(res)?;
        todo!()
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_get_data_format<R: AsRmPtr>(
        data_buffer: &ResourceManagerBuffer<'_, R>,
        format: *mut sys::ma_format,
        channels: *mut u32,
        sample_rate: *mut u32,
        channel_map: *mut sys::ma_channel,
        channel_map_cap: usize,
    ) -> MaResult<Format> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_get_data_format(
                data_buffer.to_raw(),
                format,
                channels,
                sample_rate,
                channel_map,
                channel_map_cap,
            )
        };
        MaudioError::check(res)?;
        todo!()
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_get_cursor_in_pcm_frames<R: AsRmPtr>(
        data_buffer: &ResourceManagerBuffer<'_, R>,
    ) -> MaResult<u64> {
        let mut cursor: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_get_cursor_in_pcm_frames(
                data_buffer.to_raw(),
                &mut cursor,
            )
        };
        MaudioError::check(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_get_length_in_pcm_frames<R: AsRmPtr>(
        data_buffer: &ResourceManagerBuffer<'_, R>,
    ) -> MaResult<u64> {
        let mut length: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_get_length_in_pcm_frames(
                data_buffer.to_raw(),
                &mut length,
            )
        };
        MaudioError::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_result<R: AsRmPtr>(
        data_buffer: &ResourceManagerBuffer<'_, R>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_buffer_result(data_buffer.to_raw()) };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_buffer_set_looping<R: AsRmPtr>(
        data_buffer: &ResourceManagerBuffer<'_, R>,
        is_looping: bool,
    ) -> MaResult<()> {
        let is_looping = is_looping as u32;
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_set_looping(data_buffer.to_raw(), is_looping)
        };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_buffer_is_looping<R: AsRmPtr>(
        data_buffer: &ResourceManagerBuffer<'_, R>,
    ) -> bool {
        let res = unsafe { sys::ma_resource_manager_data_buffer_is_looping(data_buffer.to_raw()) };
        res == 1
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_get_available_frames<R: AsRmPtr>(
        data_buffer: &ResourceManagerBuffer<'_, R>,
    ) -> MaResult<u64> {
        let mut frames = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_get_available_frames(
                data_buffer.to_raw(),
                &mut frames,
            )
        };
        MaudioError::check(res)?;
        Ok(frames)
    }

    // DATA SOURCES

    #[inline]
    pub fn ma_resource_manager_data_source_init_ex<R: AsRmPtr + ?Sized>(
        rm: &R,
        config: &ResourceManagerSourceBuilder<'_, R>,
        data_source: *mut sys::ma_resource_manager_data_source,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_source_init_ex(
                private_rm::rm_ptr(rm),
                config.as_raw_ptr(),
                data_source,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_resource_manager_data_source_init<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: *const core::ffi::c_char,
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_source: *mut sys::ma_resource_manager_data_source,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_source_init(
                private_rm::rm_ptr(rm),
                name,
                flags,
                notif,
                data_source,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_resource_manager_data_source_init_copy<R: AsRmPtr + ?Sized>(
        rm: &R,
        existing: &ResourceManagerSource<'_, R>,
        data_source: *mut sys::ma_resource_manager_data_source,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_source_init_copy(
                private_rm::rm_ptr(rm),
                existing.to_raw(),
                data_source,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_resource_manager_data_source_uninit<R: AsRmPtr + ?Sized>(
        data_source: &ResourceManagerSource<'_, R>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_source_uninit(data_source.to_raw()) };
        MaudioError::check(res)?;
        Ok(())
    }

    // Not used. Already available on DataSourceOps
    pub fn ma_resource_manager_data_source_read_pcm_frames<R: AsRmPtr>(
        data_source: &mut ResourceManagerSource<'_, R>,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<R::Format>> {
        let channels = data_source.data_format()?.channels;
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        let mut buffer = SampleBuffer::<R::Format>::new_zeroed(frame_count as usize, channels)?;

        let frames_read = ma_resource_manager_data_source_read_pcm_frames_internal::<R>(
            data_source,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<R::Format>::from_storage(buffer, frames_read as usize, channels)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    fn ma_resource_manager_data_source_read_pcm_frames_internal<R: AsRmPtr>(
        data_source: &mut ResourceManagerSource<'_, R>,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_source_read_pcm_frames(
                data_source.to_raw(),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_read)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_source_seek_to_pcm_frame<R: AsRmPtr>(
        data_source: &mut ResourceManagerSource<'_, R>,
        frame: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_source_seek_to_pcm_frame(data_source.to_raw(), frame)
        };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_source_get_data_format<R: AsRmPtr>(
        data_source: &ResourceManagerSource<'_, R>,
    ) -> MaResult<DataFormat> {
        let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        let mut channels: u32 = 0;
        let mut sample_rate: u32 = 0;
        let mut channel_map_raw = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];
        let res = unsafe {
            sys::ma_resource_manager_data_source_get_data_format(
                data_source.to_raw(),
                &mut format_raw,
                &mut channels,
                &mut sample_rate,
                channel_map_raw.as_mut_ptr(),
                channel_map_raw.len(),
            )
        };
        MaudioError::check(res)?;
        // Could cast when passing the ptr to miniaudio, but copying should be fine here
        let mut channel_map: Vec<Channel> =
            channel_map_raw.into_iter().map(Channel::from_raw).collect();
        channel_map.truncate(channels as usize);

        Ok(DataFormat {
            format: format_raw.try_into()?,
            channels,
            sample_rate: sample_rate.try_into()?,
            channel_map: Some(channel_map),
        })
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_source_get_cursor_in_pcm_frames<R: AsRmPtr>(
        data_source: &ResourceManagerSource<'_, R>,
    ) -> MaResult<u64> {
        let mut cursor = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_source_get_cursor_in_pcm_frames(
                data_source.to_raw(),
                &mut cursor,
            )
        };
        MaudioError::check(res)?;
        Ok(cursor)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_source_get_length_in_pcm_frames<R: AsRmPtr>(
        data_source: &ResourceManagerSource<'_, R>,
    ) -> MaResult<u64> {
        let mut length: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_source_get_length_in_pcm_frames(
                data_source.to_raw(),
                &mut length,
            )
        };
        MaudioError::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_resource_manager_data_source_result<R: AsRmPtr>(
        data_source: &ResourceManagerSource<'_, R>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_source_result(data_source.to_raw()) };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_source_set_looping<R: AsRmPtr>(
        data_source: &ResourceManagerSource<'_, R>,
        is_looping: bool,
    ) -> MaResult<()> {
        let is_looping = is_looping as u32;
        let res = unsafe {
            sys::ma_resource_manager_data_source_set_looping(data_source.to_raw(), is_looping)
        };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_source_is_looping<R: AsRmPtr>(
        data_source: &ResourceManagerSource<'_, R>,
    ) -> bool {
        let res = unsafe { sys::ma_resource_manager_data_source_is_looping(data_source.to_raw()) };
        res == 1
    }

    #[inline]
    pub fn ma_resource_manager_data_source_get_available_frames<R: AsRmPtr>(
        data_source: &ResourceManagerSource<'_, R>,
    ) -> MaResult<u64> {
        let mut frames: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_source_get_available_frames(
                data_source.to_raw(),
                &mut frames,
            )
        };
        MaudioError::check(res)?;
        Ok(frames)
    }

    // DATA STREAM

    #[inline]
    pub fn ma_resource_manager_data_stream_init_ex<R: AsRmPtr + ?Sized>(
        rm: &R,
        config: &ResourceManagerStreamBuilder<'_, R>,
        data_stream: *mut sys::ma_resource_manager_data_stream,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_stream_init_ex(
                private_rm::rm_ptr(rm),
                config.as_raw_ptr(),
                data_stream,
            )
        };
        MaudioError::check(res)
    }

    #[cfg(unix)]
    #[inline]
    pub fn ma_resource_manager_data_stream_init<R: AsRmPtr>(
        rm: &R,
        path: std::ffi::CString,
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_stream: *mut sys::ma_resource_manager_data_stream,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_stream_init(
                private_rm::rm_ptr(rm),
                path.as_ptr(),
                flags,
                notif,
                data_stream,
            )
        };
        MaudioError::check(res)
    }

    #[cfg(windows)]
    #[inline]
    pub fn ma_resource_manager_data_stream_init_w<R: AsRmPtr>(
        rm: &R,
        path: &[u16],
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_stream: *mut sys::ma_resource_manager_data_stream,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_stream_init_w(
                private_rm::rm_ptr(rm),
                path.as_ptr(),
                flags,
                notif,
                data_stream,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_resource_manager_data_stream_uninit<R: AsRmPtr + ?Sized>(
        data_stream: &mut ResourceManagerStream<'_, R>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_stream_uninit(data_stream.to_raw()) };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    pub fn ma_resource_manager_data_stream_read_pcm_frames<R: AsRmPtr>(
        data_stream: &mut ResourceManagerStream<'_, R>,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<R::Format>> {
        let channels = data_stream.data_format()?.channels;
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        let mut buffer = SampleBuffer::<R::Format>::new_zeroed(frame_count as usize, channels)?;

        let frames_read = ma_resource_manager_data_stream_read_pcm_frames_internal::<R>(
            data_stream,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<R::Format>::from_storage(buffer, frames_read as usize, channels)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    fn ma_resource_manager_data_stream_read_pcm_frames_internal<R: AsRmPtr>(
        data_stream: &mut ResourceManagerStream<'_, R>,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_stream_read_pcm_frames(
                data_stream.to_raw(),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_read)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_stream_seek_to_pcm_frame<R: AsRmPtr>(
        data_stream: &mut ResourceManagerStream<'_, R>,
        frame: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_stream_seek_to_pcm_frame(data_stream.to_raw(), frame)
        };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_stream_get_data_format<R: AsRmPtr>(
        data_stream: &ResourceManagerStream<'_, R>,
    ) -> MaResult<DataFormat> {
        let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        let mut channels: u32 = 0;
        let mut sample_rate: u32 = 0;
        let mut channel_map_raw = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];
        let res = unsafe {
            sys::ma_resource_manager_data_stream_get_data_format(
                data_stream.to_raw(),
                &mut format_raw,
                &mut channels,
                &mut sample_rate,
                channel_map_raw.as_mut_ptr(),
                channel_map_raw.len(),
            )
        };
        MaudioError::check(res)?;
        // Could cast when passing the ptr to miniaudio, but copying should be fine here
        let mut channel_map: Vec<Channel> =
            channel_map_raw.into_iter().map(Channel::from_raw).collect();
        channel_map.truncate(channels as usize);

        Ok(DataFormat {
            format: format_raw.try_into()?,
            channels,
            sample_rate: sample_rate.try_into()?,
            channel_map: Some(channel_map),
        })
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_stream_get_cursor_in_pcm_frames<R: AsRmPtr>(
        data_stream: &ResourceManagerStream<'_, R>,
    ) -> MaResult<u64> {
        let mut cursor = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_stream_get_cursor_in_pcm_frames(
                data_stream.to_raw(),
                &mut cursor,
            )
        };
        MaudioError::check(res)?;
        Ok(cursor)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_stream_get_length_in_pcm_frames<R: AsRmPtr>(
        data_stream: &ResourceManagerStream<'_, R>,
    ) -> MaResult<u64> {
        let mut length: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_stream_get_length_in_pcm_frames(
                data_stream.to_raw(),
                &mut length,
            )
        };
        MaudioError::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_resource_manager_data_stream_result<R: AsRmPtr>(
        data_stream: &ResourceManagerStream<'_, R>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_stream_result(data_stream.to_raw()) };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_stream_set_looping<R: AsRmPtr>(
        data_stream: &ResourceManagerStream<'_, R>,
        is_looping: bool,
    ) -> MaResult<()> {
        let is_looping = is_looping as u32;
        let res = unsafe {
            sys::ma_resource_manager_data_stream_set_looping(data_stream.to_raw(), is_looping)
        };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    pub fn ma_resource_manager_data_stream_is_looping<R: AsRmPtr>(
        data_stream: &ResourceManagerStream<'_, R>,
    ) -> bool {
        let res = unsafe { sys::ma_resource_manager_data_stream_is_looping(data_stream.to_raw()) };
        res == 1
    }

    #[inline]
    pub fn ma_resource_manager_data_stream_get_available_frames<R: AsRmPtr>(
        data_stream: &ResourceManagerStream<'_, R>,
    ) -> MaResult<u64> {
        let mut frames: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_stream_get_available_frames(
                data_stream.to_raw(),
                &mut frames,
            )
        };
        MaudioError::check(res)?;
        Ok(frames)
    }

    // JOB MANAGEMENT
    pub fn ma_resource_manager_post_job<R: AsRmPtr>(
        rm: &R,
        job: *const sys::ma_job,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_post_job(private_rm::rm_ptr(rm), job) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_resource_manager_post_job_quit<R: AsRmPtr>(rm: &R) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_post_job_quit(private_rm::rm_ptr(rm)) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_resource_manager_next_job<R: AsRmPtr>(rm: &R, job: *mut sys::ma_job) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_next_job(private_rm::rm_ptr(rm), job) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_resource_manager_process_next_job<R: AsRmPtr>(rm: &R) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_process_next_job(private_rm::rm_ptr(rm)) };
        MaudioError::check(res)
    }
}

impl<F: PcmFormat> Drop for InnerResourceManager<F> {
    fn drop(&mut self) {
        resource_ffi::ma_resource_manager_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

fn tiny_test_wav_mono(frames: usize) -> Vec<u8> {
    let mut samples = Vec::with_capacity(frames);
    for i in 0..frames {
        samples.push(((i as i32 * 300) % i16::MAX as i32) as i16);
    }
    wav_i16_le(1, SampleRate::Sr44100, &samples)
}

#[cfg(test)]
mod test {
    use crate::{
        engine::resource::{
            rm_builder::ResourceManagerBuilder, rm_source::ResourceManagerSourceBuilder,
            rm_source_flags::RmSourceFlags, tiny_test_wav_mono, RmOps,
        },
        test_assets::{
            decoded_data::{
                asset_interleaved_f32, asset_interleaved_i16, asset_interleaved_i32,
                asset_interleaved_s24_i32, asset_interleaved_s24_packed_le, asset_interleaved_u8,
            },
            temp_file::{unique_tmp_path, TempFileGuard},
        },
    };

    #[test]
    fn test_resource_man_basic_init() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        drop(rm);
    }

    #[test]
    fn test_resource_man_basic_init_2() {
        let rm = ResourceManagerBuilder::new()
            .job_thread_count(1)
            .channels(2)
            .sample_rate(crate::audio::sample_rate::SampleRate::Sr11025)
            .build_f32()
            .unwrap();
        drop(rm);
    }

    #[test]
    fn test_resource_man_basic_register_file() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        let wav = tiny_test_wav_mono(20);
        let path_guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(path_guard.path(), &wav).unwrap();
        let guard = rm
            .register_file(path_guard.path(), RmSourceFlags::NONE)
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE).unwrap();
        let _strm = guard.build_stream(RmSourceFlags::NONE).unwrap();
    }

    #[test]
    fn test_resource_man_basic_register_encoded_data() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        let wav: Vec<u8> = tiny_test_wav_mono(20);
        let guard = rm.encoded("test:wav", &wav).unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE).unwrap();
    }

    #[test]
    fn test_resource_man_decoded_u8() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        let data = asset_interleaved_u8(2, 100, 1);
        let guard = rm
            .register_decoded_u8(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE).unwrap();
    }

    #[test]
    fn test_resource_man_decoded_i16() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        let data = asset_interleaved_i16(2, 100, 1);
        let guard = rm
            .register_decoded_i16(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE).unwrap();
    }

    #[test]
    fn test_resource_man_decoded_i32() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        let data = asset_interleaved_i32(2, 100, 1);
        let guard = rm
            .register_decoded_i32(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE).unwrap();
    }

    #[test]
    fn test_resource_man_decoded_f32() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        let data = asset_interleaved_f32(2, 100, 1.0);
        let guard = rm
            .register_decoded_f32(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let buf = guard.build_buffer(RmSourceFlags::NONE).unwrap();
        drop(buf);
        let src = guard.build_source(RmSourceFlags::NONE).unwrap();
        drop(src);
    }

    #[test]
    fn test_resource_man_decoded_s24_packed() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        let data = asset_interleaved_s24_packed_le(2, 100, 1);
        let guard = rm
            .register_decoded_s24_packed(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE).unwrap();
    }

    #[test]
    fn test_resource_man_decoded_s24() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        let data = asset_interleaved_s24_i32(2, 100, 1);
        let guard = rm
            .register_decoded_s24(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE).unwrap();
    }

    #[test]
    fn test_resource_man_moving_to_thread() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();

        let wav = tiny_test_wav_mono(20);
        let path_guard = TempFileGuard::new(unique_tmp_path("wav"));
        let path = path_guard.path().to_path_buf();
        std::fs::write(&path, &wav).unwrap();

        let moved_rm = rm.clone();
        let moved_path = path.clone();

        let handle = std::thread::spawn(move || {
            let _data_source = ResourceManagerSourceBuilder::new(&moved_rm)
                .file_path(&moved_path)
                .build()
                .unwrap();
        });
        handle.join().unwrap();

        let _data_source = ResourceManagerSourceBuilder::new(&rm)
            .file_path(&path)
            .build()
            .unwrap();
    }
}
