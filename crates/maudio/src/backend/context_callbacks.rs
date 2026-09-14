use std::panic::AssertUnwindSafe;

use maudio_sys::ffi as sys;

use crate::{
    backend::{
        custom_backend::CustomBackend,
        custom_context::{CustomContextInner, DeviceDescriptor, UserDeviceConfig},
    },
    device::{
        custom_device::{CustomDeviceInner, UserDevice},
        device_id::DeviceId,
        device_info::DeviceInfo,
        device_type::DeviceType,
    },
    logging::{LogOwner, LogRef},
    AsRawRef, MaResult,
};

pub(crate) fn user_backend_callbacks<B: CustomBackend>() -> sys::ma_backend_callbacks {
    sys::ma_backend_callbacks {
        onContextInit: Some(custom_context_on_init::<B>),
        onContextUninit: None,
        onContextEnumerateDevices: Some(custom_context_enumerate_devices::<B>),
        onContextGetDeviceInfo: Some(custom_context_device_info::<B>),
        onDeviceInit: Some(custom_context_on_device_init::<B>),
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

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| B::init_context(log)));

    let backend_context = match res {
        Ok(Ok(context)) => context,
        Ok(Err(e)) => return e.ma_result(),
        Err(_) => return sys::ma_result_MA_ERROR,
    };

    match custom.user_context.set(backend_context) {
        Ok(_) => sys::ma_result_MA_SUCCESS,
        Err(_) => sys::ma_result_MA_ERROR,
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

    let Some(backend_context) = custom.user_context.get_mut() else {
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

        B::enumerate_devices(backend_context, &mut report, log)
    }));

    match res {
        Ok(Ok(())) => sys::ma_result_MA_SUCCESS,
        Ok(Err(error)) => error.ma_result(),
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn custom_context_device_info<B: CustomBackend>(
    context: *mut sys::ma_context,
    device_type: sys::ma_device_type,
    device_id: *const sys::ma_device_id,
    device_info: *mut sys::ma_device_info,
) -> sys::ma_result {
    if context.is_null() || device_id.is_null() || device_info.is_null() {
        return sys::ma_result_MA_ERROR;
    }

    let Ok(device_type): MaResult<DeviceType> = device_type.try_into() else {
        return sys::ma_result_MA_ERROR;
    };

    let device_id = DeviceId::from_raw(unsafe { &*device_id });

    let custom = unsafe { &mut *context.cast::<CustomContextInner<B>>() };

    let log = custom.log.as_ref().map(|l| LogRef {
        inner: l.inner,
        _owner: LogOwner::Log(l.clone()),
    });

    let Some(backend_context) = custom.user_context.get_mut() else {
        return sys::ma_result_MA_INVALID_OPERATION;
    };

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        B::device_info(backend_context, device_type, device_id, log)
    }));

    let info = match res {
        Ok(Ok(res)) => res,
        Ok(Err(error)) => return error.ma_result(),
        Err(_) => return sys::ma_result_MA_ERROR,
    };

    unsafe {
        device_info.write(info.inner);
    }

    sys::ma_result_MA_SUCCESS
}

unsafe extern "C" fn custom_context_on_device_init<B: CustomBackend>(
    device: *mut sys::ma_device,
    config: *const sys::ma_device_config,
    playback_descriptor: *mut sys::ma_device_descriptor,
    capture_descriptor: *mut sys::ma_device_descriptor,
) -> sys::ma_result {
    if device.is_null()
        || config.is_null()
        || playback_descriptor.is_null()
        || capture_descriptor.is_null()
    {
        return sys::ma_result_MA_ERROR;
    }

    let Ok(config): MaResult<UserDeviceConfig> = unsafe { *config }.try_into() else {
        return sys::ma_result_MA_ERROR;
    };

    let mut playback_descr: Option<DeviceDescriptor> = None;
    let mut capture_descr: Option<DeviceDescriptor> = None;

    if config.device_type == DeviceType::Duplex || config.device_type == DeviceType::Playback {
        if let Ok(play) = unsafe { *playback_descriptor }.try_into() {
            playback_descr = Some(play);
        }
    }
    if config.device_type == DeviceType::Duplex || config.device_type == DeviceType::Capture {
        if let Ok(capt) = unsafe { *capture_descriptor }.try_into() {
            capture_descr = Some(capt);
        }
    }

    let user_device = UserDevice(device.cast::<CustomDeviceInner<B>>());

    let device = unsafe { &*device.cast::<CustomDeviceInner<B>>() };

    // TODO:
    // Update the device descripters after the users updates them.

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        B::device_init(
            user_device,
            config,
            playback_descr.as_mut(),
            capture_descr.as_mut(),
        )
    }));

    let user_device = match res {
        Ok(Ok(device)) => device,
        Ok(Err(error)) => return error.ma_result(),
        Err(_) => return sys::ma_result_MA_ERROR,
    };

    match device.user_device.set(user_device) {
        Ok(_) => sys::ma_result_MA_SUCCESS,
        Err(_) => sys::ma_result_MA_ERROR,
    }
}
