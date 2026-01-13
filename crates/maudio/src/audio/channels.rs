use maudio_sys::ffi as sys;

use crate::MaError;

/// Channel mixing strategy used by the channel converter when a direct 1:1 channel-position mapping
/// is not possible (or when channel counts differ).
///
/// In miniaudio, if the input and output channel counts are the same *and* the channel maps contain
/// the same channel positions (just in a different order), channels are simply shuffled.
/// If there is no 1:1 mapping of channel positions, or the channel counts differ, channels are mixed
/// according to a `ChannelMixMode` configured via `ma_channel_converter_config`.
///
/// Notes from miniaudio’s channel mapping rules:
/// - **Mono → multi-channel**: the mono channel is copied to each output channel.
/// - **Multi-channel → mono**: all channels are averaged and copied to the mono channel.
/// - For more complex conversions, one of the mix modes below is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum ChannelMixMode {
    /// Rectangular mixing.
    ///
    /// Miniaudio comment: “Simple averaging based on the plane(s) the channel is sitting on.”
    ///
    /// This mode uses spatial locality based on a rectangle to compute a simple distribution
    /// between input and output channel positions. Conceptually, imagine sitting in the middle
    /// of a room with speakers on the walls representing channel positions.
    Rectangular,
    /// Simple mixing.
    ///
    /// Miniaudio comment: “Drop excess channels; zeroed out extra channels.”
    ///
    /// Excess input channels are dropped, and any extra output channels are filled with silence.
    /// Example:
    /// - 4 → 2: channels 3 and 4 are dropped
    /// - 2 → 4: channels 3 and 4 are set to silence
    Simple,
    /// Custom weights mixing.
    ///
    /// Miniaudio comment: “Use custom weights specified in ma_channel_converter_config.”
    ///
    /// This mode applies user-defined weights configured on the channel converter config.
    CustomWeights,
    /// Default mixing mode.
    ///
    /// In miniaudio this maps to `Rectangular`.
    Default,
}

impl From<ChannelMixMode> for sys::ma_channel_mix_mode {
    fn from(value: ChannelMixMode) -> Self {
        match value {
            ChannelMixMode::Rectangular =>
                sys::ma_channel_mix_mode_ma_channel_mix_mode_rectangular,
            ChannelMixMode::Simple =>
                sys::ma_channel_mix_mode_ma_channel_mix_mode_simple,
            ChannelMixMode::CustomWeights =>
                sys::ma_channel_mix_mode_ma_channel_mix_mode_custom_weights,
            ChannelMixMode::Default =>
                sys::ma_channel_mix_mode_ma_channel_mix_mode_default,
        }
    }
}

impl TryFrom<sys::ma_channel_mix_mode> for ChannelMixMode {
    type Error = MaError;

    fn try_from(value: sys::ma_channel_mix_mode) -> Result<Self, Self::Error> {
        match value {
            sys::ma_channel_mix_mode_ma_channel_mix_mode_rectangular =>
                Ok(ChannelMixMode::Rectangular),
            sys::ma_channel_mix_mode_ma_channel_mix_mode_simple =>
                Ok(ChannelMixMode::Simple),
            sys::ma_channel_mix_mode_ma_channel_mix_mode_custom_weights =>
                Ok(ChannelMixMode::CustomWeights),
            _ => Err(MaError(sys::ma_result_MA_INVALID_ARGS)),
        }
    }

}

/// Standard channel ordering conventions.
///
/// This enum specifies how audio channels are ordered in memory for
/// multi-channel audio streams. Different platforms, file formats, and
/// APIs use different canonical layouts.
///
/// These variants directly correspond to miniaudio’s
/// `ma_standard_channel_map`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum ChannelMap {
    /// Microsoft channel ordering.
    ///
    /// This is the default channel layout used by Windows audio APIs
    /// (e.g. WASAPI) and is the default channel map in miniaudio.
    ///
    /// Typical ordering:
    /// - Mono: `M`
    /// - Stereo: `L, R`
    /// - Surround: `L, R, C, LFE, SL, SR`
    Microsoft,

    /// ALSA (Advanced Linux Sound Architecture) channel ordering.
    ///
    /// Used by ALSA on Linux systems. The ordering differs slightly from
    /// Microsoft layouts for certain surround configurations.
    Alsa,

    /// RFC 3551 channel ordering.
    ///
    /// Based on AIFF channel conventions and defined in RFC 3551
    /// (RTP payload format for audio and video conferences).
    Rfc3551,

    /// FLAC channel ordering.
    ///
    /// Used by the FLAC audio format. Channels follow the ordering defined
    /// by the FLAC specification.
    Flac,

    /// Vorbis channel ordering.
    ///
    /// Used by the Vorbis audio format. This ordering is also commonly used
    /// by other container formats that adopt Vorbis conventions.
    Vorbis,

    /// FreeBSD `sound(4)` channel ordering.
    ///
    /// Used by FreeBSD’s legacy audio subsystem.
    Sound4,

    /// sndio channel ordering.
    ///
    /// Used by the sndio audio system.
    /// See: <https://www.sndio.org/tips.html>
    Sndio,

    /// Web Audio API channel ordering.
    ///
    /// Defined by the Web Audio API specification:
    /// <https://webaudio.github.io/web-audio-api/#ChannelOrdering>
    ///
    /// Only 1, 2, 4, and 6 channel layouts are explicitly defined by the
    /// specification, but additional layouts can be inferred using
    /// logical assumptions.
    ///
    /// In miniaudio, this maps to the same ordering as `Flac`.
    Webaudio,

    /// Default channel ordering.
    ///
    /// This maps to `Microsoft`, which is the default channel map used
    /// throughout miniaudio when no explicit channel map is specified.
    Default,
}

impl From<ChannelMap> for sys::ma_standard_channel_map {
    fn from(value: ChannelMap) -> Self {
        match value {
            ChannelMap::Microsoft => sys::ma_standard_channel_map_ma_standard_channel_map_microsoft,
            ChannelMap::Alsa => sys::ma_standard_channel_map_ma_standard_channel_map_alsa,
            ChannelMap::Rfc3551 => sys::ma_standard_channel_map_ma_standard_channel_map_rfc3551,
            ChannelMap::Flac => sys::ma_standard_channel_map_ma_standard_channel_map_flac,
            ChannelMap::Vorbis => sys::ma_standard_channel_map_ma_standard_channel_map_vorbis,
            ChannelMap::Sound4 => sys::ma_standard_channel_map_ma_standard_channel_map_sound4,
            ChannelMap::Sndio => sys::ma_standard_channel_map_ma_standard_channel_map_sndio,
            ChannelMap::Webaudio => sys::ma_standard_channel_map_ma_standard_channel_map_webaudio,
            ChannelMap::Default => sys::ma_standard_channel_map_ma_standard_channel_map_default,
        }
    }
}

impl TryFrom<sys::ma_standard_channel_map> for ChannelMap {
    type Error = MaError;

    fn try_from(value: sys::ma_standard_channel_map) -> Result<Self, Self::Error> {
        match value {
            sys::ma_standard_channel_map_ma_standard_channel_map_microsoft => Ok(ChannelMap::Microsoft),
            sys::ma_standard_channel_map_ma_standard_channel_map_alsa => Ok(ChannelMap::Alsa),
            sys::ma_standard_channel_map_ma_standard_channel_map_rfc3551 => Ok(ChannelMap::Rfc3551),
            sys::ma_standard_channel_map_ma_standard_channel_map_flac => Ok(ChannelMap::Flac),
            sys::ma_standard_channel_map_ma_standard_channel_map_vorbis => Ok(ChannelMap::Vorbis),
            sys::ma_standard_channel_map_ma_standard_channel_map_sound4 => Ok(ChannelMap::Sound4),
            sys::ma_standard_channel_map_ma_standard_channel_map_sndio => Ok(ChannelMap::Sndio),
            _ => Err(MaError(sys::ma_result_MA_INVALID_ARGS)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys;

    #[test]
    fn test_channel_map_from_rust_to_sys_variants() {
        // Exact-name variants.
        assert_eq!(
            sys::ma_standard_channel_map::from(ChannelMap::Microsoft),
            sys::ma_standard_channel_map_ma_standard_channel_map_microsoft
        );
        assert_eq!(
            sys::ma_standard_channel_map::from(ChannelMap::Alsa),
            sys::ma_standard_channel_map_ma_standard_channel_map_alsa
        );
        assert_eq!(
            sys::ma_standard_channel_map::from(ChannelMap::Rfc3551),
            sys::ma_standard_channel_map_ma_standard_channel_map_rfc3551
        );
        assert_eq!(
            sys::ma_standard_channel_map::from(ChannelMap::Flac),
            sys::ma_standard_channel_map_ma_standard_channel_map_flac
        );
        assert_eq!(
            sys::ma_standard_channel_map::from(ChannelMap::Vorbis),
            sys::ma_standard_channel_map_ma_standard_channel_map_vorbis
        );
        assert_eq!(
            sys::ma_standard_channel_map::from(ChannelMap::Sound4),
            sys::ma_standard_channel_map_ma_standard_channel_map_sound4
        );
        assert_eq!(
            sys::ma_standard_channel_map::from(ChannelMap::Sndio),
            sys::ma_standard_channel_map_ma_standard_channel_map_sndio
        );

        // Aliases from miniaudio:
        // - webaudio = flac
        // - default = microsoft
        assert_eq!(
            sys::ma_standard_channel_map::from(ChannelMap::Webaudio),
            sys::ma_standard_channel_map_ma_standard_channel_map_webaudio
        );
        assert_eq!(
            sys::ma_standard_channel_map_ma_standard_channel_map_webaudio,
            sys::ma_standard_channel_map_ma_standard_channel_map_flac
        );

        assert_eq!(
            sys::ma_standard_channel_map::from(ChannelMap::Default),
            sys::ma_standard_channel_map_ma_standard_channel_map_default
        );
        assert_eq!(
            sys::ma_standard_channel_map_ma_standard_channel_map_default,
            sys::ma_standard_channel_map_ma_standard_channel_map_microsoft
        );
    }

    #[test]
    fn test_channel_map_try_from_sys_to_rust_variants() {
        // Most should round-trip directly.
        assert_eq!(
            ChannelMap::try_from(sys::ma_standard_channel_map_ma_standard_channel_map_microsoft)
                .unwrap(),
            ChannelMap::Microsoft
        );
        assert_eq!(
            ChannelMap::try_from(sys::ma_standard_channel_map_ma_standard_channel_map_alsa)
                .unwrap(),
            ChannelMap::Alsa
        );
        assert_eq!(
            ChannelMap::try_from(sys::ma_standard_channel_map_ma_standard_channel_map_rfc3551)
                .unwrap(),
            ChannelMap::Rfc3551
        );
        assert_eq!(
            ChannelMap::try_from(sys::ma_standard_channel_map_ma_standard_channel_map_flac)
                .unwrap(),
            ChannelMap::Flac
        );
        assert_eq!(
            ChannelMap::try_from(sys::ma_standard_channel_map_ma_standard_channel_map_vorbis)
                .unwrap(),
            ChannelMap::Vorbis
        );
        assert_eq!(
            ChannelMap::try_from(sys::ma_standard_channel_map_ma_standard_channel_map_sound4)
                .unwrap(),
            ChannelMap::Sound4
        );
        assert_eq!(
            ChannelMap::try_from(sys::ma_standard_channel_map_ma_standard_channel_map_sndio)
                .unwrap(),
            ChannelMap::Sndio
        );

        // Alias semantics:
        // miniaudio defines:
        //   ma_standard_channel_map_webaudio = ma_standard_channel_map_flac
        //
        // That means the *sys value* for "webaudio" is numerically identical to FLAC.
        // In a TryFrom mapping, you can only pick one Rust variant for that number.
        //
        // The usual choice is to map that numeric value back to `ChannelMap::Flac`.
        // If your TryFrom intentionally maps it to `Webaudio` instead, change this assertion.
        let from_webaudio =
            ChannelMap::try_from(sys::ma_standard_channel_map_ma_standard_channel_map_webaudio)
                .unwrap();
        assert!(
            matches!(from_webaudio, ChannelMap::Flac | ChannelMap::Webaudio),
            "Expected FLAC/WEBAUDIO alias to map to Flac or Webaudio; got {from_webaudio:?}"
        );

        // Default is also an alias:
        //   ma_standard_channel_map_default = ma_standard_channel_map_microsoft
        //
        // Same ambiguity as above: the numeric value is identical. Accept either.
        let from_default =
            ChannelMap::try_from(sys::ma_standard_channel_map_ma_standard_channel_map_default)
                .unwrap();
        assert!(
            matches!(from_default, ChannelMap::Microsoft | ChannelMap::Default),
            "Expected DEFAULT/MICROSOFT alias to map to Microsoft or Default; got {from_default:?}"
        );
    }

    #[test]
    fn test_channel_map_try_from_invalid_returns_error() {
        // Create an invalid sys enum value. Bindgen usually emits these as C-like enums,
        // so transmuting an out-of-range integer is the typical FFI test approach.
        let invalid: sys::ma_standard_channel_map = i32::cast_unsigned(0x7FFF_i32);

        let err = ChannelMap::try_from(invalid).unwrap_err();
        assert_eq!(err, MaError(sys::ma_result_MA_INVALID_ARGS));
    }

    #[test]
    fn test_channel_mix_mode_from_rust_to_sys_variants() {
        assert_eq!(
            sys::ma_channel_mix_mode::from(ChannelMixMode::Rectangular),
            sys::ma_channel_mix_mode_ma_channel_mix_mode_rectangular
        );
        assert_eq!(
            sys::ma_channel_mix_mode::from(ChannelMixMode::Simple),
            sys::ma_channel_mix_mode_ma_channel_mix_mode_simple
        );
        assert_eq!(
            sys::ma_channel_mix_mode::from(ChannelMixMode::CustomWeights),
            sys::ma_channel_mix_mode_ma_channel_mix_mode_custom_weights
        );

        // Alias from miniaudio:
        // - default = rectangular
        assert_eq!(
            sys::ma_channel_mix_mode::from(ChannelMixMode::Default),
            sys::ma_channel_mix_mode_ma_channel_mix_mode_default
        );
        assert_eq!(
            sys::ma_channel_mix_mode_ma_channel_mix_mode_default,
            sys::ma_channel_mix_mode_ma_channel_mix_mode_rectangular
        );
    }

    #[test]
    fn test_channel_mix_mode_try_from_sys_to_rust_variants() {
        assert_eq!(
            ChannelMixMode::try_from(sys::ma_channel_mix_mode_ma_channel_mix_mode_rectangular)
                .unwrap(),
            ChannelMixMode::Rectangular
        );
        assert_eq!(
            ChannelMixMode::try_from(sys::ma_channel_mix_mode_ma_channel_mix_mode_simple).unwrap(),
            ChannelMixMode::Simple
        );
        assert_eq!(
            ChannelMixMode::try_from(sys::ma_channel_mix_mode_ma_channel_mix_mode_custom_weights)
                .unwrap(),
            ChannelMixMode::CustomWeights
        );

        // Default is an alias for rectangular in miniaudio; accept either mapping choice.
        let from_default =
            ChannelMixMode::try_from(sys::ma_channel_mix_mode_ma_channel_mix_mode_default).unwrap();
        assert!(
            matches!(from_default, ChannelMixMode::Rectangular | ChannelMixMode::Default),
            "Expected DEFAULT/RECTANGULAR alias to map to Rectangular or Default; got {from_default:?}"
        );
    }

    #[test]
    fn test_channel_mix_mode_try_from_invalid_returns_error() {
        let invalid: sys::ma_channel_mix_mode = i32::cast_unsigned(0x7FFF_i32);

        let err = ChannelMixMode::try_from(invalid).unwrap_err();
        assert_eq!(err, MaError(sys::ma_result_MA_INVALID_ARGS));
    }
}