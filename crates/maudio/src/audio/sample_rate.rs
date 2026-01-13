// ma_standard_sample_rate (SampleRate helpers???)

use maudio_sys::ffi as sys;

use crate::MaError;

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
/// - `Srmin` and `Srmax` are aliases provided by miniaudio and map to
///   `8_000 Hz` and `384_000 Hz` respectively. They do **not** represent
///   distinct sample rates and are included for FFI completeness.
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
    Srmin,
    Srmax,
}

impl From<SampleRate> for sys::ma_format {
    fn from(value: SampleRate) -> Self {
        match value {
            SampleRate::Sr48000 => sys::ma_standard_sample_rate_ma_standard_sample_rate_48000,
            SampleRate::Sr44100 => sys::ma_standard_sample_rate_ma_standard_sample_rate_44100,

            SampleRate::Sr32000 => sys::ma_standard_sample_rate_ma_standard_sample_rate_32000,
            SampleRate::Sr24000 => sys::ma_standard_sample_rate_ma_standard_sample_rate_24000,
            SampleRate::Sr22050 => sys::ma_standard_sample_rate_ma_standard_sample_rate_22050,

            SampleRate::Sr88200 => sys::ma_standard_sample_rate_ma_standard_sample_rate_88200,
            SampleRate::Sr96000 => sys::ma_standard_sample_rate_ma_standard_sample_rate_96000,
            SampleRate::Sr176400 => sys::ma_standard_sample_rate_ma_standard_sample_rate_176400,
            SampleRate::Sr192000 => sys::ma_standard_sample_rate_ma_standard_sample_rate_192000,

            SampleRate::Sr16000 => sys::ma_standard_sample_rate_ma_standard_sample_rate_16000,
            SampleRate::Sr11025 => sys::ma_standard_sample_rate_ma_standard_sample_rate_11025,
            SampleRate::Sr8000 => sys::ma_standard_sample_rate_ma_standard_sample_rate_8000,

            SampleRate::Sr352800 => sys::ma_standard_sample_rate_ma_standard_sample_rate_352800,
            SampleRate::Sr384000 => sys::ma_standard_sample_rate_ma_standard_sample_rate_384000,

            SampleRate::Srmin => sys::ma_standard_sample_rate_ma_standard_sample_rate_min,
            SampleRate::Srmax => sys::ma_standard_sample_rate_ma_standard_sample_rate_max,
        }
    }
}

impl TryFrom<sys::ma_standard_sample_rate> for SampleRate {
    type Error = MaError;
    fn try_from(value: sys::ma_standard_sample_rate) -> Result<Self, Self::Error> {
        match value {
            sys::ma_standard_sample_rate_ma_standard_sample_rate_48000 => Ok(SampleRate::Sr48000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_44100 => Ok(SampleRate::Sr44100),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_32000 => Ok(SampleRate::Sr32000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_24000 => Ok(SampleRate::Sr24000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_22050 => Ok(SampleRate::Sr22050),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_88200 => Ok(SampleRate::Sr88200),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_96000 => Ok(SampleRate::Sr96000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_176400 => Ok(SampleRate::Sr176400),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_192000 => Ok(SampleRate::Sr192000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_16000 => Ok(SampleRate::Sr16000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_11025 => Ok(SampleRate::Sr11025),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_8000 => Ok(SampleRate::Sr8000),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_352800 => Ok(SampleRate::Sr352800),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_384000 => Ok(SampleRate::Sr384000),
            _ => Err(MaError(sys::ma_result_MA_INVALID_ARGS)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maudio_sys::ffi as sys;

    fn invalid_args() -> MaError {
        MaError(sys::ma_result_MA_INVALID_ARGS)
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
        assert_eq!(
            sys::ma_format::from(SampleRate::Srmin),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_min
        );
        assert_eq!(
            sys::ma_format::from(SampleRate::Srmax),
            sys::ma_standard_sample_rate_ma_standard_sample_rate_max
        );
    }

    #[test]
    fn test_sample_rate_try_from_sys_accepts_concrete_values() {
        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_48000).unwrap(),
            SampleRate::Sr48000
        );
        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_44100).unwrap(),
            SampleRate::Sr44100
        );
        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_8000).unwrap(),
            SampleRate::Sr8000
        );
        assert_eq!(
            SampleRate::try_from(sys::ma_standard_sample_rate_ma_standard_sample_rate_384000).unwrap(),
            SampleRate::Sr384000
        );
    }

    #[test]
    fn test_sample_rate_try_from_sys_rejects_invalid_values() {
        let bogus = sys::ma_standard_sample_rate_ma_standard_sample_rate_48000 + 12345;
        let err = SampleRate::try_from(bogus).unwrap_err();
        assert_eq!(err, invalid_args());
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
            let rust = SampleRate::try_from(v).unwrap();
            let back = sys::ma_format::from(rust);
            assert_eq!(back, v);
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