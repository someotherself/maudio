//! Resource-managed decoded audio buffer
use std::{marker::PhantomData, mem::MaybeUninit, path::Path, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    data_source::{private_data_source, AsSourcePtr, DataSourceRef, SharedSource},
    engine::resource::{
        resource_ffi, rm_notif::NotificationPipeline, rm_source::SourceBufSource,
        rm_source_flags::RmSourceFlags, ResourceGuard, ResourceGuardInner,
    },
    pcm_frames::PcmFormat,
    sound::sound_builder::OwnedPathBuf,
    AsRawRef, Binding, MaResult,
};

/// Resource-managed fully loaded audio buffer.
///
/// Wraps a miniaudio `ma_resource_manager_data_buffer`.
///
/// A buffer represents audio data that is fully decoded and stored
/// in memory, allowing fast, repeated access without additional I/O
/// or decoding overhead.
///
/// # Async loading
///
/// Construction always returns a [`PendingResource`](crate::engine::resource).
/// When async flags are enabled, the source may not be immediately
/// ready and must be polled before use.
///
/// Otherwise, the builder returns once the resource is available and
/// [`PendingResource`](crate::engine::resource) is `Ready`.
///
/// # Notes
///
/// - Entire audio data is in memory.
/// - Best suited for short sounds or frequently reused assets.
/// - Higher memory usage compared to streaming or sources.
pub struct ResourceManagerBuffer<F: PcmFormat, I> {
    pub(crate) inner: *mut sys::ma_resource_manager_data_buffer,
    #[allow(unused)]
    pipeline_notif: Option<NotificationPipeline>,
    _format: PhantomData<F>,
    _guard: Arc<ResourceGuardInner<F, I>>,
}

unsafe impl<F: PcmFormat, I> Send for ResourceManagerBuffer<F, I> {}

impl<F: PcmFormat, I> Binding for ResourceManagerBuffer<F, I> {
    type Raw = *mut sys::ma_resource_manager_data_buffer;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat, I> SharedSource for ResourceManagerBuffer<F, I> {}

#[doc(hidden)]
impl<F: PcmFormat, I> AsSourcePtr for ResourceManagerBuffer<F, I> {
    type Format = F;
    type __PtrProvider = private_data_source::ResourceManagerBufferProvider;
}

impl<F: PcmFormat, I> ResourceManagerBuffer<F, I> {
    pub fn as_source_ref<'a>(&'a self) -> DataSourceRef<'a, F> {
        debug_assert!(!self.to_raw().is_null());
        let ptr = self.to_raw().cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

// private methods
impl<F: PcmFormat, I> ResourceManagerBuffer<F, I> {
    fn new_with_config<'a>(config: &ResourceManagerBufferBuilder<'a, F, I>) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resource_manager_data_buffer>> =
            Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_data_buffer_init_ex(
            config.guard.0.rm,
            config,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_resource_manager_data_buffer =
            Box::into_raw(mem) as *mut sys::ma_resource_manager_data_buffer;

        Ok(Self {
            inner,
            pipeline_notif: None, // config.pNotifications do not get carried over. PipeNotif will be lost.
            _format: PhantomData,
            _guard: config.guard.0.clone(),
        })
    }
}

impl<F: PcmFormat, I> Drop for ResourceManagerBuffer<F, I> {
    fn drop(&mut self) {
        let _ = resource_ffi::ma_resource_manager_data_buffer_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

/// Builder for creating a [`ResourceManagerBuffer`] without registration
pub(crate) struct ResourceManagerBufferBuilder<'a, F: PcmFormat, I> {
    guard: &'a ResourceGuard<F, I>,
    inner: sys::ma_resource_manager_data_source_config,
    flags: RmSourceFlags,
    source: SourceBufSource<'a>,
    owned_path: OwnedPathBuf,
    #[allow(unused)]
    pipeline_notif: Option<NotificationPipeline>,
}

impl<'a, F: PcmFormat, I> AsRawRef for ResourceManagerBufferBuilder<'a, F, I> {
    type Raw = sys::ma_resource_manager_data_source_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<'a, F: PcmFormat, I> ResourceManagerBufferBuilder<'a, F, I> {
    pub(crate) fn new_internal(
        guard: &'a ResourceGuard<F, I>,
        inner: sys::ma_resource_manager_data_source_config,
    ) -> Self {
        Self {
            guard,
            inner,
            flags: RmSourceFlags::NONE,
            source: SourceBufSource::None,
            owned_path: OwnedPathBuf::None,
            pipeline_notif: None,
        }
    }

    pub(crate) fn new(guard: &'a ResourceGuard<F, I>) -> Self {
        let inner = unsafe { sys::ma_resource_manager_data_source_config_init() };
        ResourceManagerBufferBuilder::new_internal(guard, inner)
    }

    pub(crate) fn flags(&mut self, flags: RmSourceFlags) -> &mut Self {
        self.inner.flags = flags.bits();
        self.flags = flags;
        self
    }

    pub(crate) fn file_path(&mut self, path: &'a Path) -> &mut Self {
        self.source = SourceBufSource::None;
        #[cfg(unix)]
        {
            self.source = SourceBufSource::FileUtf8(path);
        }
        #[cfg(windows)]
        {
            self.source = SourceBufSource::FileWide(path);
        }
        self
    }

    /// Add either an `init` or `done` [`NotificationPipeline`]
    pub(crate) fn notification(&mut self, notif: NotificationPipeline) -> &mut Self {
        self.inner.pNotifications = notif.as_raw_ptr();
        self.pipeline_notif = Some(notif);
        self
    }

    fn set_source(&mut self) -> MaResult<()> {
        let null_fields = |cfg: &mut ResourceManagerBufferBuilder<'_, F, I>| {
            cfg.inner.pFilePath = core::ptr::null();
            cfg.inner.pFilePathW = core::ptr::null();
        };
        match self.source {
            SourceBufSource::None => null_fields(self),
            #[cfg(unix)]
            SourceBufSource::FileUtf8(p) => {
                null_fields(self);
                let cstring = crate::cstring_from_path(p)?;
                self.inner.pFilePath = cstring.as_ptr();
                self.owned_path = OwnedPathBuf::Utf8(cstring); // keep the pointer alive
            }
            #[cfg(windows)]
            SourceBufSource::FileWide(p) => {
                null_fields(self);
                let wide_path = crate::wide_null_terminated(p);
                self.inner.pFilePathW = wide_path.as_ptr();
                self.owned_path = OwnedPathBuf::Wide(wide_path); // keep the pointer alive
            }
        }
        Ok(())
    }

    pub(crate) fn build_internal(&mut self) -> MaResult<ResourceManagerBuffer<F, I>> {
        self.set_source()?;
        ResourceManagerBuffer::<F, I>::new_with_config(self)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        audio::sample_rate::SampleRate,
        engine::resource::{rm_builder::ResourceManagerBuilder, RmOps},
        test_assets::{
            temp_file::{unique_tmp_path, TempFileGuard},
            wav_i16_le,
        },
    };

    use super::*;

    fn tiny_test_wav_mono(frames: usize) -> Vec<u8> {
        let mut samples = Vec::with_capacity(frames);
        for i in 0..frames {
            samples.push(((i as i32 * 300) % i16::MAX as i32) as i16);
        }
        wav_i16_le(1, SampleRate::Sr48000, &samples)
    }

    #[test]
    fn test_rm_buffer_basic_guard() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let guard = rm.register_encoded("wav", &wav).unwrap();

        drop(guard);
    }

    #[test]
    fn test_rm_buffer_multiple_guards() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let guard1 = rm.register_encoded("wav", &wav).unwrap();
        let guard2 = rm.register_encoded("wav", &wav).unwrap();
        let guard3 = rm.register_encoded("wav", &wav).unwrap();
        let guard4 = rm.register_encoded("wav", &wav).unwrap();
        let guard5 = rm.register_encoded("wav", &wav).unwrap();

        drop(guard1);
        drop(guard2);
        drop(guard3);
        drop(guard4);
        drop(guard5);
    }

    #[test]
    fn test_rm_buffer_multiple_load_guards() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let guard1 = rm.register_encoded("wav", &wav).unwrap();
        let guard2 = rm.load_name("wav").unwrap();
        let guard3 = rm.load_name("wav").unwrap();
        let guard4 = rm.load_name("wav").unwrap();
        let guard5 = rm.load_name("wav").unwrap();

        drop(guard1);
        drop(guard2);
        drop(guard3);
        drop(guard4);
        drop(guard5);
    }

    #[test]
    fn test_rm_buffer_multiple_load_buffer_guards() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let guard1 = rm.register_encoded("wav", &wav).unwrap();
        let buffer1 = guard1.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let guard2 = rm.load_name("wav").unwrap();
        let buffer2 = guard2.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let guard3 = rm.load_name("wav").unwrap();
        let buffer3 = guard3.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let guard4 = rm.load_name("wav").unwrap();
        let buffer4 = guard4.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let guard5 = rm.load_name("wav").unwrap();
        let buffer5 = guard5.build_buffer(RmSourceFlags::NONE, None).unwrap();

        drop(buffer1);
        drop(guard1);
        drop(buffer2);
        drop(guard2);
        drop(buffer3);
        drop(guard3);
        drop(buffer4);
        drop(guard4);
        drop(buffer5);
        drop(guard5);
    }

    #[test]
    fn test_rm_buffer_thread_load_buffer() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let rm_clone = rm.clone();

        let _guard1 = rm.register_encoded("wav", &wav).unwrap();

        let handle = std::thread::spawn(move || {
            let guard = rm_clone.load_name("wav").unwrap();

            let _buffer = guard.build_buffer(RmSourceFlags::NONE, None).unwrap();
        });

        let _ = handle.join();
    }

    #[test]
    fn test_rm_buffer_multiple_path_load_buffer_guards() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let guard1 = rm.register_file(guard.path(), RmSourceFlags::NONE).unwrap();
        let buffer1 = guard1.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let guard2 = rm.load_path(guard.path()).unwrap();
        let buffer2 = guard2.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let guard3 = rm.load_path(guard.path()).unwrap();
        let buffer3 = guard3.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let guard4 = rm.load_path(guard.path()).unwrap();
        let buffer4 = guard4.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let guard5 = rm.load_path(guard.path()).unwrap();
        let buffer5 = guard5.build_buffer(RmSourceFlags::NONE, None).unwrap();

        drop(buffer1);
        drop(guard1);
        drop(buffer2);
        drop(guard2);
        drop(buffer3);
        drop(guard3);
        drop(buffer4);
        drop(guard4);
        drop(buffer5);
        drop(guard5);
    }

    #[test]
    fn test_rm_buffer_thread_path_load_buffer() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let rm_clone = rm.clone();
        let path_clone = guard.path().to_path_buf();

        let _guard1 = rm.register_encoded("wav", &wav).unwrap();

        let handle = std::thread::spawn(move || {
            let guard = rm_clone.load_path(&path_clone).unwrap();

            let _buffer = guard.build_buffer(RmSourceFlags::NONE, None).unwrap();
        });

        let _ = handle.join();
    }
}
