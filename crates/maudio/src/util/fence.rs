use maudio_sys::ffi as sys;

use crate::{Binding, MaResult};

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

impl Fence {
    pub fn new() -> MaResult<Fence> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_fence>> = Box::new_uninit();

        fence_ffi::ma_fence_init(mem.as_mut_ptr())?;

        let ptr = unsafe { mem.assume_init() };
        let inner = Box::into_raw(ptr);
        Ok(Fence::from_ptr(inner))
    }

    // TODO: Implement a FenceGuard
    fn acquire(&mut self) -> MaResult<()> {
        fence_ffi::ma_fence_acquire(self)
    }

    fn release(&mut self) -> MaResult<()> {
        fence_ffi::ma_fence_release(self)
    }

    pub fn wait(&mut self) -> MaResult<()> {
        fence_ffi::ma_fence_wait(self)
    }
}

pub(crate) mod fence_ffi {
    use maudio_sys::ffi as sys;

    use crate::{Binding, MaRawResult, MaResult, util::fence::Fence};

    pub fn ma_fence_init(fence: *mut sys::ma_fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_init(fence) };
        MaRawResult::check(res)
    }

    pub fn ma_fence_uninit(fence: &mut Fence) {
        unsafe {
            sys::ma_fence_uninit(fence.to_raw());
        }
    }

    pub fn ma_fence_acquire(fence: &mut Fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_acquire(fence.to_raw()) };
        MaRawResult::check(res)
    }

    pub fn ma_fence_release(fence: &mut Fence) -> MaResult<()> {
        let res = unsafe { sys::ma_fence_release(fence.to_raw()) };
        MaRawResult::check(res)
    }

    pub fn ma_fence_wait(fence: &mut Fence) -> MaResult<()> {
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
