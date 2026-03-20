//! Audio backends supported by miniaudio.
use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

/// Audio backend identifiers used for device and context initialization.
///
/// Each variant maps directly to a `ma_backend` in miniaudio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(C)]
pub enum Backend {
    Wasapi,
    DSound,
    WinMm,
    CoreAudio,
    Sndio,
    Audio4,
    Oss,
    PulseAudio,
    Alsa,
    Jack,
    Aaudio,
    Opensl,
    WebAudio,
    Custom,
    Null,
}

impl From<Backend> for sys::ma_backend {
    fn from(v: Backend) -> Self {
        match v {
            Backend::Wasapi => sys::ma_backend_ma_backend_wasapi,
            Backend::DSound => sys::ma_backend_ma_backend_dsound,
            Backend::WinMm => sys::ma_backend_ma_backend_winmm,
            Backend::CoreAudio => sys::ma_backend_ma_backend_coreaudio,
            Backend::Sndio => sys::ma_backend_ma_backend_sndio,
            Backend::Audio4 => sys::ma_backend_ma_backend_audio4,
            Backend::Oss => sys::ma_backend_ma_backend_oss,
            Backend::PulseAudio => sys::ma_backend_ma_backend_pulseaudio,
            Backend::Alsa => sys::ma_backend_ma_backend_alsa,
            Backend::Jack => sys::ma_backend_ma_backend_jack,
            Backend::Aaudio => sys::ma_backend_ma_backend_aaudio,
            Backend::Opensl => sys::ma_backend_ma_backend_opensl,
            Backend::WebAudio => sys::ma_backend_ma_backend_webaudio,
            Backend::Custom => sys::ma_backend_ma_backend_custom,
            Backend::Null => sys::ma_backend_ma_backend_null,
        }
    }
}

impl TryFrom<sys::ma_backend> for Backend {
    type Error = MaudioError;

    fn try_from(v: sys::ma_backend) -> Result<Self, Self::Error> {
        match v {
            sys::ma_backend_ma_backend_wasapi => Ok(Backend::Wasapi),
            sys::ma_backend_ma_backend_dsound => Ok(Backend::DSound),
            sys::ma_backend_ma_backend_winmm => Ok(Backend::WinMm),
            sys::ma_backend_ma_backend_coreaudio => Ok(Backend::CoreAudio),
            sys::ma_backend_ma_backend_sndio => Ok(Backend::Sndio),
            sys::ma_backend_ma_backend_audio4 => Ok(Backend::Audio4),
            sys::ma_backend_ma_backend_oss => Ok(Backend::Oss),
            sys::ma_backend_ma_backend_pulseaudio => Ok(Backend::PulseAudio),
            sys::ma_backend_ma_backend_alsa => Ok(Backend::Alsa),
            sys::ma_backend_ma_backend_jack => Ok(Backend::Jack),
            sys::ma_backend_ma_backend_aaudio => Ok(Backend::Aaudio),
            sys::ma_backend_ma_backend_opensl => Ok(Backend::Opensl),
            sys::ma_backend_ma_backend_webaudio => Ok(Backend::WebAudio),
            sys::ma_backend_ma_backend_custom => Ok(Backend::Custom),
            sys::ma_backend_ma_backend_null => Ok(Backend::Null),
            other => Err(MaudioError::new_ma_error(
                ErrorKinds::unknown_enum::<Backend>(other as i64),
            )),
        }
    }
}

impl Backend {
    pub const ALL: &'static [Backend] = &[
        Backend::Wasapi,
        Backend::DSound,
        Backend::WinMm,
        Backend::CoreAudio,
        Backend::Sndio,
        Backend::Audio4,
        Backend::Oss,
        Backend::PulseAudio,
        Backend::Alsa,
        Backend::Jack,
        Backend::Aaudio,
        Backend::Opensl,
        Backend::WebAudio,
        Backend::Custom,
        Backend::Null,
    ];

    /// Returns `true` if this backend is supported on the current target OS.
    ///
    /// This is a compile-time check and does not account for disabled features.
    pub const fn possible_on_this_target(self) -> bool {
        match self {
            // Windows
            Backend::Wasapi | Backend::DSound | Backend::WinMm => cfg!(windows),
            // macOS/iOS
            Backend::CoreAudio => cfg!(target_os = "macos") || cfg!(target_os = "ios"),
            // BSD family
            Backend::Sndio => cfg!(target_os = "openbsd"),
            Backend::Audio4 => cfg!(target_os = "netbsd"),
            Backend::Oss => {
                cfg!(target_os = "freebsd")
                    || cfg!(target_os = "dragonfly")
                    || cfg!(target_os = "openbsd")
                    || cfg!(target_os = "netbsd")
            }
            // Linux / Unix audio servers
            Backend::Alsa => cfg!(target_os = "linux"),
            Backend::PulseAudio => cfg!(target_os = "linux") || cfg!(target_os = "android"),
            Backend::Jack => {
                cfg!(target_os = "linux")
                    || cfg!(target_os = "macos")
                    || cfg!(target_os = "freebsd")
            }
            // Android
            Backend::Opensl | Backend::Aaudio => cfg!(target_os = "android"),
            // Web
            Backend::WebAudio => cfg!(target_arch = "wasm32"),
            // Generic / internal
            Backend::Custom => true,
            Backend::Null => true,
        }
    }

    /// Returns `true` if this backend is enabled in the current build configuration.
    ///
    /// Controlled by `no-*` feature flags.
    pub const fn is_enabled_in_build(self) -> bool {
        match self {
            Backend::Wasapi => !cfg!(feature = "no-wasapi"),
            Backend::DSound => !cfg!(feature = "no-dsound"),
            Backend::WinMm => !cfg!(feature = "no-winmm"),
            Backend::CoreAudio => !cfg!(feature = "no-coreaudio"),
            Backend::Sndio => !cfg!(feature = "no-sndio"),
            Backend::Audio4 => !cfg!(feature = "no-audio4"),
            Backend::Oss => !cfg!(feature = "no-oss"),
            Backend::PulseAudio => !cfg!(feature = "no-pulseaudio"),
            Backend::Alsa => !cfg!(feature = "no-alsa"),
            Backend::Jack => !cfg!(feature = "no-jack"),
            Backend::Aaudio => !cfg!(feature = "no-aaudio"),
            Backend::Opensl => !cfg!(feature = "no-opensl"),
            Backend::WebAudio => !cfg!(feature = "no-webaudio"),
            Backend::Custom | Backend::Null => true,
        }
    }

    /// Returns `true` if this backend is both supported on this target
    /// and enabled in the current build.
    pub const fn is_available_in_build(self) -> bool {
        self.possible_on_this_target() && self.is_enabled_in_build()
    }

    /// Returns all backends supported on the current target OS.
    ///
    /// This does not account for feature flags.
    pub fn all_supported_on_this_platform() -> impl Iterator<Item = Backend> {
        Self::ALL
            .iter()
            .copied()
            .filter(|b| b.possible_on_this_target())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn backends_test_supported_on_platform() {
        let backends = Backend::all_supported_on_this_platform();
        for b in backends {
            println!("{:?}", b);
        }
    }
}
