#![allow(dead_code)]

pub mod audio;
pub mod context;
pub mod device;
pub mod engine;
pub mod sound;
pub mod util;

use maudio_sys::ffi as sys;

pub(crate) trait Binding: Sized {
    type Raw;

    /// Construct the wrapper from a raw FFI handle.
    fn from_ptr(raw: Self::Raw) -> Self;

    fn to_raw(&self) -> Self::Raw;
}

pub enum LogLevel {}

pub type Result<T> = std::result::Result<T, MaError>;

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
    pub(crate) fn new_ma_error(native: ErrorKinds) -> Self {
        Self { native: Some(native), ma_result: MaError(sys::ma_result_MA_ERROR) }
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
    InvalidSampleRate,
    InvalidBackend,
    InvalidPerformanceProfile,
    InvalidDither,
    InvalidFormat,
    InvalidChannelMap,
    InvalidChannelMixMode,
    InvalidAttenuationModel,
    InvalidHandedness,
    InvalidStreamLayout,
    InvalidPanMode,
    InvalidStreamFormat,
    InvalidNodeState,
    InvalidPositioning,
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
