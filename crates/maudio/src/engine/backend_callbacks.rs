use std::panic::AssertUnwindSafe;

use crate::{
    backend::{
        context_callbacks::{
            custom_context_device_info, custom_context_enumerate_devices, custom_context_on_init,
        },
        custom_backend::CustomBackend,
        custom_context::{BackendDeviceConfig, CustomContextInner, DeviceDescriptor},
    },
    device::{custom_device::BackendDeviceHandle, device_type::DeviceType},
    engine::{
        process_cb::{CustomBackendState, EngineUserData},
        EngineInner,
    },
    logging::{LogOwner, LogRef},
    MaResult,
};

use maudio_sys::ffi as sys;

pub(crate) fn engine_custom_backend_callbacks<B: CustomBackend>() -> sys::ma_backend_callbacks {
    sys::ma_backend_callbacks {
        onContextInit: Some(custom_context_on_init::<B>),
        onContextUninit: None,
        onContextEnumerateDevices: Some(custom_context_enumerate_devices::<B>),
        onContextGetDeviceInfo: Some(custom_context_device_info::<B>),
        onDeviceInit: Some(engine_custom_context_on_device_init::<B>),
        onDeviceUninit: None,
        onDeviceStart: Some(engine_custom_context_on_device_start::<B>),
        onDeviceStop: Some(engine_custom_context_on_device_stop::<B>),
        onDeviceRead: None,
        onDeviceWrite: None,
        onDeviceDataLoop: None,
        onDeviceDataLoopWakeup: None,
        onDeviceGetInfo: None,
    }
}

unsafe extern "C" fn engine_custom_context_on_device_init<B: CustomBackend>(
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
    let engine_ptr = device_ref.pUserData.cast::<sys::ma_engine>();
    let engine_user_data = unsafe { &*engine_ptr }
        .pProcessUserData
        .cast::<EngineUserData>();
    let engine_user_data_ref = unsafe { &*engine_user_data };

    let state_lock = &engine_user_data_ref.backend_state.lock().unwrap();
    let Some(state_ref) = state_lock.as_ref() else {
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

    let engine_inner = unsafe { &*engine_ptr.cast::<EngineInner>() };

    let log = engine_inner._logger.as_ref().map(|l| LogRef {
        inner: l.inner,
        _owner: LogOwner::Log(l.clone()),
    });

    let custom_context_ref = unsafe { &*device_ref.pContext.cast::<CustomContextInner<B>>() };

    let backend_device: BackendDeviceHandle<B> = BackendDeviceHandle {
        inner: device,
        backend_device: &backend_state_ref.backend_device,
        backend_context: &custom_context_ref.backend_context,
    };

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

unsafe extern "C" fn engine_custom_context_on_device_start<B: CustomBackend>(
    device: *mut sys::ma_device,
) -> sys::ma_result {
    if device.is_null() {
        return sys::ma_result_MA_ERROR;
    }

    let device_ref = unsafe { &*device };
    let engine_ptr = device_ref.pUserData.cast::<sys::ma_engine>();
    let engine_user_data = unsafe { &*engine_ptr }
        .pProcessUserData
        .cast::<EngineUserData>();
    let engine_user_data_ref = unsafe { &*engine_user_data };

    let state_lock = &engine_user_data_ref.backend_state.lock().unwrap();
    let Some(state_ref) = state_lock.as_ref() else {
        return sys::ma_result_MA_ERROR;
    };

    let backend_state_ptr = state_ref.data.cast::<CustomBackendState<B>>();
    let backend_state_ref = unsafe { &*backend_state_ptr };

    let engine_inner = unsafe { &*engine_ptr.cast::<EngineInner>() };

    let log = engine_inner._logger.as_ref().map(|l| LogRef {
        inner: l.inner,
        _owner: LogOwner::Log(l.clone()),
    });

    let custom_context_ref = unsafe { &*device_ref.pContext.cast::<CustomContextInner<B>>() };

    let backend_device: BackendDeviceHandle<B> = BackendDeviceHandle {
        inner: device,
        backend_device: &backend_state_ref.backend_device,
        backend_context: &custom_context_ref.backend_context,
    };

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        B::device_start(&backend_device, log.as_ref())
    }));

    match res {
        Ok(Ok(_)) => sys::ma_result_MA_SUCCESS,
        Ok(Err(error)) => error.ma_result(),
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn engine_custom_context_on_device_stop<B: CustomBackend>(
    device: *mut sys::ma_device,
) -> sys::ma_result {
    if device.is_null() {
        return sys::ma_result_MA_ERROR;
    }

    let device_ref = unsafe { &*device };
    let engine_ptr = device_ref.pUserData.cast::<sys::ma_engine>();
    let engine_user_data = unsafe { &*engine_ptr }
        .pProcessUserData
        .cast::<EngineUserData>();
    let engine_user_data_ref = unsafe { &*engine_user_data };

    let state_lock = &engine_user_data_ref.backend_state.lock().unwrap();
    let Some(state_ref) = state_lock.as_ref() else {
        return sys::ma_result_MA_ERROR;
    };

    let backend_state_ptr = state_ref.data.cast::<CustomBackendState<B>>();
    let backend_state_ref = unsafe { &*backend_state_ptr };

    let engine_inner = unsafe { &*engine_ptr.cast::<EngineInner>() };

    let log = engine_inner._logger.as_ref().map(|l| LogRef {
        inner: l.inner,
        _owner: LogOwner::Log(l.clone()),
    });

    let custom_context_ref = unsafe { &*device_ref.pContext.cast::<CustomContextInner<B>>() };

    let backend_device: BackendDeviceHandle<B> = BackendDeviceHandle {
        inner: device,
        backend_device: &backend_state_ref.backend_device,
        backend_context: &custom_context_ref.backend_context,
    };

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        B::device_stop(&backend_device, log.as_ref())
    }));

    match res {
        Ok(Ok(_)) => sys::ma_result_MA_SUCCESS,
        Ok(Err(error)) => error.ma_result(),
        Err(_) => sys::ma_result_MA_ERROR,
    }
}
