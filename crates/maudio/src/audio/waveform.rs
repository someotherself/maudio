use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum WaveformType {
    Sine,
    Square,
    Triangle,
    Sawtooth,
}

impl From<WaveformType> for sys::ma_waveform_type {
    fn from(value: WaveformType) -> Self {
        match value {
            WaveformType::Sine => sys::ma_waveform_type_ma_waveform_type_sine,
            WaveformType::Square => sys::ma_waveform_type_ma_waveform_type_square,
            WaveformType::Triangle => sys::ma_waveform_type_ma_waveform_type_triangle,
            WaveformType::Sawtooth => sys::ma_waveform_type_ma_waveform_type_sawtooth,
        }
    }
}

impl TryFrom<sys::ma_stream_format> for WaveformType {
    type Error = MaudioError;

    fn try_from(value: sys::ma_stream_format) -> Result<Self, Self::Error> {
        match value {
            sys::ma_waveform_type_ma_waveform_type_sine => Ok(WaveformType::Sine),
            sys::ma_waveform_type_ma_waveform_type_square => Ok(WaveformType::Square),
            sys::ma_waveform_type_ma_waveform_type_triangle => Ok(WaveformType::Triangle),
            sys::ma_waveform_type_ma_waveform_type_sawtooth => Ok(WaveformType::Sawtooth),
            _ => Err(MaudioError::new_ma_error(ErrorKinds::InvalidSWaveFormType)),
        }
    }
}
