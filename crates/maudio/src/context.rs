use std::{cell::Cell, marker::PhantomData};

use maudio_sys::ffi as sys;

use crate::{Binding, LogLevel, Result};

pub mod backend;

pub struct Context {
    inner: *mut sys::ma_context,
    _not_sync: PhantomData<Cell<()>>,
}

#[non_exhaustive]
struct ContextConfig {
    pub log_level: Option<LogLevel>,
}

impl Binding for Context {
    type Raw = *mut sys::ma_context;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            _not_sync: PhantomData,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl Context {
    fn new_internal() -> Result<Self> {
        // let mut mem: Box<std::mem::MaybeUninit<sys::ma_context>> = Box::new_uninit();

        todo!()
    }

    // fn context_init(
    //     c_config: &sys::ma_context_config,
    //     ctx: &mut MaybeUninit<sys::ma_context>,
    // ) -> Result<()> {
    //     let res = unsafe { sys::ma_context_init(std::ptr::null(), 1, c_config, ctx.as_mut_ptr()) };
    //     MaRawResult::resolve(res)
    // }
}

// impl Drop for Context {
//     fn drop(&mut self) {
//         unsafe {
//             sys::ma_context_uninit(self.inner.as_mut_ptr());
//         }
//     }
// }

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn context_works() -> Result<()> {
//         let res = Context::new();
//         assert!(res.is_ok());
//         res.unwrap();
//         Ok(())
//     }
// }

pub(crate) mod context_ffi {
    use maudio_sys::ffi as sys;

    use crate::{MaRawResult, Result};

    pub fn ma_context_init(
        backends: *const sys::ma_backend,
        backend_count: u32,
        config: *const sys::ma_context_config,
        context: *mut sys::ma_context,
    ) -> Result<()> {
        let res = unsafe { sys::ma_context_init(backends, backend_count, config, context) };
        MaRawResult::resolve(res)
    }
}
