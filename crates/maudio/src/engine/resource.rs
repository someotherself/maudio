use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    path::{Path, PathBuf},
};

use maudio_sys::ffi as sys;

use crate::{
    audio::{
        formats::{Format, SampleBuffer},
        sample_rate::SampleRate,
    },
    engine::resource::rm_builder::ResourceManagerBuilder,
    pcm_frames::{PcmFormatInternal, S24Packed, S24},
    test_assets::wav_i16_le,
    AsRawRef, Binding, MaResult,
};

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

pub struct ResourceRegistration<'a, R: AsRmPtr + ?Sized> {
    rm: &'a R,
}

impl<'a, R: AsRmPtr + RmOps> ResourceRegistration<'a, R> {
    pub fn file(self, path: &Path, flags: u32) -> MaResult<ResourceGuard<'a, R>> {
        #[cfg(unix)]
        {
            use crate::engine::cstring_from_path;

            let c_path = cstring_from_path(path)?;
            resource_ffi::ma_resource_manager_register_file(self.rm, c_path, flags)?;
            Ok(ResourceGuard::from_path(self.rm, path))
        }

        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated;

            let c_path = wide_null_terminated(path);

            resource_ffi::ma_resource_manager_register_file_w(self.rm, &c_path, flags)?;
            Ok(ResourceGuard::from_path(self.rm, path))
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    fn decoded_u8(
        self,
        name: &str,
        data: &'a [u8],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, R>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<u8, R>(
            self.rm,
            name,
            data,
            Format::U8,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self.rm, name, None))
    }

    fn decoded_i16(
        self,
        name: &str,
        data: &'a [i16],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, R>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<i16, R>(
            self.rm,
            name,
            data,
            Format::S16,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self.rm, name, None))
    }

    fn decoded_i32(
        self,
        name: &str,
        data: &'a [i32],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, R>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<i32, R>(
            self.rm,
            name,
            data,
            Format::S32,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self.rm, name, None))
    }

    fn decoded_s24_packed(
        self,
        name: &str,
        data: &'a [u8],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, R>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<S24Packed, R>(
            self.rm,
            name,
            data,
            Format::S24,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self.rm, name, None))
    }

    fn decoded_s24(
        self,
        name: &str,
        data: &[i32],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, R>> {
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
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<S24Packed, R>(
            self.rm,
            name,
            &dst,
            Format::S24,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self.rm, name, Some(dst)))
    }

    fn decoded_f32(
        self,
        name: &str,
        data: &'a [f32],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<'a, R>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<f32, R>(
            self.rm,
            name,
            data,
            Format::F32,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::from_data(self.rm, name, None))
    }

    fn encoded(self, name: &str, data: &'a [u8]) -> MaResult<ResourceGuard<'a, R>> {
        resource_ffi::ma_resource_manager_register_encoded_data_internal(self.rm, name, data)?;
        Ok(ResourceGuard::from_data(self.rm, name, None))
    }
}

pub struct ResourceGuard<'a, R: AsRmPtr + ?Sized> {
    rm: &'a R,
    data_name: RegisteredDataType,
    data_store: Option<Vec<u8>>,
    _data_marker: PhantomData<&'a [u8]>,
}

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

    pub(crate) fn from_data(rm: &'a R, name: &str, data: Option<Vec<u8>>) -> Self {
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

impl RmOps for ResourceManager {}
impl RmOps for ResourceManagerRef<'_> {}

pub trait RmOps: AsRmPtr {
    fn register<'a>(&'a self) -> ResourceRegistration<'a, Self> {
        ResourceRegistration { rm: self }
    }
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
    use std::path::Path;

    use crate::{
        engine::resource::{private_rm, AsRmPtr},
        ErrorKinds,
    };
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
                name.as_ptr(),
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
        rm: &mut R,
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
            ma_resource_manager_unregister_file_w(rm, c_path)
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
            ma_resource_manager_unregister_data_w(rm, name.as_ptr())
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

impl Drop for ResourceManager {
    fn drop(&mut self) {
        resource_ffi::ma_resource_manager_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

fn tiny_test_wav_mono(frames: usize) -> Vec<u8> {
    let mut samples = Vec::with_capacity(frames);
    for i in 0..frames {
        samples.push(((i as i32 * 300) % i16::MAX as i32) as i16);
    }
    wav_i16_le(1, 48_000, &samples)
}

#[cfg(test)]
mod test {
    use crate::{
        engine::resource::{rm_builder::ResourceManagerBuilder, tiny_test_wav_mono, RmOps},
        test_assets::{
            decoded_data::{
                asset_interleaved_f32, asset_interleaved_i16, asset_interleaved_i32,
                asset_interleaved_s24_i32, asset_interleaved_s24_packed_le, asset_interleaved_u8,
            },
            temp_file::{unique_tmp_path, TempFileGuard},
        },
    };

    #[test]
    fn resource_man_basic_init() {
        let rm = ResourceManagerBuilder::new().build().unwrap();
        drop(rm);
    }

    #[test]
    fn resource_man_basic_init_2() {
        let rm = ResourceManagerBuilder::new()
            .job_thread_count(1)
            .channels(2)
            .sample_rate(crate::audio::sample_rate::SampleRate::Sr11025)
            .format(crate::audio::formats::Format::F32)
            .build()
            .unwrap();
        drop(rm);
    }

    #[test]
    fn resource_man_basic_register_file() {
        let rm = ResourceManagerBuilder::new().build().unwrap();
        let wav = tiny_test_wav_mono(20);
        let path_guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(path_guard.path(), &wav).unwrap();
        let guard = rm.register().file(path_guard.path(), 0).unwrap();
        drop(guard);
    }

    #[test]
    fn resource_man_basic_register_encoded_data() {
        let rm = ResourceManagerBuilder::new().build().unwrap();
        let wav: Vec<u8> = tiny_test_wav_mono(20);
        let guard = rm.register().encoded("test:wav", &wav).unwrap();
        drop(guard);
    }

    #[test]
    fn resource_man_decoded_u8() {
        let rm = ResourceManagerBuilder::new().build().unwrap();
        let data = asset_interleaved_u8(2, 100, 1);
        let guard = rm
            .register()
            .decoded_u8(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        drop(guard);
    }

    #[test]
    fn resource_man_decoded_i16() {
        let rm = ResourceManagerBuilder::new().build().unwrap();
        let data = asset_interleaved_i16(2, 100, 1);
        let guard = rm
            .register()
            .decoded_i16(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        drop(guard);
    }

    #[test]
    fn resource_man_decoded_i32() {
        let rm = ResourceManagerBuilder::new().build().unwrap();
        let data = asset_interleaved_i32(2, 100, 1);
        let guard = rm
            .register()
            .decoded_i32(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        drop(guard);
    }

    #[test]
    fn resource_man_decoded_f32() {
        let rm = ResourceManagerBuilder::new().build().unwrap();
        let data = asset_interleaved_f32(2, 100, 1.0);
        let guard = rm
            .register()
            .decoded_f32(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        drop(guard);
    }

    #[test]
    fn resource_man_decoded_s24_packed() {
        let rm = ResourceManagerBuilder::new().build().unwrap();
        let data = asset_interleaved_s24_packed_le(2, 100, 1);
        let guard = rm
            .register()
            .decoded_s24_packed(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        drop(guard);
    }

    #[test]
    fn resource_man_decoded_s24() {
        let rm = ResourceManagerBuilder::new().build().unwrap();
        let data = asset_interleaved_s24_i32(2, 100, 1);
        let guard = rm
            .register()
            .decoded_s24(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        drop(guard);
    }
}
