use crate::{
    backend::custom_context::{BackendDeviceConfig, DeviceDescriptor},
    device::{
        custom_device::BackendDeviceHandle, device_id::DeviceId, device_info::DeviceInfo,
        device_type::DeviceType,
    },
    logging::LogRef,
    ErrorKinds, MaResult, MaudioError,
};

pub trait BackendReader {}

pub trait CustomBackend {
    type Context;
    type Device;

    fn init_context(log: Option<LogRef>) -> MaResult<Self::Context>;

    fn enumerate_devices<F>(
        _context: &mut Self::Context,
        mut _f: F,
        _log: Option<LogRef>,
    ) -> MaResult<()>
    where
        F: FnMut(DeviceType, &DeviceInfo) -> bool,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    fn context_get_device_info(
        _context: &mut Self::Context,
        _device_type: DeviceType,
        _device_id: DeviceId,
        _log: Option<LogRef>,
    ) -> MaResult<DeviceInfo> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    fn device_init(
        _device: BackendDeviceHandle<Self>,
        _config: BackendDeviceConfig,
        _playback: Option<&mut DeviceDescriptor>,
        _capture: Option<&mut DeviceDescriptor>,
        _log: Option<LogRef>,
    ) -> MaResult<Self::Device>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    fn device_start(_device: &BackendDeviceHandle<Self>) -> MaResult<()>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    fn device_stop(_device: &BackendDeviceHandle<Self>) -> MaResult<()>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }
}
