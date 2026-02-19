use std::{cell::Cell, marker::PhantomData};

use maudio_sys::ffi as sys;

use crate::{context::backend::Backend, device::device_builder::DeviceBuilder, Binding, MaResult};

pub mod device_builder;

pub struct Device {
    inner: *mut sys::ma_device,
    // user_data: *mut DeviceUserData,
    _not_sync: PhantomData<Cell<()>>,
}

impl Binding for Device {
    type Raw = *mut sys::ma_device;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl Device {
    fn init_default(_config: &DeviceBuilder) -> MaResult<Self> {
        // TODO: Used by DeviceBuilder
        todo!()
    }

    fn init_from_config_internal(_backends: &[Backend], _config: Option<&DeviceBuilder>) -> Self {
        todo!()
    }
}

pub(crate) mod device_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        context::{backend::Backend, Context},
        device::device_builder::DeviceBuilder,
        AsRawRef, Binding, MaResult, MaudioError,
    };

    pub fn ma_device_init(
        context: &mut Context,
        config: &DeviceBuilder,
        device: *mut sys::ma_device,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_device_init(context.to_raw(), config.as_raw_ptr(), device) };
        MaudioError::check(res)
    }

    pub fn ma_device_init_ex(
        backends: &[Backend],
        context_cfg: *const sys::ma_context_config,
        config: *const sys::ma_device_config,
        device: *mut sys::ma_device,
    ) -> MaResult<()> {
        let (backends_ptr, length): (*const sys::ma_backend, u32) = if !backends.is_empty() {
            (backends.as_ptr() as *const _, backends.len() as u32)
        } else {
            (core::ptr::null(), 0)
        };
        let res =
            unsafe { sys::ma_device_init_ex(backends_ptr, length, context_cfg, config, device) };
        MaudioError::check(res)
    }

    pub fn ma_device_uninit(device: *mut sys::ma_device) {
        // TODO
        unsafe { sys::ma_device_uninit(device) };
    }
}
