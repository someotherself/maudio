use std::{marker::PhantomData, mem::MaybeUninit, path::Path, pin::Pin};

use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaRawResult, Result, engine::Engine, sound::sound_flags::SoundFlags};

pub mod data_source;
pub mod sound_builder;
pub mod sound_flags;
pub mod sound_group;

pub enum SoundError {}

impl From<SoundError> for ErrorKinds {
    fn from(e: SoundError) -> Self {
        ErrorKinds::Sound(e)
    }
}

pub struct Sound<'a> {
    inner: Pin<Box<MaybeUninit<sys::ma_sound>>>,
    flags: SoundFlags,
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

    pub fn stop_sound(&mut self) -> Result<()> {
        let res = unsafe { sys::ma_sound_stop(self.assume_init_mut_ptr()) };
        MaRawResult::resolve(res)?;
        Ok(())
    }

    /// Sets the playback volume as a linear gain multiplier.
    ///
    /// A value of `1.0` plays the sound at its original volume.
    /// Values between `0.0` and `1.0` reduce the volume, while values greater
    /// than `1.0` amplify the signal and may cause clipping.
    ///
    /// A value of `0.0` silences the sound.
    ///
    /// If you prefer decibel volume control you can use the helper `sound_volume_db_to_linear`
    /// or use `Sound::set_volume_db()`
    pub fn set_volume(&mut self, volume: f32) {
        unsafe { sys::ma_sound_set_volume(self.assume_init_mut_ptr(), volume) };
    }

    /// Sets the playback volume using a gain value expressed in decibels (dB).
    ///
    /// This is a convenience wrapper around [`Sound::set_volume`] that converts
    /// the provided decibel value to a linear gain factor internally.
    pub fn set_volume_db(&mut self, volume_db: f32) {
        let volume_linear = sound_volume_db_to_linear(volume_db);
        unsafe { sys::ma_sound_set_volume(self.assume_init_mut_ptr(), volume_linear) };
    }

    /// Does not return an error.
    ///
    /// Also returns `0.0` if `Sound` if not initialized
    pub fn get_volume(&self) -> f32 {
        unsafe { sys::ma_sound_get_volume(self.assume_init_ptr()) }
    }

    /// Sets the playback pitch of the sound as a multiplier.
    ///
    /// A value of `1.0` plays the sound at its original pitch and speed.
    /// Values greater than `1.0` raise the pitch and increase playback speed,
    /// while values between `0.0` and `1.0` lower the pitch and slow playback.
    ///
    /// If the sound was created with [`SoundFlags::NO_PITCH`], this method has no effect.
    pub fn set_pitch(&mut self, pitch: f32) {
        if self.flags.contains(SoundFlags::NO_PITCH) {
            return;
        }
        unsafe { sys::ma_sound_set_pitch(self.assume_init_mut_ptr(), pitch) };
    }

    /// Does not return an error.
    ///
    /// Also returns `0.0` if `Sound` if not initialized.
    ///
    /// If the sound was created with [`SoundFlags::NO_PITCH`], this method has no effect and returns `0.0`.
    pub fn get_pitch(&self) -> f32 {
        if self.flags.contains(SoundFlags::NO_PITCH) {
            return 0.0;
        }
        unsafe { sys::ma_sound_get_pitch(self.assume_init_ptr()) }
    }

    pub fn set_pan(&mut self, pan: f32) {
        unsafe { sys::ma_sound_set_pan(self.assume_init_mut_ptr(), pan) };
    }

    /// Set sound looping on or off. Same as `SoundFlags::LOOPING`
    pub fn set_looping(&mut self, looping: bool) {
        let looping = looping as u32;
        unsafe { sys::ma_sound_set_looping(self.assume_init_mut_ptr(), looping) };
    }

    /// Check if a sound is set to loop
    pub fn is_looping(&self) -> bool {
        let res = unsafe { sys::ma_sound_is_looping(self.assume_init_ptr()) };
        res == 1
    }

    /// Check if a sound is playing
    pub fn sound_is_playing(&self) -> bool {
        let res = unsafe { sys::ma_sound_is_playing(self.assume_init_ptr()) };
        res == 11
    }

    /// Check if a sound has finished playing
    pub fn sound_at_end(&self) -> bool {
        let res = unsafe { sys::ma_sound_at_end(self.assume_init_ptr()) };
        res == 1
    }

    /// Does not return an error.
    ///
    /// Also returns `0.0` if `Sound` if not initialized
    pub fn get_pan(&self) -> f32 {
        unsafe { sys::ma_sound_get_pan(self.assume_init_ptr()) }
    }
}

/// Converts a gain value expressed in decibels (dB) to a linear volume factor.
///
/// A value of `0.0` dB corresponds to a linear factor of `1.0` (no change),
/// negative values reduce the volume, and positive values increase it.
///
/// The returned value can be passed directly to [`Sound::set_volume`].
pub fn sound_volume_db_to_linear(gain: f32) -> f32 {
    unsafe { sys::ma_volume_db_to_linear(gain) }
}

/// Converts a linear volume factor to a gain value expressed in decibels (dB).
///
/// A factor of `1.0` corresponds to `0.0` dB, values less than `1.0` produce
/// negative decibel values, and values greater than `1.0` produce positive ones.
///
/// This is useful for inspecting or serializing the current volume in dB.
pub fn sound_volume_linear_to_db(factor: f32) -> f32 {
    unsafe { sys::ma_volume_linear_to_db(factor) }
}

impl<'a> Sound<'a> {
    pub(crate) fn new_uninit(flags: SoundFlags) -> Self {
        let inner = Box::pin(MaybeUninit::<sys::ma_sound>::uninit());
        Self {
            inner,
            flags,
            init: false,
            _engine: PhantomData,
        }
    }

    pub(crate) fn set_init(&mut self) {
        self.init = true;
    }

    pub(crate) fn flag_bits(&self) -> u32 {
        self.flags.bits()
    }

    /// Gets a pointer to an initialized `MaybeUninit<sys::ma_sound>`
    pub(crate) fn assume_init_ptr(&self) -> *const sys::ma_sound {
        debug_assert!(self.init, "Sound used before initialization.");
        self.inner.as_ptr()
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
