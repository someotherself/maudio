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
use std::{mem::MaybeUninit, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{Binding, MaResult};

/// An owned fence used to synchronize sound initialization.
///
/// `Fence` can be used internally when creating certain types (like `Sound`) from a file path with
/// asynchronous loading enabled.
///
/// The `Fence` is fully threadsafe and cheap to clone
///
/// In that mode, miniaudio can return from initialization before the sound is
/// fully ready, and a fence can be used to block until loading/decoding has completed.
#[derive(Clone)]
pub struct Fence {
    inner: Arc<FenceInner>,
}

unsafe impl Send for FenceInner {}
unsafe impl Sync for FenceInner {}

struct FenceInner {
    inner: *mut sys::ma_fence,
}

impl Binding for Fence {
    type Raw = *mut sys::ma_fence;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: Arc::new(FenceInner { inner: raw }),
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner.inner
    }
}

/// A scoped guard representing an acquired fence.
///
/// When all acquired `FenceGuard` instances are dropped,
/// the associated fence is automatically released.
#[must_use]
pub struct FenceGuard {
    inner: Fence,
    active: bool,
}

impl Fence {
    /// Creates a new fence.
    pub fn new() -> MaResult<Fence> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_fence>> = Box::new(MaybeUninit::uninit());

        fence_ffi::ma_fence_init(mem.as_mut_ptr())?;

        let inner: *mut sys::ma_fence = Box::into_raw(mem) as *mut sys::ma_fence;
        Ok(Fence::from_ptr(inner))
    }

    /// Manually acquires the fence and returns a guard.
    ///
    /// While the returned [`FenceGuard`] is alive, the fence remains acquired
    /// and calls to [`Fence::wait()`] will block until all FenceGuards are dropped.
    ///
    /// This is useful when you want to keep a fence acquired for the
    /// duration of a scope or custom operation.
    ///
    /// When a fence is attached to asynchronous sound initialization,
    /// you usually do not need to call this yourself because the sound
    /// manages the fence internally.
    pub fn acquire(&self) -> MaResult<FenceGuard> {
        fence_ffi::ma_fence_acquire(self.clone())?;
        Ok(FenceGuard {
            inner: self.clone(),
            active: true,
        })
    }

    fn release(&self) -> MaResult<()> {
        fence_ffi::ma_fence_release(self.clone())
    }

    /// Blocks the current thread until the fence is released.
    ///
    /// This is a blocking wait using OS primitives (no busy-spinning).
    pub fn wait(&self) -> MaResult<()> {
        fence_ffi::ma_fence_wait(self.clone())
    }
}

pub(crate) mod fence_ffi {
    use maudio_sys::ffi as sys;

    use crate::{util::fence::Fence, Binding, MaResult, MaudioError};

    pub fn ma_fence_init(fence: *mut sys::ma_fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_init(fence) };
        MaudioError::check(res)
    }

    pub fn ma_fence_uninit(fence: Fence) {
        unsafe {
            sys::ma_fence_uninit(fence.to_raw());
        }
    }

    pub fn ma_fence_acquire(fence: Fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_acquire(fence.to_raw()) };
        MaudioError::check(res)
    }

    pub fn ma_fence_release(fence: Fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_release(fence.to_raw()) };
        MaudioError::check(res)
    }

    pub fn ma_fence_wait(fence: Fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_wait(fence.to_raw()) };
        MaudioError::check(res)
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        fence_ffi::ma_fence_uninit(self.clone());
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

impl Drop for FenceGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = self.inner.release();
        }
    }
}
