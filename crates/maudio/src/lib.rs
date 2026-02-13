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
mod context; // not implemented
pub mod data_source;
mod device; // not implemented
pub mod engine;
pub mod pcm_frames;
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
struct MaError(pub sys::ma_result);

impl MaudioError {
    // Only used for checking error codes from C ffi functions
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

    /// Returns the wrapper-level error is present.
    pub fn is_kind(&self) -> bool {
        self.native.is_some()
    }

    /// Returns the wrapper-level error kind, if any.
    pub fn kind(&self) -> Option<&ErrorKinds> {
        self.native.as_ref()
    }

    /// Returns the underlying miniaudio result code.
    pub fn ma_result(&self) -> sys::ma_result {
        self.ma_result.0
    }

    fn from_ma_result(error: sys::ma_result) -> Self {
        Self {
            native: None,
            ma_result: MaError(error),
        }
    }

    fn new_ma_error(native: ErrorKinds) -> Self {
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

impl std::fmt::Display for MaudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.native {
            None => {
                write!(f, "{}", self.ma_result)
            }
            Some(kind) => {
                write!(f, "{kind}.")?;
                write!(f, " MA: ({})", self.ma_result)?;
                Ok(())
            }
        }
    }
}

impl std::fmt::Display for ErrorKinds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKinds::UnknownEnumValue { type_name, value } => {
                write!(f, "unknown {type_name} value: {value}")
            }
            ErrorKinds::BufferSizeMismatch {
                context,
                expected,
                actual,
            } => {
                if context.is_empty() {
                    write!(
                        f,
                        "buffer size mismatch (expected {expected}, got {actual})"
                    )
                } else {
                    write!(
                        f,
                        "{context}: buffer size mismatch (expected {expected}, got {actual})"
                    )
                }
            }
            ErrorKinds::IntegerOverflow { op, lhs, rhs } => {
                write!(f, "integer overflow while computing {op} ({lhs} * {rhs})")
            }
            ErrorKinds::S24OverFlow => {
                write!(f, "Overflow when converting S24 to miniaudio storage")
            }
            ErrorKinds::S24UnderFlow => {
                write!(f, "Underflow when converting S24 to miniaudio storage")
            }
            ErrorKinds::InvalidPackedSampleSize {
                bytes_per_sample,
                actual_len,
            } => {
                write!(f, "SampleBuffer<S24Packed> with invalid length {actual_len} % {bytes_per_sample} != 0")
            }
            ErrorKinds::WriteExceedsCapacity { capacity, written } => {
                write!(
                    f,
                    "Amount written exceds the capacity: {capacity}, written: {written}"
                )
            }
            ErrorKinds::ReadExceedsAvailability { available, read } => {
                write!(
                    f,
                    "Amount read exceds availability: {available}, read: {read}"
                )
            }
            ErrorKinds::InvalidGraphState => write!(f, "invalid graph state"),
            ErrorKinds::ChannelRecieveError => write!(f, "channel receive error"),
            ErrorKinds::ChannelSendError => write!(f, "channel send error"),
            ErrorKinds::InvalidFormat => write!(f, "invalid format"),
            ErrorKinds::InvalidCString => write!(f, "invalid C string"),
        }
    }
}

impl std::fmt::Display for MaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name(), self.0)
    }
}

impl std::fmt::Debug for MaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, ({})", self.name(), self.0)
    }
}

impl MaError {
    pub fn name(self) -> &'static str {
        match self.0 {
            sys::ma_result_MA_ERROR => "MA_ERROR",
            sys::ma_result_MA_INVALID_ARGS => "MA_INVALID_ARGS",
            sys::ma_result_MA_INVALID_OPERATION => "MA_INVALID_OPERATION",
            sys::ma_result_MA_OUT_OF_MEMORY => "MA_OUT_OF_MEMORY",
            sys::ma_result_MA_OUT_OF_RANGE => "MA_OUT_OF_RANGE",
            sys::ma_result_MA_ACCESS_DENIED => "MA_ACCESS_DENIED",
            sys::ma_result_MA_DOES_NOT_EXIST => "MA_DOES_NOT_EXIST",
            sys::ma_result_MA_ALREADY_EXISTS => "MA_ALREADY_EXISTS",
            sys::ma_result_MA_TOO_MANY_OPEN_FILES => "MA_TOO_MANY_OPEN_FILES",
            sys::ma_result_MA_INVALID_FILE => "MA_INVALID_FILE",
            sys::ma_result_MA_TOO_BIG => "MA_TOO_BIG",
            sys::ma_result_MA_PATH_TOO_LONG => "MA_PATH_TOO_LONG",
            sys::ma_result_MA_NAME_TOO_LONG => "MA_NAME_TOO_LONG",
            sys::ma_result_MA_NOT_DIRECTORY => "MA_NOT_DIRECTORY",
            sys::ma_result_MA_IS_DIRECTORY => "MA_IS_DIRECTORY",
            sys::ma_result_MA_DIRECTORY_NOT_EMPTY => "MA_DIRECTORY_NOT_EMPTY",
            sys::ma_result_MA_AT_END => "MA_AT_END",
            sys::ma_result_MA_NO_SPACE => "MA_NO_SPACE",
            sys::ma_result_MA_BUSY => "MA_BUSY",
            sys::ma_result_MA_IO_ERROR => "MA_IO_ERROR",
            sys::ma_result_MA_INTERRUPT => "MA_INTERRUPT",
            sys::ma_result_MA_UNAVAILABLE => "MA_UNAVAILABLE",
            sys::ma_result_MA_ALREADY_IN_USE => "MA_ALREADY_IN_USE",
            sys::ma_result_MA_BAD_ADDRESS => "MA_BAD_ADDRESS",
            sys::ma_result_MA_BAD_SEEK => "MA_BAD_SEEK",
            sys::ma_result_MA_BAD_PIPE => "MA_BAD_PIPE",
            sys::ma_result_MA_DEADLOCK => "MA_DEADLOCK",
            sys::ma_result_MA_TOO_MANY_LINKS => "MA_TOO_MANY_LINKS",
            sys::ma_result_MA_NOT_IMPLEMENTED => "MA_NOT_IMPLEMENTED",
            sys::ma_result_MA_NO_MESSAGE => "MA_NO_MESSAGE",
            sys::ma_result_MA_BAD_MESSAGE => "MA_BAD_MESSAGE",
            sys::ma_result_MA_NO_DATA_AVAILABLE => "MA_NO_DATA_AVAILABLE",
            sys::ma_result_MA_INVALID_DATA => "MA_INVALID_DATA",
            sys::ma_result_MA_TIMEOUT => "MA_TIMEOUT",
            sys::ma_result_MA_NO_NETWORK => "MA_NO_NETWORK",
            sys::ma_result_MA_NOT_UNIQUE => "MA_NOT_UNIQUE",
            sys::ma_result_MA_NOT_SOCKET => "MA_NOT_SOCKET",
            sys::ma_result_MA_NO_ADDRESS => "MA_NO_ADDRESS",
            sys::ma_result_MA_BAD_PROTOCOL => "MA_BAD_PROTOCOL",
            sys::ma_result_MA_PROTOCOL_UNAVAILABLE => "MA_PROTOCOL_UNAVAILABLE",
            sys::ma_result_MA_PROTOCOL_NOT_SUPPORTED => "MA_PROTOCOL_NOT_SUPPORTED",
            sys::ma_result_MA_PROTOCOL_FAMILY_NOT_SUPPORTED => "MA_PROTOCOL_FAMILY_NOT_SUPPORTED",
            sys::ma_result_MA_ADDRESS_FAMILY_NOT_SUPPORTED => "MA_ADDRESS_FAMILY_NOT_SUPPORTED",
            sys::ma_result_MA_SOCKET_NOT_SUPPORTED => "MA_SOCKET_NOT_SUPPORTED",
            sys::ma_result_MA_CONNECTION_RESET => "MA_CONNECTION_RESET",
            sys::ma_result_MA_ALREADY_CONNECTED => "MA_ALREADY_CONNECTED",
            sys::ma_result_MA_NOT_CONNECTED => "MA_NOT_CONNECTED",
            sys::ma_result_MA_CONNECTION_REFUSED => "MA_CONNECTION_REFUSED",
            sys::ma_result_MA_NO_HOST => "MA_NO_HOST",
            sys::ma_result_MA_IN_PROGRESS => "MA_IN_PROGRESS",
            sys::ma_result_MA_CANCELLED => "MA_CANCELLED",
            sys::ma_result_MA_MEMORY_ALREADY_MAPPED => "MemoryAlreadyMapped",
            // General non-standard errors.
            sys::ma_result_MA_CRC_MISMATCH => "MA_CRC_MISMATCH",
            // General miniaudio-specific errors.
            sys::ma_result_MA_FORMAT_NOT_SUPPORTED => "MA_FORMAT_NOT_SUPPORTED",
            sys::ma_result_MA_DEVICE_TYPE_NOT_SUPPORTED => "MA_DEVICE_TYPE_NOT_SUPPORTED",
            sys::ma_result_MA_SHARE_MODE_NOT_SUPPORTED => "MA_SHARE_MODE_NOT_SUPPORTED",
            sys::ma_result_MA_NO_BACKEND => "MA_NO_BACKEND",
            sys::ma_result_MA_NO_DEVICE => "MA_NO_DEVICE",
            sys::ma_result_MA_API_NOT_FOUND => "MA_API_NOT_FOUND",
            sys::ma_result_MA_INVALID_DEVICE_CONFIG => "MA_INVALID_DEVICE_CONFIG",
            sys::ma_result_MA_LOOP => "MA_LOOP",
            sys::ma_result_MA_BACKEND_NOT_ENABLED => "MA_BACKEND_NOT_ENABLED",
            // State errors.
            sys::ma_result_MA_DEVICE_NOT_INITIALIZED => "MA_DEVICE_NOT_INITIALIZED",
            sys::ma_result_MA_DEVICE_ALREADY_INITIALIZED => "MA_DEVICE_ALREADY_INITIALIZED",
            sys::ma_result_MA_DEVICE_NOT_STARTED => "MA_DEVICE_NOT_STARTED",
            sys::ma_result_MA_DEVICE_NOT_STOPPED => "MA_DEVICE_NOT_STOPPED",
            // Operation errors.
            sys::ma_result_MA_FAILED_TO_INIT_BACKEND => "MA_FAILED_TO_INIT_BACKEND",
            sys::ma_result_MA_FAILED_TO_OPEN_BACKEND_DEVICE => "MA_FAILED_TO_OPEN_BACKEND_DEVICE",
            sys::ma_result_MA_FAILED_TO_START_BACKEND_DEVICE => "MA_FAILED_TO_START_BACKEND_DEVICE",
            sys::ma_result_MA_FAILED_TO_STOP_BACKEND_DEVICE => "MA_FAILED_TO_STOP_BACKEND_DEVICE",
            _ => "UNKNOWN_MA_ERROR",
        }
    }
}

impl ErrorKinds {
    #[inline]
    pub fn unknown_enum<T>(raw: i64) -> Self {
        Self::UnknownEnumValue {
            type_name: core::any::type_name::<T>(),
            value: raw,
        }
    }
}

/// Wrapper-level error kinds.
///
/// These errors are generated by the maudio crate itself, typically when:
///
/// - Converting raw miniaudio values into safe Rust types
/// - Validating buffer sizes or invariants
/// - Detecting arithmetic overflow
///
/// Miniaudio-native errors are represented separately by `MA_RESULT`.
#[derive(Debug)]
#[non_exhaustive]
pub enum ErrorKinds {
    // Error converting a raw value to an enum variant
    UnknownEnumValue {
        type_name: &'static str,
        value: i64,
    },
    // data.len() != expected
    BufferSizeMismatch {
        context: &'static str,
        expected: usize,
        actual: usize,
    },
    // checked_mul error
    IntegerOverflow {
        op: &'static str, // "frames * channels"
        lhs: u64,
        rhs: u64,
    },
    InvalidPackedSampleSize {
        bytes_per_sample: usize, // 3
        actual_len: usize,
    },
    WriteExceedsCapacity {
        capacity: usize,
        written: usize,
    },
    ReadExceedsAvailability {
        available: usize,
        read: usize,
    },
    S24OverFlow,
    S24UnderFlow,
    InvalidGraphState,
    /// Used by Handle types. Error during a recv
    ChannelRecieveError,
    /// Used by Handle types. Error during a send
    ChannelSendError,
    /// TryFrom error converting raw miniaudio value to Maudio
    InvalidFormat,
    /// Error coverting Path to CString
    InvalidCString,
}

/// Error type returned by the maudio crate.
///
/// `MaudioError` can originate from two sources:
///
/// - **Miniaudio errors**, represented by an underlying `MA_RESULT`.
/// - **Wrapper-level errors**, produced by additional validation and safety
///   checks performed by this crate.
///
/// When `kind()` is `None`, the error originates directly from miniaudio.
///
/// When `Some`, the error was produced by the wrapper and may include an
/// associated miniaudio result for context. In this case, ma_result will be `MA_ERROR (-1)`.
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
