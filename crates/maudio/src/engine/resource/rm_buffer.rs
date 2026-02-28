use std::{marker::PhantomData, mem::MaybeUninit, path::Path};

use maudio_sys::ffi as sys;

use crate::{
    data_source::{private_data_source, AsSourcePtr, DataSourceRef, SharedSource},
    engine::resource::{
        resource_ffi, rm_source::SourceBufSource, rm_source_flags::RmSourceFlags, AsRmPtr,
    },
    sound::sound_builder::OwnedPathBuf,
    AsRawRef, Binding, MaResult,
};

pub struct ResourceManagerBuffer<'a, R: AsRmPtr + ?Sized> {
    inner: *mut sys::ma_resource_manager_data_buffer,
    _format: PhantomData<R::Format>,
    _marker: PhantomData<&'a R>,
}

impl<'a, R: AsRmPtr + ?Sized> Binding for ResourceManagerBuffer<'a, R> {
    type Raw = *mut sys::ma_resource_manager_data_buffer;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a, R: AsRmPtr> SharedSource for ResourceManagerBuffer<'a, R> {
    type Format = R::Format;
}

#[doc(hidden)]
impl<'a, R: AsRmPtr> AsSourcePtr for ResourceManagerBuffer<'a, R> {
    type __PtrProvider = private_data_source::ResourceManagerBufferProvider;
}

impl<'a, R: AsRmPtr> ResourceManagerBuffer<'a, R> {
    pub fn as_source(&'a self) -> DataSourceRef<'a> {
        debug_assert!(!self.to_raw().is_null());
        let ptr = self.to_raw().cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

// private methods
impl<'a, R: AsRmPtr + ?Sized> ResourceManagerBuffer<'a, R> {
    fn new_copy_with_config(
        config: &ResourceManagerBufferBuilder<'a, R>,
        existing: &ResourceManagerBuffer<'a, R>,
    ) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resource_manager_data_buffer>> =
            Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_data_buffer_init_copy(
            config.rm,
            existing,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_resource_manager_data_buffer =
            Box::into_raw(mem) as *mut sys::ma_resource_manager_data_buffer;

        Ok(Self {
            inner,
            _format: PhantomData,
            _marker: PhantomData,
        })
    }

    fn new_with_config(config: &ResourceManagerBufferBuilder<'a, R>) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resource_manager_data_buffer>> =
            Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_data_buffer_init_ex(config.rm, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_resource_manager_data_buffer =
            Box::into_raw(mem) as *mut sys::ma_resource_manager_data_buffer;

        Ok(Self {
            inner,
            _format: PhantomData,
            _marker: PhantomData,
        })
    }
}

impl<'a, R: AsRmPtr + ?Sized> Drop for ResourceManagerBuffer<'a, R> {
    fn drop(&mut self) {
        let _ = resource_ffi::ma_resource_manager_data_buffer_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub struct ResourceManagerBufferBuilder<'a, R: AsRmPtr + ?Sized> {
    rm: &'a R,
    inner: sys::ma_resource_manager_data_source_config,
    source: SourceBufSource<'a>,
    owned_path: OwnedPathBuf,
}

impl<'a, R: AsRmPtr + ?Sized> AsRawRef for ResourceManagerBufferBuilder<'a, R> {
    type Raw = sys::ma_resource_manager_data_source_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl<'a, R: AsRmPtr + ?Sized> ResourceManagerBufferBuilder<'a, R> {
    pub(crate) fn new_internal(
        rm: &'a R,
        inner: sys::ma_resource_manager_data_source_config,
    ) -> Self {
        Self {
            rm,
            inner,
            source: SourceBufSource::None,
            owned_path: OwnedPathBuf::None,
        }
    }

    pub fn new(rm: &'a R) -> Self {
        let inner = unsafe { sys::ma_resource_manager_data_source_config_init() };
        ResourceManagerBufferBuilder::new_internal(rm, inner)
    }

    pub fn flags(&mut self, flags: RmSourceFlags) -> &mut Self {
        self.inner.flags = flags.bits();
        self
    }

    // TODO: Document - This is not the right path for in memory sounds
    // Correct way is to register the sound first
    // This is a convenince method when you don't want to register.
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
        let null_fields = |cfg: &mut ResourceManagerBufferBuilder<'_, R>| {
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

    pub fn build(&mut self) -> MaResult<ResourceManagerBuffer<'a, R>> {
        self.set_source()?;
        ResourceManagerBuffer::<R>::new_with_config(self)
    }

    pub fn build_copy(
        &mut self,
        existing: &ResourceManagerBuffer<'a, R>,
    ) -> MaResult<ResourceManagerBuffer<'a, R>> {
        self.set_source()?;
        ResourceManagerBuffer::<R>::new_copy_with_config(self, existing)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        engine::resource::{
            rm_buffer::ResourceManagerBufferBuilder, rm_builder::ResourceManagerBuilder,
            tiny_test_wav_mono,
        },
        test_assets::temp_file::{unique_tmp_path, TempFileGuard},
    };

    #[test]
    fn test_res_man_data_source_buffer_builder_basic_init() {
        let rm = ResourceManagerBuilder::new().build_u8().unwrap();

        let wav = tiny_test_wav_mono(20);
        let path_guard = TempFileGuard::new(unique_tmp_path("wav"));
        let path = path_guard.path().to_path_buf();
        std::fs::write(&path, &wav).unwrap();

        let _ = ResourceManagerBufferBuilder::new(&rm)
            .file_path(&path)
            .build()
            .unwrap();
    }
}
