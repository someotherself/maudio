use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum WaveFormType {
    Sine,
    Square,
    Triangle,
    Sawtooth,
}

impl From<WaveFormType> for sys::ma_waveform_type {
    fn from(value: WaveFormType) -> Self {
        match value {
            WaveFormType::Sine => sys::ma_waveform_type_ma_waveform_type_sine,
            WaveFormType::Square => sys::ma_waveform_type_ma_waveform_type_square,
            WaveFormType::Triangle => sys::ma_waveform_type_ma_waveform_type_triangle,
            WaveFormType::Sawtooth => sys::ma_waveform_type_ma_waveform_type_sawtooth,
        }
    }
}

impl TryFrom<sys::ma_stream_format> for WaveFormType {
    type Error = MaudioError;

    fn try_from(value: sys::ma_stream_format) -> Result<Self, Self::Error> {
        match value {
            sys::ma_waveform_type_ma_waveform_type_sine => Ok(WaveFormType::Sine),
            sys::ma_waveform_type_ma_waveform_type_square => Ok(WaveFormType::Square),
            sys::ma_waveform_type_ma_waveform_type_triangle => Ok(WaveFormType::Triangle),
            sys::ma_waveform_type_ma_waveform_type_sawtooth => Ok(WaveFormType::Sawtooth),
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                WaveFormType,
            >(other as i64))),
        }
    }
}
