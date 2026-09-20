use std::panic::AssertUnwindSafe;

use maudio_sys::ffi as sys;

use crate::{
    backend::{
        custom_backend::CustomBackend,
        custom_context::{BackendDeviceConfig, CustomContextInner, DeviceDescriptor},
    },
    device::{
        custom_device::BackendDeviceHandle, device_builder::DeviceState, device_id::DeviceId,
        device_info::DeviceInfo, device_type::DeviceType,
    },
    engine::process_cb::CustomBackendState,
    logging::{LogOwner, LogRef},
    AsRawRef, MaResult,
};

pub(crate) fn custom_backend_callbacks<B: CustomBackend>() -> sys::ma_backend_callbacks {
    sys::ma_backend_callbacks {
        onContextInit: Some(custom_context_on_init::<B>),
        onContextUninit: None,
        onContextEnumerateDevices: Some(custom_context_enumerate_devices::<B>),
        onContextGetDeviceInfo: Some(custom_context_device_info::<B>),
        onDeviceInit: Some(custom_context_on_device_init::<B>),
        onDeviceUninit: None,
        onDeviceStart: Some(custom_context_on_device_start::<B>),
        onDeviceStop: Some(custom_context_on_device_stop::<B>),
        onDeviceRead: None,
        onDeviceWrite: None,
        onDeviceDataLoop: None,
        onDeviceDataLoopWakeup: None,
        onDeviceGetInfo: Some(custom_context_device_get_info::<B>),
    }
}

pub(crate) unsafe extern "C" fn custom_context_on_init<B: CustomBackend>(
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

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| B::init_context(log.as_ref())));

    let backend_context = match res {
        Ok(Ok(context)) => context,
        Ok(Err(e)) => return e.ma_result(),
        Err(_) => return sys::ma_result_MA_ERROR,
    };

    match custom.backend_context.set(backend_context) {
        Ok(_) => sys::ma_result_MA_SUCCESS,
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

pub(crate) unsafe extern "C" fn custom_context_enumerate_devices<B: CustomBackend>(
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

    let Some(backend_context) = custom.backend_context.get_mut() else {
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

        B::enumerate_devices(backend_context, &mut report, log.as_ref())
    }));

    match res {
        Ok(Ok(())) => sys::ma_result_MA_SUCCESS,
        Ok(Err(error)) => error.ma_result(),
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

pub(crate) unsafe extern "C" fn custom_context_device_info<B: CustomBackend>(
    context: *mut sys::ma_context,
    device_type: sys::ma_device_type,
    device_id: *const sys::ma_device_id,
    device_info: *mut sys::ma_device_info,
) -> sys::ma_result {
    // device_id may be null if functions like device_get_name
    // are called when onDeviceGetInfo is null, but we delegate
    // that responsibility to onDeviceGetInfo.
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

    let Some(backend_context) = custom.backend_context.get_mut() else {
        return sys::ma_result_MA_INVALID_OPERATION;
    };

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        B::context_query_device_info(backend_context, device_type, device_id, log.as_ref())
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

    let Ok(config): MaResult<BackendDeviceConfig> = unsafe { *config }.try_into() else {
        return sys::ma_result_MA_ERROR;
    };

    let device_ref = unsafe { &*device };
    let user_data_ptr = device_ref.pUserData.cast::<DeviceState>();
    let user_data_ref = unsafe { &*user_data_ptr };

    let Some(state_ref) = user_data_ref.backend_state.as_ref() else {
        return sys::ma_result_MA_ERROR;
    };

    let backend_state_ptr = state_ref.data.cast::<CustomBackendState<B>>();
    let backend_state_ref = unsafe { &*backend_state_ptr };

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

    let custom_context_ref = unsafe { &*device_ref.pContext.cast::<CustomContextInner<B>>() };

    let backend_device: BackendDeviceHandle<B> = BackendDeviceHandle {
        inner: device,
        backend_device: &backend_state_ref.backend_device,
        backend_context: &custom_context_ref.backend_context,
    };

    let log = custom_context_ref.log.as_ref().map(|l| LogRef {
        inner: l.inner,
        _owner: LogOwner::Log(l.clone()),
    });

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        B::device_init(
            backend_device,
            config,
            playback_descr.as_mut(),
            capture_descr.as_mut(),
            log.as_ref(),
        )
    }));

    let backend_device = match res {
        Ok(Ok(device)) => device,
        Ok(Err(error)) => return error.ma_result(),
        Err(_) => return sys::ma_result_MA_ERROR,
    };

    if let Some(play_descr) = playback_descr {
        play_descr.update_raw_descriptor(unsafe { &mut *playback_descriptor });
    }

    if let Some(capt_descr) = capture_descr {
        capt_descr.update_raw_descriptor(unsafe { &mut *capture_descriptor });
    }

    match backend_state_ref.backend_device.set(backend_device) {
        Ok(_) => sys::ma_result_MA_SUCCESS,
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn custom_context_on_device_start<B: CustomBackend>(
    device: *mut sys::ma_device,
) -> sys::ma_result {
    if device.is_null() {
        return sys::ma_result_MA_ERROR;
    }

    let device_ref = unsafe { &*device };
    let user_data_ptr = device_ref.pUserData.cast::<DeviceState>();
    let user_data_ref = unsafe { &*user_data_ptr };

    let Some(state_ref) = user_data_ref.backend_state.as_ref() else {
        return sys::ma_result_MA_ERROR;
    };

    let backend_state_ptr = state_ref.data.cast::<CustomBackendState<B>>();
    let backend_state_ref = unsafe { &*backend_state_ptr };

    let custom_context_ref = unsafe { &*device_ref.pContext.cast::<CustomContextInner<B>>() };

    let backend_device: BackendDeviceHandle<B> = BackendDeviceHandle {
        inner: device,
        backend_device: &backend_state_ref.backend_device,
        backend_context: &custom_context_ref.backend_context,
    };

    let log = custom_context_ref.log.as_ref().map(|l| LogRef {
        inner: l.inner,
        _owner: LogOwner::Log(l.clone()),
    });

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        B::device_start(&backend_device, log.as_ref())
    }));

    match res {
        Ok(Ok(_)) => sys::ma_result_MA_SUCCESS,
        Ok(Err(error)) => error.ma_result(),
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn custom_context_on_device_stop<B: CustomBackend>(
    device: *mut sys::ma_device,
) -> sys::ma_result {
    if device.is_null() {
        return sys::ma_result_MA_ERROR;
    }

    let device_ref = unsafe { &*device };
    let user_data_ptr = device_ref.pUserData.cast::<DeviceState>();
    let user_data_ref = unsafe { &*user_data_ptr };

    let Some(state_ref) = user_data_ref.backend_state.as_ref() else {
        return sys::ma_result_MA_ERROR;
    };

    let backend_state_ptr = state_ref.data.cast::<CustomBackendState<B>>();
    let backend_state_ref = unsafe { &*backend_state_ptr };

    let custom_context_ref = unsafe { &*device_ref.pContext.cast::<CustomContextInner<B>>() };

    let backend_device: BackendDeviceHandle<B> = BackendDeviceHandle {
        inner: device,
        backend_device: &backend_state_ref.backend_device,
        backend_context: &custom_context_ref.backend_context,
    };

    let log = custom_context_ref.log.as_ref().map(|l| LogRef {
        inner: l.inner,
        _owner: LogOwner::Log(l.clone()),
    });

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        B::device_stop(&backend_device, log.as_ref())
    }));

    match res {
        Ok(Ok(_)) => sys::ma_result_MA_SUCCESS,
        Ok(Err(error)) => error.ma_result(),
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn custom_context_device_get_info<B: CustomBackend>(
    device: *mut sys::ma_device,
    device_type: sys::ma_device_type,
    device_info: *mut sys::ma_device_info,
) -> sys::ma_result {
    if device.is_null() {
        return sys::ma_result_MA_ERROR;
    }

    let device_ref = unsafe { &*device };
    let user_data_ptr = device_ref.pUserData.cast::<DeviceState>();
    let user_data_ref = unsafe { &*user_data_ptr };

    let Some(state_ref) = user_data_ref.backend_state.as_ref() else {
        return sys::ma_result_MA_ERROR;
    };

    let backend_state_ptr = state_ref.data.cast::<CustomBackendState<B>>();
    let backend_state_ref = unsafe { &*backend_state_ptr };

    let custom_context_ref = unsafe { &*device_ref.pContext.cast::<CustomContextInner<B>>() };

    let backend_device: BackendDeviceHandle<B> = BackendDeviceHandle {
        inner: device,
        backend_device: &backend_state_ref.backend_device,
        backend_context: &custom_context_ref.backend_context,
    };

    let log = custom_context_ref.log.as_ref().map(|l| LogRef {
        inner: l.inner,
        _owner: LogOwner::Log(l.clone()),
    });

    let Some(context) = custom_context_ref.backend_context.get() else {
        return sys::ma_result_MA_INVALID_ARGS;
    };

    let Ok(device_type) = device_type.try_into() else {
        return sys::ma_result_MA_INVALID_ARGS;
    };

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        B::device_get_info(backend_device, context, device_type, log.as_ref())
    }));

    let info = match res {
        Ok(Ok(info)) => info,
        Ok(Err(error)) => return error.ma_result(),
        Err(_) => return sys::ma_result_MA_ERROR,
    };

    device_info.write(info.inner);

    sys::ma_result_MA_SUCCESS
}
