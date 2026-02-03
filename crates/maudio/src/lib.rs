//! `maudio` is an audio library built on top of miniaudio, providing both a
//! high-level playback-focused API and the foundation for a more flexible
//! low-level interface.
//!
//! At the high level, audio is driven through an `Engine`, which offers a
//! simple and ergonomic way to play sounds without requiring manual audio
//! processing or buffer management.
//!
//! The `Engine` is designed primarily for playback. It does not currently
//! support recording, loopback, or full duplex operation, and it intentionally
//! hides much of the complexity exposed by the low-level API. A lower-level,
//! more flexible interface is planned and under active development.
//!
//! Internally, an `Engine` owns a `NodeGraph`, which represents a directed graph
//! of audio processing units called `Nodes`. Nodes can act as audio sources
//! (such as sounds or waveforms), processing units (DSP, filters, splitters),
//! or endpoints. Audio flows through the graph from source nodes, through
//! optional processing stages, and finally into an output endpoint.
//!
//! By default, sounds created from an `Engine` are automatically connected to
//! the graph’s endpoint and played in a push-based manner. Audio generation,
//! mixing, and playback are handled internally by the engine, so users do not
//! need to manually pull or read audio data.
//!
//! While basic playback can be achieved without interacting directly with the
//! `NodeGraph`, more advanced setups allow nodes to be explicitly connected,
//! reordered, or routed through custom processing chains.
//!
//! Most types in `maudio` are constructed using a builder pattern, enabling
//! additional configuration at creation time while keeping common use cases
//! straightforward.
//! # Feature flags
//!
//! This crate builds and links the vendored **miniaudio** C library and exposes raw FFI bindings.
//!
//! ## `vorbis`
//! Enables Ogg/Vorbis decoding by compiling the `stb_vorbis` implementation into the miniaudio
//! translation unit.
//!
//! - Vorbis `.ogg` files can be decoded via miniaudio's decoding APIs.
//!
//! ## `generate-bindings`
//! Generates bindings at build time using `bindgen`.
//!
//! - Required on MacOS
//! - Intended for maintainers when updating the vendored miniaudio version.
//! - Regular users should prefer the pre-generated bindings shipped with the crate.
//! - Adds a build dependency on clang/libclang via `bindgen`.
#![allow(dead_code)]

pub mod audio;
mod context;
pub mod data_source;
mod device;
pub mod engine;
pub mod sound;
pub mod util;

#[doc(hidden)]
pub extern crate maudio_sys;

use maudio_sys::ffi as sys;

pub(crate) trait Binding: Sized {
    type Raw;

    /// Construct the wrapper from a raw FFI handle.
    fn from_ptr(raw: Self::Raw) -> Self;

    fn to_raw(&self) -> Self::Raw;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MaError(pub sys::ma_result);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaRawResult;

impl MaRawResult {
    fn check(res: i32) -> MaResult<()> {
        if res == sys::ma_result_MA_SUCCESS {
            Ok(())
        } else {
            Err(MaudioError {
                native: None,
                ma_result: MaError(res as sys::ma_result),
            })
        }
    }
}

impl MaudioError {
    pub(crate) fn from_ma_result(error: sys::ma_result) -> Self {
        Self {
            native: None,
            ma_result: MaError(error),
        }
    }

    pub(crate) fn new_ma_error(native: ErrorKinds) -> Self {
        Self {
            native: Some(native),
            ma_result: MaError(sys::ma_result_MA_ERROR),
        }
    }
}

impl PartialEq<MaError> for MaudioError {
    fn eq(&self, other: &MaError) -> bool {
        self.ma_result.0.eq(&other.0)
    }
}

impl std::fmt::Display for MaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MaError: {} ({})", self.name(), self.0)
    }
}

impl std::fmt::Debug for MaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MaError({}, {})", self.name(), self.0)
    }
}

impl MaError {
    pub fn name(self) -> &'static str {
        match self.0 {
            sys::ma_result_MA_ERROR => "MiniaudioError",
            sys::ma_result_MA_INVALID_ARGS => "InvalidArgs",
            sys::ma_result_MA_INVALID_OPERATION => "InvalidOperation",
            sys::ma_result_MA_OUT_OF_MEMORY => "OutOfMemory",
            sys::ma_result_MA_OUT_OF_RANGE => "OutOfRange",
            sys::ma_result_MA_ACCESS_DENIED => "AccessDenied",
            sys::ma_result_MA_DOES_NOT_EXIST => "DoesNotExist",
            sys::ma_result_MA_ALREADY_EXISTS => "AlreadyExists",
            sys::ma_result_MA_TOO_MANY_OPEN_FILES => "TooManyOpenFiles",
            sys::ma_result_MA_INVALID_FILE => "InvalidFile",
            sys::ma_result_MA_TOO_BIG => "TooBig",
            sys::ma_result_MA_PATH_TOO_LONG => "PathTooLong",
            sys::ma_result_MA_NAME_TOO_LONG => "NameTooLong",
            sys::ma_result_MA_NOT_DIRECTORY => "NotDirectory",
            sys::ma_result_MA_IS_DIRECTORY => "IsDirectory",
            sys::ma_result_MA_DIRECTORY_NOT_EMPTY => "DirectoryNotEmpty",
            sys::ma_result_MA_AT_END => "AtEnd",
            sys::ma_result_MA_NO_SPACE => "NoSpace",
            sys::ma_result_MA_BUSY => "Busy",
            sys::ma_result_MA_IO_ERROR => "IoError",
            sys::ma_result_MA_INTERRUPT => "Interrupt",
            sys::ma_result_MA_UNAVAILABLE => "Unavailable",
            sys::ma_result_MA_ALREADY_IN_USE => "AlreadyInUse",
            sys::ma_result_MA_BAD_ADDRESS => "BadAddress",
            sys::ma_result_MA_BAD_SEEK => "BadSeek",
            sys::ma_result_MA_BAD_PIPE => "BadPipe",
            sys::ma_result_MA_DEADLOCK => "Deadlock",
            sys::ma_result_MA_TOO_MANY_LINKS => "TooManyLinks",
            sys::ma_result_MA_NOT_IMPLEMENTED => "NotImplemented",
            sys::ma_result_MA_NO_MESSAGE => "NoMessage",
            sys::ma_result_MA_BAD_MESSAGE => "BadMessage",
            sys::ma_result_MA_NO_DATA_AVAILABLE => "NoDataAvailable",
            sys::ma_result_MA_INVALID_DATA => "InvalidData",
            sys::ma_result_MA_TIMEOUT => "Timeout",
            sys::ma_result_MA_NO_NETWORK => "NoNetwork",
            sys::ma_result_MA_NOT_UNIQUE => "NotUnique",
            sys::ma_result_MA_NOT_SOCKET => "NotSocket",
            sys::ma_result_MA_NO_ADDRESS => "NoAddress",
            sys::ma_result_MA_BAD_PROTOCOL => "BadProtocol",
            sys::ma_result_MA_PROTOCOL_UNAVAILABLE => "ProtocolUnavailable",
            sys::ma_result_MA_PROTOCOL_NOT_SUPPORTED => "ProtocolNotSupported",
            sys::ma_result_MA_PROTOCOL_FAMILY_NOT_SUPPORTED => "ProtocolFamilyNotSupported",
            sys::ma_result_MA_ADDRESS_FAMILY_NOT_SUPPORTED => "AddressFamilyNotSupported",
            sys::ma_result_MA_SOCKET_NOT_SUPPORTED => "SocketNotSupported",
            sys::ma_result_MA_CONNECTION_RESET => "ConnectionReset",
            sys::ma_result_MA_ALREADY_CONNECTED => "AlreadyConnected",
            sys::ma_result_MA_NOT_CONNECTED => "NotConnected",
            sys::ma_result_MA_CONNECTION_REFUSED => "ConnectionRefused",
            sys::ma_result_MA_NO_HOST => "NoHost",
            sys::ma_result_MA_IN_PROGRESS => "InProgress",
            sys::ma_result_MA_CANCELLED => "Cancelled",
            sys::ma_result_MA_MEMORY_ALREADY_MAPPED => "MemoryAlreadyMapped",
            // General non-standard errors.
            sys::ma_result_MA_CRC_MISMATCH => "CrcMismatch",
            // General miniaudio-specific errors.
            sys::ma_result_MA_FORMAT_NOT_SUPPORTED => "FormatNotSupported",
            sys::ma_result_MA_DEVICE_TYPE_NOT_SUPPORTED => "DeviceTypeNotSupported",
            sys::ma_result_MA_SHARE_MODE_NOT_SUPPORTED => "ShareModeNotSupported",
            sys::ma_result_MA_NO_BACKEND => "NoBackend",
            sys::ma_result_MA_NO_DEVICE => "NoDevice",
            sys::ma_result_MA_API_NOT_FOUND => "ApiNotFound",
            sys::ma_result_MA_INVALID_DEVICE_CONFIG => "InvalidDeviceConfig",
            sys::ma_result_MA_LOOP => "Loop",
            sys::ma_result_MA_BACKEND_NOT_ENABLED => "BackendNotEnabled",
            // State errors.
            sys::ma_result_MA_DEVICE_NOT_INITIALIZED => "DeviceNotInitialized",
            sys::ma_result_MA_DEVICE_ALREADY_INITIALIZED => "DeviceAlreadyInitialized",
            sys::ma_result_MA_DEVICE_NOT_STARTED => "DeviceNotStarted",
            sys::ma_result_MA_DEVICE_NOT_STOPPED => "DeviceNotStopped",
            // Operation errors.
            sys::ma_result_MA_FAILED_TO_INIT_BACKEND => "FailedToInitBackend",
            sys::ma_result_MA_FAILED_TO_OPEN_BACKEND_DEVICE => "FailedToOpenBackendDevice",
            sys::ma_result_MA_FAILED_TO_START_BACKEND_DEVICE => "FailedToStartBackendDevice",
            sys::ma_result_MA_FAILED_TO_STOP_BACKEND_DEVICE => "FailedToStopBackendDevice",
            _ => "UNKNOWN_MA_ERROR",
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ErrorKinds {
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidChannelPosition,
    InvalidGraphState,
    /// Size mismatch between data.len() and frames * channels
    BufferSizeError,
    /// Used by Handle types. Error during a recv
    ChannelRecieveError,
    /// Used by Handle types. Erro during a send
    ChannelSendError,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidSWaveFormType,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidSampleRate,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidBackend,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidPerformanceProfile,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidDither,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidFormat,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidChannelMap,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidChannelMixMode,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidAttenuationModel,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidHandedness,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidStreamLayout,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidPanMode,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidStreamFormat,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidNodeState,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidPositioning,
    /// Error coverting Path to CString
    InvalidCString,
}

#[derive(Debug)]
pub struct MaudioError {
    native: Option<ErrorKinds>,
    ma_result: MaError,
}

pub type MaResult<T> = std::result::Result<T, MaudioError>;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn context_init_error_is_readable() {
        let err = MaError(sys::ma_result_MA_INVALID_ARGS);
        assert!(err.to_string().contains("InvalidArgs"));

        let err = MaError(sys::ma_result_MA_BAD_MESSAGE);
        assert!(err.to_string().contains("BadMessage"));

        let err = MaError(sys::ma_result_MA_PROTOCOL_NOT_SUPPORTED);
        assert!(err.to_string().contains("ProtocolNotSupported"));

        let err = MaError(sys::ma_result_MA_INVALID_FILE);
        assert!(err.to_string().contains("InvalidFile"));

        let err = MaError(sys::ma_result_MA_OUT_OF_MEMORY);
        assert!(err.to_string().contains("OutOfMemory"));
    }
}
