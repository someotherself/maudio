//! Device state definitions.
use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

/// Represents the current state of an audio device.
///
/// Maps directly to `ma_device_state` in miniaudio.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub enum DeviceState {
    Started,
    Stopped,
    Starting,
    Stopping,
    Uninitialized,
}

impl From<DeviceState> for sys::ma_device_state {
    fn from(value: DeviceState) -> Self {
        match value {
            DeviceState::Started => sys::ma_device_state_ma_device_state_started,
            DeviceState::Stopped => sys::ma_device_state_ma_device_state_stopped,
            DeviceState::Starting => sys::ma_device_state_ma_device_state_starting,
            DeviceState::Stopping => sys::ma_device_state_ma_device_state_stopping,
            DeviceState::Uninitialized => sys::ma_device_state_ma_device_state_uninitialized,
        }
    }
}

impl TryFrom<sys::ma_device_state> for DeviceState {
    type Error = MaudioError;

    fn try_from(value: sys::ma_device_state) -> Result<Self, Self::Error> {
        match value {
            sys::ma_device_state_ma_device_state_started => Ok(DeviceState::Started),
            sys::ma_device_state_ma_device_state_stopped => Ok(DeviceState::Stopped),
            sys::ma_device_state_ma_device_state_starting => Ok(DeviceState::Starting),
            sys::ma_device_state_ma_device_state_stopping => Ok(DeviceState::Stopping),
            sys::ma_device_state_ma_device_state_uninitialized => Ok(DeviceState::Uninitialized),
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                DeviceState,
            >(other as i64))),
        }
    }
}
