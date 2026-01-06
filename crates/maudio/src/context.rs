use std::{
    ffi::{CStr, CString},
    mem::MaybeUninit,
    path::Path,
    pin::Pin,
    sync::atomic::AtomicBool,
};

use maudio_sys::ffi as sys;

use crate::{LogLevel, MaError, MaRawResult, Result};

pub struct Context {
    inner: Pin<Box<MaybeUninit<sys::ma_context>>>,
}

#[non_exhaustive]
struct ContextConfig {
    pub log_level: Option<LogLevel>,
}

impl Context {
    pub fn new() -> Result<Self> {
        let c_config = unsafe { sys::ma_context_config_init() };

        let mut ctx = Box::pin(MaybeUninit::<sys::ma_context>::uninit());

        Context::context_init(&c_config, &mut ctx)?;

        Ok(Self { inner: ctx })
    }

    fn context_init(
        c_config: &sys::ma_context_config,
        ctx: &mut MaybeUninit<sys::ma_context>,
    ) -> Result<()> {
        let res = unsafe { sys::ma_context_init(std::ptr::null(), 1, c_config, ctx.as_mut_ptr()) };
        MaRawResult::resolve(res)
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            sys::ma_context_uninit(self.inner.as_mut_ptr());
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn context_works() -> Result<()> {
        let res = Context::new();
        assert!(res.is_ok());
        res.unwrap();
        Ok(())
    }
}
