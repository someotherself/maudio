use std::{marker::PhantomData, mem::MaybeUninit, path::Path};

use maudio_sys::ffi as sys;

use crate::{
    data_source::{private_data_source, AsSourcePtr, DataSourceRef, SharedSource},
    engine::resource::{
        resource_ffi, rm_source::SourceBufSource, rm_source_flags::RmSourceFlags, AsRmPtr,
        PendingResource,
    },
    sound::sound_builder::OwnedPathBuf,
    util::fence::Fence,
    AsRawRef, Binding, MaResult,
};

#[derive(Debug)]
pub struct ResourceManagerStream<'a, R: AsRmPtr + ?Sized> {
    inner: *mut sys::ma_resource_manager_data_stream,
    _format: PhantomData<R::Format>,
    _marker: PhantomData<&'a R>,
}

impl<'a, R: AsRmPtr + ?Sized> Binding for ResourceManagerStream<'a, R> {
    type Raw = *mut sys::ma_resource_manager_data_stream;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a, R: AsRmPtr> SharedSource for ResourceManagerStream<'a, R> {
    type Format = R::Format;
}

#[doc(hidden)]
impl<'a, R: AsRmPtr> AsSourcePtr for ResourceManagerStream<'a, R> {
    type __PtrProvider = private_data_source::ResourceManagerStreamProvider;
}

impl<'a, R: AsRmPtr> ResourceManagerStream<'a, R> {
    pub fn as_source(&'a self) -> DataSourceRef<'a> {
        debug_assert!(!self.to_raw().is_null());
        let ptr = self.to_raw().cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

pub struct ResourceManagerStreamBuilder<'a, R: AsRmPtr + ?Sized> {
    rm: &'a R,
    inner: sys::ma_resource_manager_data_source_config,
    flags: RmSourceFlags,
    fence: Option<&'a Fence>,
    source: SourceBufSource<'a>,
    owned_path: OwnedPathBuf,
}

impl<'a, R: AsRmPtr + ?Sized> AsRawRef for ResourceManagerStreamBuilder<'a, R> {
    type Raw = sys::ma_resource_manager_data_source_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

// private methods
impl<'a, R: AsRmPtr> ResourceManagerStream<'a, R> {
    fn new_with_config(config: &ResourceManagerStreamBuilder<'a, R>) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resource_manager_data_stream>> =
            Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_data_stream_init_ex(config.rm, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_resource_manager_data_stream =
            Box::into_raw(mem) as *mut sys::ma_resource_manager_data_stream;

        Ok(Self {
            inner,
            _format: PhantomData,
            _marker: PhantomData,
        })
    }
}

impl<'a, R: AsRmPtr> ResourceManagerStreamBuilder<'a, R> {
    pub fn new(rm: &'a R) -> Self {
        let inner = unsafe { sys::ma_resource_manager_data_source_config_init() };
        Self {
            rm,
            inner,
            flags: RmSourceFlags::NONE,
            fence: None,
            source: SourceBufSource::None,
            owned_path: OwnedPathBuf::None,
        }
    }

    pub fn flags(&mut self, flags: RmSourceFlags) -> &mut Self {
        self.inner.flags = flags.bits();
        self
    }

    pub fn file_path(&mut self, path: &'a Path) -> &mut Self {
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

    fn set_source(&mut self) -> MaResult<()> {
        let null_fields = |cfg: &mut ResourceManagerStreamBuilder<'_, R>| {
            cfg.inner.pFilePath = core::ptr::null();
            cfg.inner.pFilePathW = core::ptr::null();
        };
        match self.source {
            SourceBufSource::None => null_fields(self),
            #[cfg(unix)]
            SourceBufSource::FileUtf8(p) => {
                null_fields(self);
                let cstring = crate::engine::cstring_from_path(p)?;
                self.inner.pFilePath = cstring.as_ptr();
                self.owned_path = OwnedPathBuf::Utf8(cstring); // keep the pointer alive
            }
            #[cfg(windows)]
            SourceBufSource::FileWide(p) => {
                null_fields(self);
                let wide_path = crate::engine::wide_null_terminated(p);
                self.inner.pFilePathW = wide_path.as_ptr();
                self.owned_path = OwnedPathBuf::Wide(wide_path); // keep the pointer alive
            }
        }
        Ok(())
    }

    pub fn async_load(&mut self, yes: bool) -> &mut Self {
        let mut flags = RmSourceFlags::from_bits(self.inner.flags);
        if yes {
            flags.insert(RmSourceFlags::ASYNC);
        } else {
            flags.remove(RmSourceFlags::ASYNC);
        }
        self.inner.flags = flags.bits();
        self.flags = flags;
        self
    }

    pub fn fence(&mut self, fence: &'a Fence) -> &mut Self {
        self.fence = Some(fence);
        self.async_load(true);
        self
    }

    pub(crate) fn build_internal(&mut self) -> MaResult<ResourceManagerStream<'a, R>> {
        self.set_source()?;
        ResourceManagerStream::<R>::new_with_config(self)
    }

    pub fn build(&mut self) -> MaResult<PendingResource<ResourceManagerStream<'a, R>>> {
        let buf = self.build_internal()?;
        if self.flags.intersects(RmSourceFlags::ASYNC) {
            return Ok(PendingResource::Pending { inner: Some(buf) });
        }
        Ok(PendingResource::Ready { inner: buf })
    }
}

impl<'a, R: AsRmPtr + ?Sized> Drop for ResourceManagerStream<'a, R> {
    fn drop(&mut self) {
        let _ = resource_ffi::ma_resource_manager_data_stream_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

#[cfg(test)]
mod test {
    use crate::{
        engine::resource::{
            rm_builder::ResourceManagerBuilder, rm_stream::ResourceManagerStreamBuilder,
            tiny_test_wav_mono,
        },
        test_assets::temp_file::{unique_tmp_path, TempFileGuard},
    };

    #[test]
    fn test_res_man_data_source_stream_builder_basic_init() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();

        let wav = tiny_test_wav_mono(20);
        let path_guard = TempFileGuard::new(unique_tmp_path("wav"));
        let path = path_guard.path().to_path_buf();
        std::fs::write(&path, &wav).unwrap();

        let _ = ResourceManagerStreamBuilder::new(&rm)
            .file_path(&path)
            .build()
            .unwrap();
    }
}
