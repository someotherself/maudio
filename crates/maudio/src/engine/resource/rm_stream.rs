//! Resource-managed streaming audio sources
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

/// Resource-managed streaming audio source.
///
/// Wraps a miniaudio `ma_resource_manager_data_stream`.
///
/// A stream represents audio that is decoded incrementally during
/// playback, rather than fully loaded into memory.
///
/// # Async loading
///
/// Construction always returns a [`PendingResource`](crate::engine::resource).
/// When async flags are enabled, the source may not be immediately
/// ready and must be polled before use.
///
/// # Notes
///
/// - Data is streamed on demand during playback.
/// - Lower memory usage compared to buffers.
/// - Ideal for long audio (music, ambience, etc.).
pub struct ResourceManagerStream<F: PcmFormat, I> {
    pub(crate) inner: *mut sys::ma_resource_manager_data_stream,
    #[allow(unused)]
    pipeline_notif: Option<NotificationPipeline>,
    _format: PhantomData<F>,
    _guard: Arc<ResourceGuardInner<F, I>>,
}

unsafe impl<F: PcmFormat, I> Send for ResourceManagerStream<F, I> {}

impl<F: PcmFormat, I> Binding for ResourceManagerStream<F, I> {
    type Raw = *mut sys::ma_resource_manager_data_stream;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat, I> SharedSource for ResourceManagerStream<F, I> {}

#[doc(hidden)]
impl<F: PcmFormat, I> AsSourcePtr for ResourceManagerStream<F, I> {
    type Format = F;
    type __PtrProvider = private_data_source::ResourceManagerStreamProvider;
}

impl<F: PcmFormat, I> ResourceManagerStream<F, I> {
    pub fn as_source_ref<'a>(&'a self) -> DataSourceRef<'a, F> {
        debug_assert!(!self.to_raw().is_null());
        let ptr = self.to_raw().cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

// private methods
impl<F: PcmFormat, I> ResourceManagerStream<F, I> {
    fn new_with_config<'a>(config: &ResourceManagerStreamBuilder<'a, F, I>) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resource_manager_data_stream>> =
            Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_data_stream_init_ex(
            config.guard.0.rm,
            config,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_resource_manager_data_stream =
            Box::into_raw(mem) as *mut sys::ma_resource_manager_data_stream;

        Ok(Self {
            inner,
            pipeline_notif: None,
            _format: PhantomData,
            _guard: config.guard.0.clone(),
        })
    }
}

pub(crate) struct ResourceManagerStreamBuilder<'a, F: PcmFormat, I> {
    guard: &'a ResourceGuard<F, I>,
    inner: sys::ma_resource_manager_data_source_config,
    #[allow(unused)]
    flags: RmSourceFlags,
    source: SourceBufSource<'a>,
    owned_path: OwnedPathBuf,
    #[allow(unused)]
    pipeline_notif: Option<NotificationPipeline>,
}

impl<'a, F: PcmFormat, I> AsRawRef for ResourceManagerStreamBuilder<'a, F, I> {
    type Raw = sys::ma_resource_manager_data_source_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<'a, F: PcmFormat, I> ResourceManagerStreamBuilder<'a, F, I> {
    pub(crate) fn new(guard: &'a ResourceGuard<F, I>) -> Self {
        let inner = unsafe { sys::ma_resource_manager_data_source_config_init() };
        Self {
            guard,
            inner,
            flags: RmSourceFlags::NONE,
            source: SourceBufSource::None,
            owned_path: OwnedPathBuf::None,
            pipeline_notif: None,
        }
    }

    pub(crate) fn flags(&mut self, flags: RmSourceFlags) -> &mut Self {
        self.inner.flags = flags.bits();
        self
    }

    pub(crate) fn file_path(&mut self, path: &'a Path) -> &mut Self {
        self.source = SourceBufSource::None;
        #[cfg(not(windows))]
        {
            self.source = SourceBufSource::FileUtf8(path);
        }
        #[cfg(windows)]
        {
            self.source = SourceBufSource::FileWide(path);
        }
        self
    }

    pub(crate) fn notification(&mut self, notif: NotificationPipeline) -> &mut Self {
        self.inner.pNotifications = notif.as_raw_ptr();
        self.pipeline_notif = Some(notif);
        self
    }

    fn set_source(&mut self) -> MaResult<()> {
        let null_fields = |cfg: &mut ResourceManagerStreamBuilder<'_, F, I>| {
            cfg.inner.pFilePath = core::ptr::null();
            cfg.inner.pFilePathW = core::ptr::null();
        };
        match self.source {
            SourceBufSource::None => null_fields(self),
            #[cfg(not(windows))]
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

    pub(crate) fn build_internal(&mut self) -> MaResult<ResourceManagerStream<F, I>> {
        self.set_source()?;
        ResourceManagerStream::<F, I>::new_with_config(self)
    }
}

impl<F: PcmFormat, I> Drop for ResourceManagerStream<F, I> {
    fn drop(&mut self) {
        let _ = resource_ffi::ma_resource_manager_data_stream_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
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
    fn test_rm_stream_multiple_load_stream_guards() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let guard1 = rm.register_encoded("wav", &wav).unwrap();
        let res = guard1.build_stream(RmSourceFlags::NONE, None);
        // TODO: Document this
        assert!(res.is_err());
    }

    #[test]
    fn test_rm_stream_thread_load_stream() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let rm_clone = rm.clone();

        let _guard1 = rm.register_encoded("wav", &wav).unwrap();

        let handle = std::thread::spawn(move || {
            let guard = rm_clone.load_name("wav").unwrap();

            let _stream = guard.build_stream(RmSourceFlags::NONE, None).unwrap();
        });

        let _ = handle.join();
    }

    #[test]
    fn test_rm_stream_multiple_path_load_stream_guards() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let guard1 = rm.register_file(guard.path(), RmSourceFlags::NONE).unwrap();
        let stream1 = guard1.build_stream(RmSourceFlags::NONE, None).unwrap();
        let guard2 = rm.load_path(guard.path()).unwrap();
        let stream2 = guard2.build_stream(RmSourceFlags::NONE, None).unwrap();
        let guard3 = rm.load_path(guard.path()).unwrap();
        let stream3 = guard3.build_stream(RmSourceFlags::NONE, None).unwrap();
        let guard4 = rm.load_path(guard.path()).unwrap();
        let stream4 = guard4.build_stream(RmSourceFlags::NONE, None).unwrap();
        let guard5 = rm.load_path(guard.path()).unwrap();
        let stream5 = guard5.build_stream(RmSourceFlags::NONE, None).unwrap();

        drop(stream1);
        drop(guard1);
        drop(stream2);
        drop(guard2);
        drop(stream3);
        drop(guard3);
        drop(stream4);
        drop(guard4);
        drop(stream5);
        drop(guard5);
    }

    #[test]
    fn test_rm_stream_thread_path_load_stream() {
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

            let _buffer = guard.build_stream(RmSourceFlags::NONE, None).unwrap();
        });

        let _ = handle.join();
    }
}
