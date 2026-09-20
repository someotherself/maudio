#![allow(unused)]
use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::MaybeUninit,
    sync::{atomic::AtomicBool, Arc, OnceLock},
};

use maudio_sys::ffi as sys;

use crate::{
    backend::{
        custom_backend::CustomBackend,
        custom_context::{CustomContext, CustomContextInner},
    },
    device::{
        device_builder::{private_device_b, AsDeviceBuilder},
        device_ffi,
        device_id::DeviceId,
        private_device, AsDevicePtr,
    },
    logging::{LogInner, StoredLogs},
    pcm_frames::PcmFormat,
    util::{device_notif::DeviceStateNotifier, proc_notif::ProcFramesNotif},
    Binding, MaResult,
};

pub struct BackendDeviceHandle<'device, B: CustomBackend> {
    pub(crate) inner: *mut sys::ma_device,
    pub(crate) backend_device: &'device OnceLock<B::Device<'device>>,
    pub(crate) backend_context: &'device OnceLock<B::Context>,
}

impl<'device, B: CustomBackend> Clone for BackendDeviceHandle<'device, B> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner,
            backend_device: self.backend_device,
            backend_context: self.backend_context,
        }
    }
}

impl<'device, B: CustomBackend> Binding for BackendDeviceHandle<'device, B> {
    type Raw = *mut sys::ma_device;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

unsafe impl<'device, B: CustomBackend> Send for BackendDeviceHandle<'device, B> {}

impl<'device, B: CustomBackend> BackendDeviceHandle<'device, B> {
    pub fn backend_device(&self) -> Option<&B::Device<'device>> {
        self.backend_device.get()
    }

    pub fn handle_backend_data_callback<F: PcmFormat, R: PcmFormat>(
        &self,
        output: Option<&mut [F::StorageUnit]>,
        input: Option<&[R::StorageUnit]>,
    ) -> MaResult<()> {
        device_ffi::ma_device_handle_backend_data_callback::<F, R>(self.to_raw(), output, input)
    }

    pub fn backend_context(&self) -> &B::Context {
        self.backend_context.get().unwrap()
    }
}
