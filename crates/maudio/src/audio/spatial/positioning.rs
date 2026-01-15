use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Positioning {
    Absolute,
    Relative,
}

impl From<Positioning> for sys::ma_positioning {
    fn from(v: Positioning) -> Self {
        match v {
            Positioning::Absolute => sys::ma_positioning_ma_positioning_absolute,
            Positioning::Relative => sys::ma_positioning_ma_positioning_relative,
        }
    }
}

impl TryFrom<sys::ma_positioning> for Positioning {
    type Error = MaudioError;

    fn try_from(v: sys::ma_positioning) -> Result<Self, Self::Error> {
        match v {
            sys::ma_positioning_ma_positioning_absolute => Ok(Positioning::Absolute),
            sys::ma_positioning_ma_positioning_relative => Ok(Positioning::Relative),
            _ => Err(MaudioError::new_ma_error(ErrorKinds::InvalidPositioning)),
        }
    }
}
