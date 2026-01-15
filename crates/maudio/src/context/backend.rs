use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

#[derive(Clone, Copy, Debug)]
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
            _ => Err(MaudioError::new_ma_error(ErrorKinds::InvalidBackend)),
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

    pub const fn supported_on_this_platform(self) -> bool {
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

    pub fn all_supported_on_this_platform() -> impl Iterator<Item = Backend> {
        Self::ALL
            .iter()
            .copied()
            .filter(|b| b.supported_on_this_platform())
    }
}

#[cfg(test)]
mod test {
    use crate::context::backend::Backend;

    // TODO: Create a reliable way to order them
    #[test]
    fn backends_test_supported_on_platform() {
        let backends = Backend::all_supported_on_this_platform();
        for b in backends {
            println!("{:?}", b);
        }
    }
}
