use std::panic::AssertUnwindSafe;

use maudio_sys::ffi as sys;

use crate::{
    backend::{custom_backend::CustomBackend, custom_context::CustomContextInner},
    context::ContextBuilder,
    device::{device_info::DeviceInfo, device_type::DeviceType},
    logging::{LogOwner, LogRef},
    AsRawRef,
};

pub(crate) fn user_backend_callbacks<B: CustomBackend>() -> sys::ma_backend_callbacks {
    sys::ma_backend_callbacks {
        onContextInit: Some(custom_context_on_init::<B>),
        onContextUninit: None,
        onContextEnumerateDevices: Some(custom_context_enumerate_devices::<B>),
        onContextGetDeviceInfo: None,
        onDeviceInit: None,
        onDeviceUninit: None,
        onDeviceStart: None,
        onDeviceStop: None,
        onDeviceRead: None,
        onDeviceWrite: None,
        onDeviceDataLoop: None,
        onDeviceDataLoopWakeup: None,
        onDeviceGetInfo: None,
    }
}

unsafe extern "C" fn custom_context_enumerate_devices<B: CustomBackend>(
    context: *mut sys::ma_context,
    callback: sys::ma_enum_devices_callback_proc,
    user_data: *mut core::ffi::c_void,
) -> sys::ma_result {
    if context.is_null() || user_data.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    let Some(callback) = callback else {
        return sys::ma_result_MA_INVALID_ARGS;
    };

    let custom = unsafe { &mut *context.cast::<CustomContextInner<B>>() };

    let log = custom.log.as_ref().map(|l| LogRef {
        inner: l.inner,
        _owner: LogOwner::Log(l.clone()),
    });

    let Some(mut backend_context) = custom.context.get_mut() else {
        return sys::ma_result_MA_INVALID_OPERATION;
    };

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut stopped = false;

        let mut report = |device_type: DeviceType, info: &DeviceInfo| {
            if stopped {
                return false;
            }

            let keep_going = callback(context, device_type.into(), info.as_raw_ptr(), user_data);

            stopped = keep_going == 0;
            !stopped
        };

        B::enumerate_devices(&mut backend_context, &mut report, log)
    }));

    match res {
        Ok(Ok(())) => sys::ma_result_MA_SUCCESS,
        Ok(Err(error)) => error.ma_result(),
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn custom_context_on_init<B: CustomBackend>(
    context: *mut sys::ma_context,
    config: *const sys::ma_context_config,
    callbacks: *mut sys::ma_backend_callbacks,
) -> sys::ma_result {
    if context.is_null() || config.is_null() || callbacks.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    let custom = unsafe { &mut *context.cast::<CustomContextInner<B>>() };

    let log = custom.log.as_ref().map(|l| LogRef {
        inner: l.inner,
        _owner: LogOwner::Log(l.clone()),
    });

    let config: ContextBuilder = ContextBuilder {
        inner: unsafe { *config },
        backends: custom._backends.take(),
        log: custom.log.clone(),
    };

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| B::init_context(config, log)));

    let backend_context = match res {
        Ok(Ok(context)) => context,
        Ok(Err(e)) => return e.ma_result(),
        Err(_) => return sys::ma_result_MA_ERROR,
    };

    match custom.context.set(backend_context) {
        Ok(_) => sys::ma_result_MA_SUCCESS,
        Err(_) => sys::ma_result_MA_ERROR,
    }
}
