use std::{marker::PhantomData, mem::MaybeUninit, path::Path, pin::Pin};

use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaRawResult, Result, engine::Engine};

pub enum SoundError {}

impl From<SoundError> for ErrorKinds {
    fn from(e: SoundError) -> Self {
        ErrorKinds::Sound(e)
    }
}

pub struct Sound<'a> {
    inner: Pin<Box<MaybeUninit<sys::ma_sound>>>,
    /// Marks if inner is initialized
    init: bool,
    /// Sound should not outlive Engine
    _engine: PhantomData<&'a Engine>,
}

impl<'a> Sound<'a> {
    pub fn play_sound(&mut self) -> Result<()> {
        let res = unsafe { sys::ma_sound_start(self.assume_init_mut_ptr()) };
        MaRawResult::resolve(res)?;
        Ok(())
    }

    pub(crate) fn new_uninit() -> Self {
        let inner = Box::pin(MaybeUninit::<sys::ma_sound>::uninit());
        Self {
            inner,
            init: false,
            _engine: PhantomData,
        }
    }

    pub(crate) fn set_init(&mut self) {
        self.init = true;
    }

    /// Gets a pointer to an UNINITIALIZED `MaybeUninit<sys::ma_sound>`
    pub(crate) fn maybe_uninit_mut_ptr(&mut self) -> *mut sys::ma_sound {
        self.inner.as_mut_ptr()
    }

    /// Gets a pointer to an initialized `MaybeUninit<sys::ma_sound>`
    pub(crate) fn assume_init_mut_ptr(&mut self) -> *mut sys::ma_sound {
        debug_assert!(self.init, "Sound used before initialization.");
        unsafe { self.inner.as_mut().get_mut().assume_init_mut() }
    }
}

impl<'a> Drop for Sound<'a> {
    fn drop(&mut self) {
        if !self.init {
            return;
        }
        unsafe {
            sys::ma_sound_uninit(self.assume_init_mut_ptr());
        }
    }
}

// rename to SoundBuilder and create a builder pattern
pub struct SoundConfig {
    inner: sys::ma_sound_config,
}

impl SoundConfig {
    //     pub fn from_file<'a>(engine: &'a mut Engine, path: &Path) -> Result<Sound<'a>> {
    //         todo!()
    //     }
}

impl SoundConfig {
    pub(crate) fn new(config: sys::ma_sound_config) -> Self {
        Self { inner: config }
    }

    pub(crate) fn get_raw(&self) -> *const sys::ma_sound_config {
        &self.inner as *const _
    }
}
