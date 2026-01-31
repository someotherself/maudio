//! A synchronization primitive used during sound initialization from a path.
//!
//! `Fence` is a small RAII wrapper around `ma_fence` and is used internally
//! to coordinate asynchronous work performed by miniaudio when creating a `Sound` from a file path.
//!
//! In this context, the fence allows one thread to block until background
//! decoding or initialization work has completed.
//!
//! ## Example:
//! ```no_run
//! # use maudio::engine::Engine;
//! # use std::path::Path;
//! # use maudio::util::fence::Fence;
//! # use maudio::sound::sound_flags::SoundFlags;
//! # fn new_sound(path: &Path) -> maudio::MaResult<()> {
//! let engine = Engine::new()?;
//! let fence = Fence::new()?;
//! let mut sound = engine.new_sound_from_file_with_flags(path, SoundFlags::ASYNC, Some(&fence))?;
//! // Block until the sound has finished loading
//! fence.wait()?;
//! # sound.play_sound()?;
//! # Ok(())
//! # }
//! ```
use std::mem::MaybeUninit;

use maudio_sys::ffi as sys;

use crate::{Binding, MaResult};

/// An owned fence used to synchronize sound initialization.
///
/// `Fence` can be used internally when creating a `Sound` from a file path with
/// asynchronous loading enabled.
///
/// In that mode, miniaudio can return from initialization before the sound is
/// fully ready, and a fence can be used to block until loading/decoding has completed.
pub struct Fence {
    inner: *mut sys::ma_fence,
}

impl Binding for Fence {
    type Raw = *mut sys::ma_fence;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self { inner: raw }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

/// A scoped guard representing an acquired fence.
///
/// When all acquired `FenceGuard` instances are dropped,
/// the associated fence is automatically released.
#[must_use]
pub struct FenceGuard<'a> {
    inner: &'a Fence,
    active: bool,
}

impl Fence {
    /// Creates a new fence.
    pub fn new() -> MaResult<Fence> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_fence>> = Box::new(MaybeUninit::uninit());

        fence_ffi::ma_fence_init(mem.as_mut_ptr())?;

        let ptr = unsafe { mem.assume_init() };
        let inner = Box::into_raw(ptr);
        Ok(Fence::from_ptr(inner))
    }

    /// Acquires the fence and returns a guard.
    ///
    /// While the returned [`FenceGuard`] is alive, the fence remains acquired.
    /// When the guard is dropped, the fence is automatically released.
    pub fn acquire(&self) -> MaResult<FenceGuard<'_>> {
        fence_ffi::ma_fence_acquire(self)?;
        Ok(FenceGuard {
            inner: self,
            active: true,
        })
    }

    fn release(&self) -> MaResult<()> {
        fence_ffi::ma_fence_release(self)
    }

    /// Blocks the current thread until the fence is released.
    ///
    /// This call will block until another thread releases the fence.
    /// It does not spin or poll.
    pub fn wait(&self) -> MaResult<()> {
        fence_ffi::ma_fence_wait(self)
    }
}

pub(crate) mod fence_ffi {
    use maudio_sys::ffi as sys;

    use crate::{util::fence::Fence, Binding, MaRawResult, MaResult};

    pub fn ma_fence_init(fence: *mut sys::ma_fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_init(fence) };
        MaRawResult::check(res)
    }

    pub fn ma_fence_uninit(fence: &Fence) {
        unsafe {
            sys::ma_fence_uninit(fence.to_raw());
        }
    }

    pub fn ma_fence_acquire(fence: &Fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_acquire(fence.to_raw()) };
        MaRawResult::check(res)
    }

    pub fn ma_fence_release(fence: &Fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_release(fence.to_raw()) };
        MaRawResult::check(res)
    }

    pub fn ma_fence_wait(fence: &Fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_wait(fence.to_raw()) };
        MaRawResult::check(res)
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        fence_ffi::ma_fence_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

impl Drop for FenceGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.inner.release();
        }
    }
}
