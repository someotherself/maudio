use crate::{
    backend::custom_context::{DeviceDescriptor, UserDeviceConfig},
    device::{
        custom_device::UserDevice, device_id::DeviceId, device_info::DeviceInfo,
        device_type::DeviceType,
    },
    logging::LogRef,
    ErrorKinds, MaResult, MaudioError,
};

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

    fn device_info(
        _context: &mut Self::Context,
        _device_type: DeviceType,
        _device_id: DeviceId,
        _log: Option<LogRef>,
    ) -> MaResult<DeviceInfo> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    fn device_init(
        _device: UserDevice<Self>,
        _config: UserDeviceConfig,
        _playback: Option<&mut DeviceDescriptor>,
        _capture: Option<&mut DeviceDescriptor>,
    ) -> MaResult<Self::Device>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }
}
