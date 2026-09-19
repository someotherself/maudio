use crate::{
    backend::custom_context::{BackendDeviceConfig, DeviceDescriptor},
    device::{
        custom_device::BackendDeviceHandle, device_id::DeviceId, device_info::DeviceInfo,
        device_type::DeviceType,
    },
    logging::LogRef,
    ErrorKinds, MaResult, MaudioError,
};

pub trait CustomBackend {
    type Context;
    type Device<'device>;

    fn init_context(log: Option<&LogRef>) -> MaResult<Self::Context>;

    fn enumerate_devices<F>(
        _context: &mut Self::Context,
        mut _f: F,
        _log: Option<&LogRef>,
    ) -> MaResult<()>
    where
        F: FnMut(DeviceType, &DeviceInfo) -> bool,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    fn context_query_device_info(
        _context: &mut Self::Context,
        _device_type: DeviceType,
        _device_id: DeviceId,
        _log: Option<&LogRef>,
    ) -> MaResult<DeviceInfo> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    fn device_init<'device>(
        _device: BackendDeviceHandle<'device, Self>,
        _config: BackendDeviceConfig,
        _playback: Option<&mut DeviceDescriptor>,
        _capture: Option<&mut DeviceDescriptor>,
        _log: Option<&LogRef>,
    ) -> MaResult<Self::Device<'device>>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    fn device_start<'device>(
        _device: &BackendDeviceHandle<'device, Self>,
        _log: Option<&LogRef>,
    ) -> MaResult<()>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    fn device_stop<'device>(
        _device: &BackendDeviceHandle<'device, Self>,
        _log: Option<&LogRef>,
    ) -> MaResult<()>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    fn device_get_info<'device>(
        _device: BackendDeviceHandle<'device, Self>,
        _context: &'device Self::Context,
        _device_type: DeviceType,
        _log: Option<&LogRef>,
    ) -> MaResult<DeviceInfo>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }
}
