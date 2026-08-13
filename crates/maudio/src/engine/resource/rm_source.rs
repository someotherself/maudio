//! Resource-managed audio data source
use std::{marker::PhantomData, mem::MaybeUninit, path::Path, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    data_source::{private_data_source, AsSourcePtr, DataSourceRef, SharedSource},
    engine::resource::{
        resource_ffi, rm_notif::NotificationPipeline, rm_source_flags::RmSourceFlags,
        ResourceGuard, ResourceGuardInner,
    },
    pcm_frames::PcmFormat,
    sound::sound_builder::OwnedPathBuf,
    AsRawRef, Binding, MaResult,
};

/// Resource-managed audio data source.
///
/// This is a lightweight wrapper around a miniaudio
/// `ma_resource_manager_data_source`.
///
/// A source represents a decoded audio input that can be used by
/// sounds, nodes, or other playback components without fully loading
/// the entire audio into memory.
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
/// - Decoding is handled internally by the resource manager.
/// - Does not guarantee full data residency in memory.
/// - Suitable for general playback and flexible data access.
pub struct ResourceManagerSource<F: PcmFormat, I> {
    inner: *mut sys::ma_resource_manager_data_source,
    #[allow(unused)]
    pipeline_notif: Option<NotificationPipeline>,
    _format: PhantomData<F>,
    _guard: Arc<ResourceGuardInner<F, I>>,
}

unsafe impl<F: PcmFormat, I> Send for ResourceManagerSource<F, I> {}

impl<F: PcmFormat, I> Binding for ResourceManagerSource<F, I> {
    type Raw = *mut sys::ma_resource_manager_data_source;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat, I> SharedSource for ResourceManagerSource<F, I> {}

#[doc(hidden)]
impl<F: PcmFormat, I> AsSourcePtr for ResourceManagerSource<F, I> {
    type Format = F;
    type __PtrProvider = private_data_source::ResourceManagerSourceProvider;
}

impl<F: PcmFormat, I> ResourceManagerSource<F, I> {
    pub fn as_source_ref<'a>(&'a self) -> DataSourceRef<'a, F> {
        debug_assert!(!self.to_raw().is_null());
        let ptr = self.to_raw().cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

// Private methods
impl<F: PcmFormat, I> ResourceManagerSource<F, I> {
    fn new_with_config<'a>(config: &ResourceManagerSourceBuilder<'a, F, I>) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resource_manager_data_source>> =
            Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_data_source_init_ex(
            config.guard.0.rm,
            config,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_resource_manager_data_source =
            Box::into_raw(mem) as *mut sys::ma_resource_manager_data_source;
        Ok(Self {
            inner,
            pipeline_notif: None,
            _format: PhantomData,
            _guard: config.guard.0.clone(),
        })
    }
}

pub(crate) struct ResourceManagerSourceBuilder<'a, F: PcmFormat, I> {
    guard: &'a ResourceGuard<F, I>,
    inner: sys::ma_resource_manager_data_source_config,
    flags: RmSourceFlags,
    source: SourceBufSource<'a>,
    owned_path: OwnedPathBuf,
    #[allow(unused)]
    pipeline_notif: Option<NotificationPipeline>,
}

impl<'a, F: PcmFormat, I> AsRawRef for ResourceManagerSourceBuilder<'a, F, I> {
    type Raw = sys::ma_resource_manager_data_source_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

#[derive(PartialEq)]
pub(crate) enum SourceBufSource<'a> {
    None,
    #[cfg(unix)]
    FileUtf8(&'a Path),
    #[cfg(windows)]
    FileWide(&'a Path),
}

// TODO:
// initialSeekPointInPCMFrames;
// rangeBegInPCMFrames;
// rangeEndInPCMFrames;
// loopPointBegInPCMFrames;
// loopPointEndInPCMFrames;
impl<'a, F: PcmFormat, I> ResourceManagerSourceBuilder<'a, F, I> {
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

    pub(crate) fn notification(&mut self, notif: NotificationPipeline) -> &mut Self {
        self.inner.pNotifications = notif.as_raw_ptr();
        self.pipeline_notif = Some(notif);
        self
    }

    fn set_source(&mut self) -> MaResult<()> {
        let null_fields = |cfg: &mut ResourceManagerSourceBuilder<'a, F, I>| {
            cfg.inner.pFilePath = core::ptr::null();
            cfg.inner.pFilePathW = core::ptr::null();
        };
        match self.source {
            SourceBufSource::None => null_fields(self),
            #[cfg(unix)]
            SourceBufSource::FileUtf8(p) => {
                use crate::sound::sound_builder::OwnedPathBuf;

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

    pub(crate) fn build_internal(&mut self) -> MaResult<ResourceManagerSource<F, I>> {
        self.set_source()?;
        ResourceManagerSource::<F, I>::new_with_config(self)
    }
}

impl<F: PcmFormat, I> Drop for ResourceManagerSource<F, I> {
    fn drop(&mut self) {
        let _ = resource_ffi::ma_resource_manager_data_source_uninit(self);
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
    fn test_rm_source_multiple_load_buffer_guards() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let guard1 = rm.register_encoded("wav", &wav).unwrap();
        let source1 = guard1.build_source(RmSourceFlags::NONE, None).unwrap();
        let guard2 = rm.load_name("wav").unwrap();
        let source2 = guard2.build_source(RmSourceFlags::NONE, None).unwrap();
        let guard3 = rm.load_name("wav").unwrap();
        let source3 = guard3.build_source(RmSourceFlags::NONE, None).unwrap();
        let guard4 = rm.load_name("wav").unwrap();
        let source4 = guard4.build_source(RmSourceFlags::NONE, None).unwrap();
        let guard5 = rm.load_name("wav").unwrap();
        let source5 = guard5.build_source(RmSourceFlags::NONE, None).unwrap();

        drop(source1);
        drop(guard1);
        drop(source2);
        drop(guard2);
        drop(source3);
        drop(guard3);
        drop(source4);
        drop(guard4);
        drop(source5);
        drop(guard5);
    }

    #[test]
    fn test_rm_stream_thread_load_buffer() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let rm_clone = rm.clone();

        let _guard1 = rm.register_encoded("wav", &wav).unwrap();

        let handle = std::thread::spawn(move || {
            let guard = rm_clone.load_name("wav").unwrap();

            let _source = guard.build_source(RmSourceFlags::NONE, None).unwrap();
        });

        let _ = handle.join();
    }

    #[test]
    fn test_rm_source_multiple_path_load_buffer_guards() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let guard1 = rm.register_encoded("wav", &wav).unwrap();
        let source1 = guard1.build_source(RmSourceFlags::NONE, None).unwrap();
        let guard2 = rm.load_name("wav").unwrap();
        let source2 = guard2.build_source(RmSourceFlags::NONE, None).unwrap();
        let guard3 = rm.load_name("wav").unwrap();
        let source3 = guard3.build_source(RmSourceFlags::NONE, None).unwrap();
        let guard4 = rm.load_name("wav").unwrap();
        let source4 = guard4.build_source(RmSourceFlags::NONE, None).unwrap();
        let guard5 = rm.load_name("wav").unwrap();
        let source5 = guard5.build_source(RmSourceFlags::NONE, None).unwrap();

        drop(source1);
        drop(guard1);
        drop(source2);
        drop(guard2);
        drop(source3);
        drop(guard3);
        drop(source4);
        drop(guard4);
        drop(source5);
        drop(guard5);
    }

    #[test]
    fn test_rm_source_thread_path_load_buffer() {
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

            let _buffer = guard.build_source(RmSourceFlags::NONE, None).unwrap();
        });

        let _ = handle.join();
    }
}
