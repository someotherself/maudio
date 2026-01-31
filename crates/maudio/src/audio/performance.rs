use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

/// Performance tuning profile for audio processing.
///
/// This enum provides a hint to miniaudio about the performance characteristics
/// preferred by the application. It primarily influences internal defaults such
/// as buffer sizes and scheduling behavior.
///
/// In most cases this can be left at its default value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum PerformanceProfile {
    /// Low-latency performance profile.
    ///
    /// This is the default profile used by miniaudio.
    ///
    /// Prioritizes lower audio latency, typically by using smaller internal
    /// buffers. This may increase CPU usage, but results in more responsive
    /// audio output, which is generally desirable for real-time audio
    /// applications such as games or interactive audio software.
    LowLatency,
    /// Conservative performance profile.
    ///
    /// Prioritizes stability and reduced CPU usage over minimal latency.
    ///
    /// This typically results in larger internal buffers, which can improve
    /// robustness on slower systems or under heavy load, at the cost of
    /// increased audio latency.
    Conservative,
}

impl From<PerformanceProfile> for sys::ma_performance_profile {
    fn from(value: PerformanceProfile) -> Self {
        match value {
            PerformanceProfile::LowLatency => {
                sys::ma_performance_profile_ma_performance_profile_low_latency
            }
            PerformanceProfile::Conservative => {
                sys::ma_performance_profile_ma_performance_profile_conservative
            }
        }
    }
}

impl TryFrom<sys::ma_performance_profile> for PerformanceProfile {
    type Error = MaudioError;

    fn try_from(value: sys::ma_performance_profile) -> Result<Self, Self::Error> {
        match value {
            sys::ma_performance_profile_ma_performance_profile_low_latency => {
                Ok(PerformanceProfile::LowLatency)
            }
            sys::ma_performance_profile_ma_performance_profile_conservative => {
                Ok(PerformanceProfile::Conservative)
            }
            _ => Err(MaudioError::new_ma_error(
                ErrorKinds::InvalidPerformanceProfile,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sys, MaError};

    #[test]
    fn test_performance_profile_from_rust_to_sys_low_latency() {
        let sys_val: sys::ma_performance_profile = PerformanceProfile::LowLatency.into();
        assert_eq!(
            sys_val,
            sys::ma_performance_profile_ma_performance_profile_low_latency
        );
    }

    #[test]
    fn test_performance_profile_from_rust_to_sys_conservative() {
        let sys_val: sys::ma_performance_profile = PerformanceProfile::Conservative.into();
        assert_eq!(
            sys_val,
            sys::ma_performance_profile_ma_performance_profile_conservative
        );
    }

    #[test]
    fn test_performance_profile_try_from_sys_to_rust_low_latency() {
        let rust_val = PerformanceProfile::try_from(
            sys::ma_performance_profile_ma_performance_profile_low_latency,
        )
        .unwrap();
        assert_eq!(rust_val, PerformanceProfile::LowLatency);
    }

    #[test]
    fn test_performance_profile_try_from_sys_to_rust_conservative() {
        let rust_val = PerformanceProfile::try_from(
            sys::ma_performance_profile_ma_performance_profile_conservative,
        )
        .unwrap();
        assert_eq!(rust_val, PerformanceProfile::Conservative);
    }

    #[test]
    fn test_performance_profile_try_from_invalid_returns_error() {
        let invalid: sys::ma_performance_profile = 0x7FFF as sys::ma_performance_profile;

        let err = PerformanceProfile::try_from(invalid).unwrap_err();
        assert_eq!(err, MaError(sys::ma_result_MA_ERROR));
    }
}
