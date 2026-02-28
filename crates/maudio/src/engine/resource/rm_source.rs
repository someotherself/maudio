use std::{marker::PhantomData, mem::MaybeUninit, path::Path};

use maudio_sys::ffi as sys;

use crate::{
    data_source::{private_data_source, AsSourcePtr, DataSourceRef, SharedSource},
    engine::resource::{resource_ffi, rm_source_flags::RmSourceFlags, AsRmPtr},
    sound::sound_builder::OwnedPathBuf,
    AsRawRef, Binding, MaResult,
};

pub struct ResourceManagerSource<'a, R: AsRmPtr + ?Sized> {
    inner: *mut sys::ma_resource_manager_data_source,
    _format: PhantomData<R::Format>,
    _marker: PhantomData<&'a R>,
}

impl<'a, R: AsRmPtr + ?Sized> Binding for ResourceManagerSource<'a, R> {
    type Raw = *mut sys::ma_resource_manager_data_source;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a, R: AsRmPtr> SharedSource for ResourceManagerSource<'a, R> {
    type Format = R::Format;
}

#[doc(hidden)]
impl<'a, R: AsRmPtr> AsSourcePtr for ResourceManagerSource<'a, R> {
    type __PtrProvider = private_data_source::ResourceManagerSourceProvider;
}

impl<'a, R: AsRmPtr> ResourceManagerSource<'a, R> {
    pub fn as_source(&'a self) -> DataSourceRef<'a> {
        debug_assert!(!self.to_raw().is_null());
        let ptr = self.to_raw().cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

// Private methods
impl<'a, R: AsRmPtr + ?Sized> ResourceManagerSource<'a, R> {
    fn new_copy_with_config(
        config: &ResourceManagerSourceBuilder<'a, R>,
        existing: &ResourceManagerSource<'a, R>,
    ) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resource_manager_data_source>> =
            Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_data_source_init_copy(
            config.rm,
            existing,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_resource_manager_data_source =
            Box::into_raw(mem) as *mut sys::ma_resource_manager_data_source;
        Ok(Self {
            inner,
            _format: PhantomData,
            _marker: PhantomData,
        })
    }

    fn new_with_config(config: &ResourceManagerSourceBuilder<'a, R>) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resource_manager_data_source>> =
            Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_data_source_init_ex(config.rm, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_resource_manager_data_source =
            Box::into_raw(mem) as *mut sys::ma_resource_manager_data_source;
        Ok(Self {
            inner,
            _format: PhantomData,
            _marker: PhantomData,
        })
    }
}

pub struct ResourceManagerSourceBuilder<'a, R: AsRmPtr + ?Sized> {
    rm: &'a R,
    inner: sys::ma_resource_manager_data_source_config,
    source: SourceBufSource<'a>,
    owned_path: OwnedPathBuf,
}

impl<'a, R: AsRmPtr + ?Sized> AsRawRef for ResourceManagerSourceBuilder<'a, R> {
    type Raw = sys::ma_resource_manager_data_source_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

#[derive(PartialEq)]
pub enum SourceBufSource<'a> {
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
impl<'a, R: AsRmPtr + ?Sized> ResourceManagerSourceBuilder<'a, R> {
    pub fn new(rm: &'a R) -> Self {
        let inner = unsafe { sys::ma_resource_manager_data_source_config_init() };
        Self {
            rm,
            inner,
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
        let null_fields = |cfg: &mut ResourceManagerSourceBuilder<'a, R>| {
            cfg.inner.pFilePath = core::ptr::null();
            cfg.inner.pFilePathW = core::ptr::null();
        };
        match self.source {
            SourceBufSource::None => null_fields(self),
            #[cfg(unix)]
            SourceBufSource::FileUtf8(p) => {
                use crate::sound::sound_builder::OwnedPathBuf;

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

    pub fn build(&mut self) -> MaResult<ResourceManagerSource<'a, R>> {
        self.set_source()?;
        ResourceManagerSource::new_with_config(self)
    }

    pub fn build_copy(
        &mut self,
        existing: &ResourceManagerSource<'a, R>,
    ) -> MaResult<ResourceManagerSource<'a, R>> {
        self.set_source()?;
        ResourceManagerSource::new_copy_with_config(self, existing)
    }
}

impl<'a, R: AsRmPtr + ?Sized> Drop for ResourceManagerSource<'a, R> {
    fn drop(&mut self) {
        let _ = resource_ffi::ma_resource_manager_data_source_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

#[cfg(test)]
mod test {
    use crate::{
        engine::resource::{
            rm_builder::ResourceManagerBuilder, rm_source::ResourceManagerSourceBuilder,
            tiny_test_wav_mono,
        },
        test_assets::temp_file::{unique_tmp_path, TempFileGuard},
    };

    #[test]
    fn test_res_man_data_source_builder_basic_init() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();

        let wav = tiny_test_wav_mono(20);
        let path_guard = TempFileGuard::new(unique_tmp_path("wav"));
        let path = path_guard.path().to_path_buf();
        std::fs::write(&path, &wav).unwrap();

        let _ = ResourceManagerSourceBuilder::new(&rm)
            .file_path(&path)
            .build()
            .unwrap();
    }
}
