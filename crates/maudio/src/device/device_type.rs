//! Defines playback, capture, and duplex device types.
use std::fmt::Display;

use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

/// Specifies the role of an audio device.
///
/// Maps directly to `ma_device_type` in miniaudio.
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DeviceType {
    /// Playback (output) device.
    Playback,
    /// Capture (input) device.
    Capture,
    /// Combined playback and capture device.
    Duplex,
    /// Loopback device capturing system output. Windows only.
    Loopback,
}

impl From<DeviceType> for sys::ma_device_type {
    fn from(v: DeviceType) -> Self {
        match v {
            DeviceType::Playback => sys::ma_device_type_ma_device_type_playback,
            DeviceType::Capture => sys::ma_device_type_ma_device_type_capture,
            DeviceType::Duplex => sys::ma_device_type_ma_device_type_duplex,
            DeviceType::Loopback => sys::ma_device_type_ma_device_type_loopback,
        }
    }
}

impl TryFrom<sys::ma_device_type> for DeviceType {
    type Error = MaudioError;

    fn try_from(value: sys::ma_device_type) -> Result<Self, Self::Error> {
        match value {
            sys::ma_device_type_ma_device_type_playback => Ok(DeviceType::Playback),
            sys::ma_device_type_ma_device_type_capture => Ok(DeviceType::Capture),
            sys::ma_device_type_ma_device_type_duplex => Ok(DeviceType::Duplex),
            sys::ma_device_type_ma_device_type_loopback => Ok(DeviceType::Loopback),
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                DeviceType,
            >(other as i64))),
        }
    }
}

impl Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceType::Playback => write!(f, "Playback"),
            DeviceType::Capture => write!(f, "Capture"),
            DeviceType::Loopback => write!(f, "Loopback"),
            DeviceType::Duplex => write!(f, "Duplex"),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DeviceShareMode {
    Shared,
    Exclusive,
}

impl From<DeviceShareMode> for sys::ma_share_mode {
    fn from(value: DeviceShareMode) -> Self {
        match value {
            DeviceShareMode::Shared => sys::ma_share_mode_ma_share_mode_shared,
            DeviceShareMode::Exclusive => sys::ma_share_mode_ma_share_mode_exclusive,
        }
    }
}

impl TryFrom<sys::ma_share_mode> for DeviceShareMode {
    type Error = MaudioError;

    fn try_from(value: sys::ma_share_mode) -> Result<Self, Self::Error> {
        match value {
            sys::ma_share_mode_ma_share_mode_shared => Ok(DeviceShareMode::Shared),
            sys::ma_share_mode_ma_share_mode_exclusive => Ok(DeviceShareMode::Exclusive),
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                DeviceShareMode,
            >(other as i64))),
        }
    }
}
