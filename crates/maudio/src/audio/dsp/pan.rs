use maudio_sys::ffi as sys;

use crate::MaError;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanMode {
    Balance,
    Pan,
}

impl From<PanMode> for sys::ma_pan_mode {
    fn from(value: PanMode) -> Self {
        match value {
            PanMode::Balance => sys::ma_pan_mode_ma_pan_mode_balance,
            PanMode::Pan => sys::ma_pan_mode_ma_pan_mode_pan,
        }
    }
}

impl TryFrom<sys::ma_pan_mode> for PanMode {
    type Error = MaError;

    fn try_from(value: sys::ma_pan_mode) -> Result<Self, Self::Error> {
        match value {
            sys::ma_pan_mode_ma_pan_mode_balance => Ok(PanMode::Balance),
            sys::ma_pan_mode_ma_pan_mode_pan => Ok(PanMode::Pan),
            _ => Err(MaError(sys::ma_result_MA_INVALID_ARGS)),
        }
    }
}
