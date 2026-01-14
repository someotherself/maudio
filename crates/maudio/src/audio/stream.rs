use maudio_sys::ffi as sys;

use crate::MaError;

/// Audio stream sample format.
///
/// This enum specifies the fundamental representation of audio samples
/// in a stream.
///
/// At present, miniaudio only defines PCM streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum StreamFormat {
    /// Pulse-code modulation (PCM).
    ///
    /// Audio samples are provided directly as raw PCM data.
    /// This is the only stream format currently supported by miniaudio.
    Pcm,
}

impl From<StreamFormat> for sys::ma_stream_format {
    fn from(value: StreamFormat) -> Self {
        match value {
            StreamFormat::Pcm => sys::ma_stream_format_ma_stream_format_pcm,
        }
    }
}

impl TryFrom<sys::ma_stream_format> for StreamFormat {
    type Error = MaError;

    fn try_from(value: sys::ma_stream_format) -> Result<Self, Self::Error> {
        match value {
            sys::ma_stream_format_ma_stream_format_pcm => Ok(StreamFormat::Pcm),
            _ => Err(MaError(sys::ma_result_MA_INVALID_ARGS)),
        }
    }
}

/// Channel data layout for audio streams.
///
/// This enum specifies how multi-channel audio samples are laid out in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum StreamLayout {
    /// Interleaved channel layout.
    ///
    /// Samples for all channels are interleaved in a single buffer.
    /// For example, stereo audio is laid out as:
    /// `L, R, L, R, L, R, ...`
    ///
    /// This is the most common layout used by audio APIs and is the default
    /// layout in miniaudio.
    Interleaved,
    /// Deinterleaved channel layout.
    ///
    /// Each channel’s samples are stored in a separate buffer.
    /// For example, stereo audio is laid out as:
    /// `LLLL...`, `RRRR...`
    ///
    /// This layout can be useful for certain DSP algorithms but is less
    /// commonly used by audio backends.
    Deinterleaved,
}

impl From<StreamLayout> for sys::ma_stream_layout {
    fn from(value: StreamLayout) -> Self {
        match value {
            StreamLayout::Interleaved => sys::ma_stream_layout_ma_stream_layout_interleaved,
            StreamLayout::Deinterleaved => sys::ma_stream_layout_ma_stream_layout_deinterleaved,
        }
    }
}

impl TryFrom<sys::ma_stream_layout> for StreamLayout {
    type Error = MaError;

    fn try_from(value: sys::ma_stream_layout) -> Result<Self, Self::Error> {
        match value {
            sys::ma_stream_layout_ma_stream_layout_interleaved => Ok(StreamLayout::Interleaved),
            sys::ma_stream_layout_ma_stream_layout_deinterleaved => Ok(StreamLayout::Deinterleaved),
            _ => Err(MaError(sys::ma_result_MA_INVALID_ARGS)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maudio_sys::ffi as sys;

    #[test]
    fn test_stream_format_from_rust_to_sys_pcm() {
        let sys_val: sys::ma_stream_format = StreamFormat::Pcm.into();
        assert_eq!(sys_val, sys::ma_stream_format_ma_stream_format_pcm);
    }

    #[test]
    fn test_stream_format_try_from_sys_to_rust_pcm() {
        let rust_val = StreamFormat::try_from(sys::ma_stream_format_ma_stream_format_pcm).unwrap();
        assert_eq!(rust_val, StreamFormat::Pcm);
    }

    #[test]
    fn test_stream_format_try_from_invalid_returns_error() {
        let invalid: sys::ma_stream_format = 0x7FFF as sys::ma_stream_format;
        let err = StreamFormat::try_from(invalid).unwrap_err();
        assert_eq!(err, MaError(sys::ma_result_MA_INVALID_ARGS));
    }

    #[test]
    fn test_stream_layout_from_rust_to_sys_interleaved() {
        let sys_val: sys::ma_stream_layout = StreamLayout::Interleaved.into();
        assert_eq!(sys_val, sys::ma_stream_layout_ma_stream_layout_interleaved);
    }

    #[test]
    fn test_stream_layout_from_rust_to_sys_deinterleaved() {
        let sys_val: sys::ma_stream_layout = StreamLayout::Deinterleaved.into();
        assert_eq!(
            sys_val,
            sys::ma_stream_layout_ma_stream_layout_deinterleaved
        );
    }

    #[test]
    fn test_stream_layout_try_from_sys_to_rust_interleaved() {
        let rust_val =
            StreamLayout::try_from(sys::ma_stream_layout_ma_stream_layout_interleaved).unwrap();
        assert_eq!(rust_val, StreamLayout::Interleaved);
    }

    #[test]
    fn test_stream_layout_try_from_sys_to_rust_deinterleaved() {
        let rust_val =
            StreamLayout::try_from(sys::ma_stream_layout_ma_stream_layout_deinterleaved).unwrap();
        assert_eq!(rust_val, StreamLayout::Deinterleaved);
    }

    #[test]
    fn test_stream_layout_try_from_invalid_returns_error() {
        let invalid: sys::ma_stream_format = 0x7FFF as sys::ma_stream_format;
        let err = StreamLayout::try_from(invalid).unwrap_err();
        assert_eq!(err, MaError(sys::ma_result_MA_INVALID_ARGS));
    }
}
