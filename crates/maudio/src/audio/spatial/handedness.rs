//! Handedness type definition
use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

/// Defines the coordinate system handedness used for spatial audio calculations.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handedness {
    Right,
    Left,
}

impl From<Handedness> for sys::ma_handedness {
    fn from(v: Handedness) -> Self {
        match v {
            Handedness::Right => sys::ma_handedness_ma_handedness_right,
            Handedness::Left => sys::ma_handedness_ma_handedness_left,
        }
    }
}

impl TryFrom<sys::ma_handedness> for Handedness {
    type Error = MaudioError;

    fn try_from(v: sys::ma_handedness) -> Result<Self, Self::Error> {
        match v {
            sys::ma_handedness_ma_handedness_right => Ok(Handedness::Right),
            sys::ma_handedness_ma_handedness_left => Ok(Handedness::Left),
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                Handedness,
            >(other as i64))),
        }
    }
}
