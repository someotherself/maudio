use std::{ffi::CString, mem::MaybeUninit, path::Path, pin::Pin};

use crate::{
    ErrorKinds, LogLevel, MaError, MaRawResult, Result,
    sound::{Sound, sound_builder::SoundBuilder, sound_flags::SoundFlags},
};

use maudio_sys::ffi as sys;

pub enum EngineError {}

impl From<EngineError> for ErrorKinds {
    fn from(e: EngineError) -> Self {
        ErrorKinds::Engine(e)
    }
}

pub struct Engine {
    inner: Pin<Box<MaybeUninit<sys::ma_engine>>>,
    /// Marks if inner is initialized
    init: bool,
}

impl Engine {
    pub fn new() -> Result<Self> {
        Self::new_with_config(None)
    }

    pub fn with_config(config: EngineConfig) -> Result<Self> {
        Self::new_with_config(Some(&config))
    }

    fn new_with_config(config: Option<&EngineConfig>) -> Result<Self> {
        let inner: Pin<Box<MaybeUninit<sys::ma_engine>>> = Box::pin(MaybeUninit::zeroed());
        let mut engine = Self { inner, init: false };
        Engine::init(config, engine.maybe_uninit_mut_ptr())?;
        engine.set_init();
        Ok(engine)
    }

    pub fn new_sound(&mut self) -> Result<Sound<'_>> {
        let mut sound = Sound::new_uninit(SoundFlags::NONE);
        let config = SoundBuilder::init(self.assume_init_mut_ptr());
        // let config = self.new_sound_config();
        let res = unsafe {
            sys::ma_sound_init_ex(
                self.assume_init_mut_ptr(),
                config.get_raw(),
                sound.maybe_uninit_mut_ptr(),
            )
        };
        MaRawResult::resolve(res)?;
        sound.set_init();
        Ok(sound)
    }

    pub fn new_sound_with_config(&mut self, config: SoundBuilder) -> Result<Sound<'_>> {
        let mut sound = Sound::new_uninit(SoundFlags::NONE);
        let res = unsafe {
            sys::ma_sound_init_ex(
                self.assume_init_mut_ptr(),
                config.get_raw(),
                sound.maybe_uninit_mut_ptr(),
            )
        };
        MaRawResult::resolve(res)?;
        sound.set_init();
        Ok(sound)
    }

    // TODO Compare with miniaudio API - should flags be a param?
    // Or leave as convenience methods and create different API?
    pub fn new_sound_from_file_with_flags(
        &mut self,
        path: &Path,
        flags: SoundFlags,
    ) -> Result<Sound<'_>> {
        let mut sound = Sound::new_uninit(flags);
        self.init_sound_from_file_raw(path, &mut sound)?;
        sound.set_init();
        Ok(sound)
    }

    pub fn new_sound_from_file(&mut self, path: &Path) -> Result<Sound<'_>> {
        let mut sound = Sound::new_uninit(SoundFlags::NONE);
        self.init_sound_from_file_raw(path, &mut sound)?;
        sound.set_init();
        Ok(sound)
    }

    fn init_sound_from_file_raw(&mut self, path: &Path, sound: &mut Sound) -> Result<()> {
        let p_group: *mut sys::ma_sound = core::ptr::null_mut();
        let p_done_fence: *mut sys::ma_fence = core::ptr::null_mut();
        #[cfg(unix)]
        {
            let c_path = cstring_from_path(path)?;
            let res = unsafe {
                sys::ma_sound_init_from_file(
                    self.assume_init_mut_ptr(),
                    c_path.as_ptr(),
                    sound.flag_bits(),
                    p_group,      // TODO
                    p_done_fence, // TODO
                    sound.maybe_uninit_mut_ptr(),
                )
            };
            MaRawResult::resolve(res)?;
            Ok(())
        }
        #[cfg(windows)]
        {
            let c_path = wide_null_terminated(&path);
            let res = unsafe {
                sys::ma_sound_init_from_file_w(
                    self.assume_init_mut_ptr(),
                    c_path.as_ptr(),
                    sound.flag_bits(),
                    p_group,      // TODO
                    p_done_fence, // TODO
                    sound.maybe_uninit_mut_ptr(),
                )
            };
            MaRawResult::resolve(res)?;
            return Ok(());
        }

        // TODO. What other platforms can be added
        #[cfg(not(any(unix, windows)))]
        compile_error!("init_sound_from_file is only supported on unix and windows");
    }

    // pub fn get_device(&mut self) {
    //     let res = unsafe { sys::ma_engine_get_device(self.assume_init_mut_ptr()) };
    // }
}

impl Engine {
    fn init(config: Option<&EngineConfig>, engine: *mut sys::ma_engine) -> Result<()> {
        let p_config: *const sys::ma_engine_config =
            config.map_or(core::ptr::null(), |c| &c.inner as *const _);
        let res = unsafe { sys::ma_engine_init(p_config, engine) };
        MaRawResult::resolve(res)
    }

    pub(crate) fn set_init(&mut self) {
        self.init = true;
    }

    /// Gets a pointer to an UNINITIALIZED `MaybeUninit<sys::ma_engine>`
    pub(crate) fn maybe_uninit_mut_ptr(&mut self) -> *mut sys::ma_engine {
        self.inner.as_mut_ptr()
    }

    /// Gets a pointer to an initialized `MaybeUninit<sys::ma_engine>`
    pub(crate) fn assume_init_mut_ptr(&mut self) -> *mut sys::ma_engine {
        debug_assert!(self.init, "Engine used before initialization.");
        unsafe { self.inner.as_mut().get_mut().assume_init_mut() }
    }

    /// Use carefully. Some functions (like ma_sound_config_init_2) require `*mut sys::ma_engine` (bindgen generated) even though they don't mutate it.
    pub(crate) unsafe fn as_mut_ptr_from_ref(&self) -> *mut sys::ma_engine {
        self.inner.as_ptr() as *mut sys::ma_engine
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if !self.init {
            return;
        }
        unsafe {
            sys::ma_engine_uninit(self.assume_init_mut_ptr());
        }
    }
}

#[cfg(unix)]
pub(crate) fn cstring_from_path(path: &Path) -> Result<CString> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(path.as_os_str().as_bytes()).map_err(|_| MaError(sys::ma_result_MA_INVALID_ARGS))
}

#[cfg(windows)]
pub(crate) fn wide_null_terminated(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    // UTF-16 + trailing NUL
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub struct EngineConfig {
    inner: sys::ma_engine_config,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineConfig {
    pub fn new() -> Self {
        Self {
            inner: unsafe { sys::ma_engine_config_init() },
        }
    }

    fn get_raw(&mut self) -> &mut sys::ma_engine_config {
        &mut self.inner
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use super::*;

    #[test]
    #[cfg(feature = "device-tests")]
    fn engine_works_with_default() {
        let _engine = Engine::new().unwrap();
    }

    #[test]
    #[cfg(feature = "device-tests")]
    fn engine_works_with_cfg() {
        let config = EngineConfig::new();
        let _engine = Engine::with_config(config).unwrap();
    }

    #[test]
    #[cfg(feature = "device-tests")]
    fn init_engine_and_sound() {
        let mut engine = Engine::new().unwrap();
        let _sound = engine.new_sound().unwrap();
    }

    #[test]
    #[cfg(feature = "device-tests")]
    fn init_engine_and_sound_with_config() {
        // TODO: Which config needs to be consumed?
        let config = EngineConfig::new();
        let mut engine = Engine::new_with_config(Some(&config)).unwrap();
        let s_config = engine.new_sound_config();
        let _sound = engine.new_sound_with_config(s_config).unwrap();
    }

    #[test]
    #[cfg(feature = "device-tests")]
    fn init_sound_from_path() {
        let mut engine = Engine::new().unwrap();
        let path = Path::new("tests/assets/sample.mp3");
        let mut sound = engine.new_sound_from_file(path).unwrap();
        sound.play_sound().unwrap();
    }
}
