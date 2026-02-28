//! Sample rate definitions and conversion utilities.

use crate::{ErrorKinds, MaudioError};

/// Common standard audio sample rates.
///
/// This enum represents a fixed set of widely used sample rates,
/// primarily intended for configuration and interoperability with
/// audio devices, decoders, and DSP components.
///
/// Not all backends or devices support every listed rate. Unsupported
/// rates may be silently converted or rejected depending on the context.
///
/// ### Notes
///
/// - `48_000 Hz` and `44_100 Hz` are the most commonly used rates.
/// - Lower and higher rates are included for compatibility with legacy,
///   low-power, or high-resolution audio pipelines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum SampleRate {
    Sr48000,
    Sr44100,
    Sr32000,
    Sr24000,
    Sr22050,
    Sr88200,
    Sr96000,
    Sr176400,
    Sr192000,
    Sr16000,
    Sr11025,
    Sr8000,
    Sr352800,
    Sr384000,
    Custom(u32),
}

impl From<SampleRate> for i32 {
    fn from(value: SampleRate) -> Self {
        match value {
            SampleRate::Sr48000 => 48_000,
            SampleRate::Sr44100 => 44_100,

            SampleRate::Sr32000 => 32_000,
            SampleRate::Sr24000 => 24_000,
            SampleRate::Sr22050 => 22_050,

            SampleRate::Sr88200 => 88_200,
            SampleRate::Sr96000 => 96_000,
            SampleRate::Sr176400 => 176_400,
            SampleRate::Sr192000 => 192_000,

            SampleRate::Sr16000 => 16_000,
            SampleRate::Sr11025 => 11_025,
            SampleRate::Sr8000 => 8_000,

            SampleRate::Sr352800 => 352_800,
            SampleRate::Sr384000 => 384_000,
            SampleRate::Custom(v) => v as i32,
        }
    }
}

impl From<SampleRate> for u32 {
    fn from(value: SampleRate) -> Self {
        match value {
            SampleRate::Sr48000 => 48_000,
            SampleRate::Sr44100 => 44_100,

            SampleRate::Sr32000 => 32_000,
            SampleRate::Sr24000 => 24_000,
            SampleRate::Sr22050 => 22_050,

            SampleRate::Sr88200 => 88_200,
            SampleRate::Sr96000 => 96_000,
            SampleRate::Sr176400 => 176_400,
            SampleRate::Sr192000 => 192_000,

            SampleRate::Sr16000 => 16_000,
            SampleRate::Sr11025 => 11_025,
            SampleRate::Sr8000 => 8_000,

            SampleRate::Sr352800 => 352_800,
            SampleRate::Sr384000 => 384_000,
            SampleRate::Custom(v) => v,
        }
    }
}

impl TryFrom<u32> for SampleRate {
    type Error = MaudioError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            48_000 => Ok(SampleRate::Sr48000),
            44_100 => Ok(SampleRate::Sr44100),
            32_000 => Ok(SampleRate::Sr32000),
            24_000 => Ok(SampleRate::Sr24000),
            22_050 => Ok(SampleRate::Sr22050),
            88_200 => Ok(SampleRate::Sr88200),
            96_000 => Ok(SampleRate::Sr96000),
            176_400 => Ok(SampleRate::Sr176400),
            192_000 => Ok(SampleRate::Sr192000),
            16_000 => Ok(SampleRate::Sr16000),
            11_025 => Ok(SampleRate::Sr11025),
            8_000 => Ok(SampleRate::Sr8000),
            352_800 => Ok(SampleRate::Sr352800),
            384_000 => Ok(SampleRate::Sr384000),
            v if v == 0 => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                SampleRate,
            >(v as i64))),
            v => Ok(Self::Custom(v)),
        }
    }
}

impl TryFrom<i32> for SampleRate {
    type Error = MaudioError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            48_000 => Ok(SampleRate::Sr48000),
            44_100 => Ok(SampleRate::Sr44100),
            32_000 => Ok(SampleRate::Sr32000),
            24_000 => Ok(SampleRate::Sr24000),
            22_050 => Ok(SampleRate::Sr22050),
            88_200 => Ok(SampleRate::Sr88200),
            96_000 => Ok(SampleRate::Sr96000),
            176_400 => Ok(SampleRate::Sr176400),
            192_000 => Ok(SampleRate::Sr192000),
            16_000 => Ok(SampleRate::Sr16000),
            11_025 => Ok(SampleRate::Sr11025),
            8_000 => Ok(SampleRate::Sr8000),
            352_800 => Ok(SampleRate::Sr352800),
            384_000 => Ok(SampleRate::Sr384000),
            v if v <= 0 => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                SampleRate,
            >(v as i64))),
            v => Ok(Self::Custom(v as u32)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::MaError;

    use super::*;
    use maudio_sys::ffi as sys;

    fn ma_error() -> MaError {
        MaError(sys::ma_result_MA_ERROR)
    }

    #[test]
    fn test_sample_rate_into_sys_matches_expected_constants() {
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr48000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_48000
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr44100),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_44100
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr32000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_32000
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr24000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_24000
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr22050),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_22050
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr88200),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_88200
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr96000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_96000
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr176400),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_176400
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr192000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_192000
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr16000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_16000
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr11025),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_11025
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr8000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_8000
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr352800),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_352800
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Sr384000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_384000
        );
    }

    #[test]
    fn test_sample_rate_try_from_sys_accepts_concrete_values() {
        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_48000)
                .unwrap(),
            SampleRate::Sr48000
        );
        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_44100)
                .unwrap(),
            SampleRate::Sr44100
        );
        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_8000)
                .unwrap(),
            SampleRate::Sr8000
        );
        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_384000)
                .unwrap(),
            SampleRate::Sr384000
        );
    }

    #[test]
    fn test_sample_rate_try_from_sys_accepts_concrete_values_i32() {
        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_48000 as i32)
                .unwrap(),
            SampleRate::Sr48000
        );

        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_44100 as i32)
                .unwrap(),
            SampleRate::Sr44100
        );

        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_8000 as i32)
                .unwrap(),
            SampleRate::Sr8000
        );

        assert_eq!(
            SampleRate::try_from(
                sys::ma_standard_sample_rate_ma_standard_sample_rate_384000 as i32
            )
            .unwrap(),
            SampleRate::Sr384000
        );
    }

    #[test]
    fn test_sample_rate_try_from_sys_accepts_concrete_values_u32() {
        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_48000)
                .unwrap(),
            SampleRate::Sr48000
        );

        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_44100)
                .unwrap(),
            SampleRate::Sr44100
        );

        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_8000)
                .unwrap(),
            SampleRate::Sr8000
        );

        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_384000)
                .unwrap(),
            SampleRate::Sr384000
        );
    }

    #[test]
    fn test_sample_rate_try_from_sys_rejects_invalid_values() {
        let bogus_i32 = (sys::ma_standard_sample_rate_ma_standard_sample_rate_48000) + 12_345;
        let bogus_u32 = (sys::ma_standard_sample_rate_ma_standard_sample_rate_48000) + 12_345;

        let err_i32 = SampleRate::try_from(bogus_i32).unwrap_err();
        let err_u32 = SampleRate::try_from(bogus_u32).unwrap_err();

        assert_eq!(err_i32, ma_error());
        assert_eq!(err_u32, ma_error());
    }

    #[test]
    fn test_sample_rate_roundtrip_sys_to_rust_to_sys() {
        let cases = [
            sys::ma_standard_sample_rate_ma_standard_sample_rate_48000,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_44100,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_32000,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_24000,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_22050,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_88200,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_96000,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_176400,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_192000,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_16000,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_11025,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_8000,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_352800,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_384000,
        ];

        for &v in &cases {
            // i32 roundtrip
            let rust_i32 = SampleRate::try_from(v as i32).unwrap();
            let back_i32: i32 = rust_i32.into();
            assert_eq!(back_i32, v as i32);

            // u32 roundtrip
            let rust_u32 = SampleRate::try_from(v).unwrap();
            assert_eq!(v, rust_u32.into());
        }
    }

    #[test]
    fn test_sample_rate_min_max_are_aliases() {
        // These are intentionally aliases in miniaudio.
        assert_eq!(
            sys::ma_standard_sample_rate_ma_standard_sample_rate_min,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_8000
        );
        assert_eq!(
            sys::ma_standard_sample_rate_ma_standard_sample_rate_max,
            sys::ma_standard_sample_rate_ma_standard_sample_rate_384000
        );
    }
}
