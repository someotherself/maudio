use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    sync::{Arc, OnceLock},
};

use maudio_sys::ffi as sys;

use crate::{
    backend::{custom_backend::CustomBackend, Backend},
    context::{context_ffi, private_context, AsContextPtr, ContextBuilder},
    logging::LogInner,
    AsRawRef, MaResult,
};

pub struct CustomContext<B: CustomBackend> {
    inner: *mut CustomContextInner<B>,
}

#[repr(C)]
pub(crate) struct CustomContextInner<B: CustomBackend> {
    pub(crate) inner: sys::ma_context,
    pub(crate) context: OnceLock<B::Context>,
    pub(super) log: Option<Arc<LogInner>>,
    pub(super) _backends: Option<Box<[Backend]>>,
    pub(crate) backend: PhantomData<B>,
}

impl<B: CustomBackend> AsRawRef for CustomContext<B> {
    type Raw = sys::ma_context;

    fn as_raw(&self) -> &Self::Raw {
        unsafe { &(*self.inner).inner }
    }
}

impl<B: CustomBackend> AsContextPtr for CustomContext<B> {
    type __PtrProvider = private_context::CustomContextProvider;
}

// Private methods
impl<B: CustomBackend> CustomContext<B> {
    pub(crate) fn new_with_config(config: &mut ContextBuilder) -> MaResult<Self> {
        let mut inner = Box::new(CustomContextInner {
            inner: unsafe { MaybeUninit::zeroed().assume_init() },
            context: OnceLock::new(),
            log: config.log.take(),
            _backends: config.backends.clone(),
            backend: PhantomData,
        });

        let base_ptr = core::ptr::addr_of_mut!(inner.inner);

        context_ffi::ma_context_init(config.backends.as_deref(), config, base_ptr)?;

        let inner_ptr = Box::into_raw(inner);

        debug_assert_eq!(
            unsafe { core::ptr::addr_of_mut!((*inner_ptr).inner) }.cast::<u8>(),
            inner_ptr.cast::<u8>(),
        );

        Ok(CustomContext { inner: inner_ptr })
    }
}

impl<B: CustomBackend> Drop for CustomContext<B> {
    fn drop(&mut self) {
        let _ = context_ffi::ma_context_uninit(self.as_raw_ptr() as *mut _);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
