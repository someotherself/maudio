use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

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
            ChannelMixMode::Rectangular => sys::ma_channel_mix_mode_ma_channel_mix_mode_rectangular,
            ChannelMixMode::Simple => sys::ma_channel_mix_mode_ma_channel_mix_mode_simple,
            ChannelMixMode::CustomWeights => {
                sys::ma_channel_mix_mode_ma_channel_mix_mode_custom_weights
            }
            ChannelMixMode::Default => sys::ma_channel_mix_mode_ma_channel_mix_mode_default,
        }
    }
}

impl TryFrom<sys::ma_channel_mix_mode> for ChannelMixMode {
    type Error = MaudioError;

    fn try_from(value: sys::ma_channel_mix_mode) -> Result<Self, Self::Error> {
        match value {
            sys::ma_channel_mix_mode_ma_channel_mix_mode_rectangular => {
                Ok(ChannelMixMode::Rectangular)
            }
            sys::ma_channel_mix_mode_ma_channel_mix_mode_simple => Ok(ChannelMixMode::Simple),
            sys::ma_channel_mix_mode_ma_channel_mix_mode_custom_weights => {
                Ok(ChannelMixMode::CustomWeights)
            }
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                ChannelMixMode,
            >(other as i64))),
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
    type Error = MaudioError;

    fn try_from(value: sys::ma_standard_channel_map) -> Result<Self, Self::Error> {
        match value {
            sys::ma_standard_channel_map_ma_standard_channel_map_microsoft => {
                Ok(ChannelMap::Microsoft)
            }
            sys::ma_standard_channel_map_ma_standard_channel_map_alsa => Ok(ChannelMap::Alsa),
            sys::ma_standard_channel_map_ma_standard_channel_map_rfc3551 => Ok(ChannelMap::Rfc3551),
            sys::ma_standard_channel_map_ma_standard_channel_map_flac => Ok(ChannelMap::Flac),
            sys::ma_standard_channel_map_ma_standard_channel_map_vorbis => Ok(ChannelMap::Vorbis),
            sys::ma_standard_channel_map_ma_standard_channel_map_sound4 => Ok(ChannelMap::Sound4),
            sys::ma_standard_channel_map_ma_standard_channel_map_sndio => Ok(ChannelMap::Sndio),
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                ChannelMap,
            >(other as i64))),
        }
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Channel(pub sys::ma_channel);

impl Channel {
    #[inline]
    pub const fn as_raw(self) -> sys::ma_channel {
        self.0
    }

    #[inline]
    pub const fn from_raw(v: sys::ma_channel) -> Self {
        Self(v)
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChannelPosition {
    None,
    Mono,
    FrontLeft,
    FrontRight,
    FrontCenter,
    Lfe,
    BackLeft,
    BackRight,
    FrontLeftCenter,
    FrontRightCenter,
    BackCenter,
    SideLeft,
    SideRight,
    TopCenter,
    TopFrontLeft,
    TopFrontCenter,
    TopFrontRight,
    TopBackLeft,
    TopBackCenter,
    TopBackRight,
    Aux0,
    Aux1,
    Aux2,
    Aux3,
    Aux4,
    Aux5,
    Aux6,
    Aux7,
    Aux8,
    Aux9,
    Aux10,
    Aux11,
    Aux12,
    Aux13,
    Aux14,
    Aux15,
    Aux16,
    Aux17,
    Aux18,
    Aux19,
    Aux20,
    Aux21,
    Aux22,
    Aux23,
    Aux24,
    Aux25,
    Aux26,
    Aux27,
    Aux28,
    Aux29,
    Aux30,
    Aux31,
}

impl TryFrom<sys::ma_channel> for ChannelPosition {
    type Error = MaudioError;

    fn try_from(v: sys::ma_channel) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::Mono),
            2 => Ok(Self::FrontLeft),
            3 => Ok(Self::FrontRight),
            4 => Ok(Self::FrontCenter),
            5 => Ok(Self::Lfe),
            6 => Ok(Self::BackLeft),
            7 => Ok(Self::BackRight),
            8 => Ok(Self::FrontLeftCenter),
            9 => Ok(Self::FrontRightCenter),
            10 => Ok(Self::BackCenter),
            11 => Ok(Self::SideLeft),
            12 => Ok(Self::SideRight),
            13 => Ok(Self::TopCenter),
            14 => Ok(Self::TopFrontLeft),
            15 => Ok(Self::TopFrontCenter),
            16 => Ok(Self::TopFrontRight),
            17 => Ok(Self::TopBackLeft),
            18 => Ok(Self::TopBackCenter),
            19 => Ok(Self::TopBackRight),
            20 => Ok(Self::Aux0),
            21 => Ok(Self::Aux1),
            22 => Ok(Self::Aux2),
            23 => Ok(Self::Aux3),
            24 => Ok(Self::Aux4),
            25 => Ok(Self::Aux5),
            26 => Ok(Self::Aux6),
            27 => Ok(Self::Aux7),
            28 => Ok(Self::Aux8),
            29 => Ok(Self::Aux9),
            30 => Ok(Self::Aux10),
            31 => Ok(Self::Aux11),
            32 => Ok(Self::Aux12),
            33 => Ok(Self::Aux13),
            34 => Ok(Self::Aux14),
            35 => Ok(Self::Aux15),
            36 => Ok(Self::Aux16),
            37 => Ok(Self::Aux17),
            38 => Ok(Self::Aux18),
            39 => Ok(Self::Aux19),
            40 => Ok(Self::Aux20),
            41 => Ok(Self::Aux21),
            42 => Ok(Self::Aux22),
            43 => Ok(Self::Aux23),
            44 => Ok(Self::Aux24),
            45 => Ok(Self::Aux25),
            46 => Ok(Self::Aux26),
            47 => Ok(Self::Aux27),
            48 => Ok(Self::Aux28),
            49 => Ok(Self::Aux29),
            50 => Ok(Self::Aux30),
            51 => Ok(Self::Aux31),
            _ => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                ChannelPosition,
            >(v as i64))),
        }
    }
}

impl TryFrom<Channel> for ChannelPosition {
    type Error = MaudioError;

    #[inline]
    fn try_from(c: Channel) -> Result<Self, Self::Error> {
        ChannelPosition::try_from(c.0)
    }
}

impl From<ChannelPosition> for Channel {
    #[inline]
    fn from(p: ChannelPosition) -> Self {
        Channel(p as sys::ma_channel)
    }
}

impl From<ChannelPosition> for sys::ma_channel {
    #[inline]
    fn from(p: ChannelPosition) -> Self {
        p as sys::ma_channel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sys, MaError};

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
        let invalid: sys::ma_standard_channel_map = 0x7FFF as sys::ma_standard_channel_map;

        let err = ChannelMap::try_from(invalid).unwrap_err();
        assert_eq!(err, MaError(sys::ma_result_MA_ERROR));
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
            matches!(
                from_default,
                ChannelMixMode::Rectangular | ChannelMixMode::Default
            ),
            "Expected DEFAULT/RECTANGULAR alias to map to Rectangular or Default; got {from_default:?}"
        );
    }

    #[test]
    fn test_channel_mix_mode_try_from_invalid_returns_error() {
        let invalid: sys::ma_standard_channel_map = 0x7FFF as sys::ma_standard_channel_map;

        let err = ChannelMixMode::try_from(invalid).unwrap_err();
        assert_eq!(err, MaError(sys::ma_result_MA_ERROR));
    }
}
