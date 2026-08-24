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
//! Under the hood, the engine consists of:
//! - **ResourceManager**: It is responsible for loading sounds into memory or streaming them.
//!   It is also responsible for refence counting them to avoid loading sounds into memory multiple times.
//!   It also has a **Decoder** and can decode audio either before or after it is loaded into memory.
//! - **NodeGraph**: It is a directed graph of audio processing units called Nodes.
//!   Nodes can be audio sources (such as sounds or waveforms), processing units (DSP, filters, splitters), or endpoints. Audio data flows through the graph from source nodes, through optional processing nodes, and finally into the endpoint.
//! - **Device**: An abstraction of a physical device. Represents the audio playback device and is
//!   responsible for driving the engine. Internally, it runs a callback on a dedicated audio thread,
//!   which continuously requests (pulls) audio frames from the engine.
//!   The engine, in turn, processes the node graph to produce the requested audio data.
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
//!
//! In addition to the high level `Engine` API, `maudio` exposes a low level interface for working directly with the core audio building blocks.
//! While the high level API provides a ready-to-use playback system, the low level API gives you the components needed to build your own.
//! This includes manual control over devices, audio graphs, data sources, decoding, and resource management.
//!
//! The two APIs are closely related: the high level engine is built using many of the same concepts exposed by the low level API,
//! but organizes them into a simpler, playback-focused workflow.
//!
//! The low level API includes:
//!
//! - **Context** for initializing the audio backend and enumerating devices.
//! - **Device** for creating playback, capture, loopback, or duplex streams with direct control over the audio callback.
//! - **Decoder** for reading audio from encoded formats.
//! - **Data sources** as a unified interface for producing PCM frames.
//! - **Audio buffers** for working with decoded PCM data in memory.
//! - **Utility primitives** such as ring buffers, fences, and notification systems for real-time and asynchronous coordination.
//!
//! Use the low level API when you need full control over how audio is generated, processed, or delivered, or when building abstractions on top of `maudio`.
//!
//! # Feature flags
//!
//! This crate builds and links the vendored **miniaudio** C library and exposes raw FFI bindings.
//!
//!
//! ## Engine
//!
//! The Engine (and the high level API) is enabled by default. It can be disabled when building the crate
//! with `--no-default-features`, or when importing the library using
//! `default-features = false`:
//!
//! ```toml
//! maudio = { version = "...", default-features = false }
//! ```
//!
//! Disabling the Engine can reduce binary size and memory usage on platforms
//! where the Engine is not needed.
//!
//! When supplying your own static libraries (via the `supplybin` feature),
//! this should be paired with the `MA_NO_ENGINE`, `MA_NO_NODE_GRAPH` and `MA_NO_RESOURCE_MANAGER`
//! compile-time flags when compiling miniaudio.c
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
//! - Intended for maintainers when updating the vendored miniaudio version.
//! - Regular users should prefer the pre-generated bindings shipped with the crate.
//! - Adds a build dependency on via `bindgen`.
//!
//! ## `worklet` (Audioworklet - emscripten)
//!
//! The worklets feature enables AudioWorklet support when building for Emscripten.
//! AudioWorklets allow audio processing to run in a dedicated audio thread,
//! which can provide more reliable, low-latency audio processing in the browser.
//!
//! Only works for target `wasm32-unknown-emscripten`.

#[cfg(feature = "use-global-allocator")]
pub(crate) mod alloc_api;

pub mod audio;
pub mod backend;
pub mod context;
pub mod data_source;
pub mod device;
pub mod encoder;
#[cfg(feature = "engine")]
pub mod engine;
pub mod pcm_frames;
#[cfg(feature = "engine")]
pub mod sound;
pub(crate) mod test_assets;
pub mod util;

#[doc(hidden)]
pub extern crate maudio_sys;

use std::num::TryFromIntError;

use maudio_sys::ffi as sys;

#[cfg(feature = "use-global-allocator")]
use crate::alloc_api::ma_global_allocation_callbacks;

/// IMPORTANT: type Raw must be a *mut pointer
pub(crate) trait Binding: Sized {
    type Raw;

    fn to_raw(&self) -> Self::Raw;
}

// Used instead of impl Binding when type raw is a C struct (not a pointer)
// Example- inner: sys::ma_waveform_config
// Calling the Binding trait as &config.to_raw() as *const _ would not be safe.
pub(crate) trait AsRawRef {
    type Raw;

    fn as_raw(&self) -> &Self::Raw;

    #[inline]
    fn as_raw_ptr(&self) -> *const Self::Raw {
        self.as_raw() as *const _
    }
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

    /// Returns true if error is miniaudio error MA_RESULT_MA_BUSY
    pub fn is_busy(&self) -> bool {
        let a = self.ma_result;
        a.name() == "MA_BUSY"
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

    /// Construct a new `MaudioError`
    ///
    /// Use ErrorKinds::Other for a generic error
    pub fn new_ma_error(native: ErrorKinds) -> Self {
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
            ErrorKinds::IntegerOverflow { op } => {
                write!(f, "integer overflow while computing {op})")
            }
            ErrorKinds::InvalidDecodedDataLength => {
                write!(f, "Decoded data does not contain a whole number of frames")
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
            ErrorKinds::InvalidFormat => write!(f, "invalid format"),
            ErrorKinds::InvalidCString => write!(f, "invalid C string"),
            ErrorKinds::IoError { err } => write!(f, "IO error: {err}"),
            ErrorKinds::IntegerError { err } => write!(f, "Integer error: {err}"),
            ErrorKinds::InvalidOperation(error) => write!(f, "{error}",),
            ErrorKinds::Other(error) => write!(f, "{error}",),
            ErrorKinds::NotImplemented => write!(f, "Not implemented"),
            ErrorKinds::ReaderExists => write!(f, "Reader already exists"),
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
    pub(crate) fn unknown_enum<T>(raw: i64) -> Self {
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    InvalidDecodedDataLength,
    S24OverFlow,
    S24UnderFlow,
    InvalidGraphState,
    InvalidFormat,
    /// Error coverting Path to CString
    InvalidCString,
    InvalidOperation(&'static str),
    Other(String),
    IoError {
        err: std::io::ErrorKind,
    },
    IntegerError {
        err: std::num::TryFromIntError,
    },
    NotImplemented,
    ReaderExists,
}

impl std::error::Error for MaudioError {}

impl From<std::io::Error> for MaudioError {
    fn from(value: std::io::Error) -> Self {
        Self {
            native: Some(ErrorKinds::IoError { err: value.kind() }),
            ma_result: MaError(sys::ma_result_MA_ERROR),
        }
    }
}

impl From<TryFromIntError> for MaudioError {
    fn from(value: TryFromIntError) -> Self {
        Self {
            native: Some(ErrorKinds::IntegerError { err: value }),
            ma_result: MaError(sys::ma_result_MA_ERROR),
        }
    }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaudioError {
    native: Option<ErrorKinds>,
    ma_result: MaError,
}

pub type MaResult<T> = std::result::Result<T, MaudioError>;

#[cfg(not(windows))]
pub(crate) fn cstring_from_path(path: &std::path::Path) -> MaResult<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| crate::MaudioError::new_ma_error(crate::ErrorKinds::InvalidCString))
}

#[cfg(windows)]
pub(crate) fn wide_null_terminated(path: &std::path::Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
pub(crate) fn wide_null_terminated_name(name: &str) -> Vec<u16> {
    use std::os::windows::prelude::OsStrExt;

    std::ffi::OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Custom memory allocation callbacks for miniaudio.
///
/// Miniaudio allows callers to override how heap memory is allocated and freed
/// by providing a `ma_allocation_callbacks` struct (malloc/realloc/free + user data).
///
/// Types such as `NodeGraph` may accept these callbacks at initialization time.
/// If callbacks are not provided, miniaudio uses its default allocator
/// (typically the system allocator).
///
/// Custom allocators are currently not implemented.
pub(crate) struct AllocationCallbacks(pub(crate) sys::ma_allocation_callbacks);

unsafe impl Send for AllocationCallbacks {}
unsafe impl Sync for AllocationCallbacks {}

impl AllocationCallbacks {
    #[allow(unused)]
    pub(crate) fn cb_ptr() -> *const sys::ma_allocation_callbacks {
        Self::get().map_or(std::ptr::null_mut(), |a| &a.0 as *const _)
    }

    pub(crate) fn clone_callbacks() -> Option<sys::ma_allocation_callbacks> {
        Self::get().map(|a| a.0)
    }

    #[inline]
    #[cfg(feature = "use-global-allocator")]
    fn get() -> Option<&'static AllocationCallbacks> {
        use crate::alloc_api::GLOBAL_ALLOC;

        Some(GLOBAL_ALLOC.get_or_init(|| ma_global_allocation_callbacks()))
    }

    #[inline]
    #[cfg(not(feature = "use-global-allocator"))]
    fn get() -> Option<&'static AllocationCallbacks> {
        None
    }
}

impl AsRawRef for AllocationCallbacks {
    type Raw = sys::ma_allocation_callbacks;

    fn as_raw(&self) -> &Self::Raw {
        &self.0
    }
}

#[cfg(feature = "external-lib")]
pub mod external_build {
    const MINIAUDIO_VERSION: &str = "0.11.23";

    pub fn link_external(root: impl AsRef<std::path::Path>) {
        let root = std::path::PathBuf::from(root.as_ref());

        let target =
            std::env::var("TARGET").expect("Cargo did not provide the TARGET environment variable");

        let ver_folder = root.join(format!("miniaudio-{MINIAUDIO_VERSION}"));
        if !ver_folder.is_dir() {
            panic!(
                "no compatible libraries were found for version `{}`",
                ver_folder.display()
            );
        }

        let lib_dir = ver_folder.join(&target);

        let filename = if target.contains("windows-msvc") {
            "miniaudio.lib"
        } else {
            "libminiaudio.a"
        };

        let library = lib_dir.join(filename);

        if !library.is_file() {
            panic!(
                "could not find the precompiled miniaudio library for target \
                `{target}` at `{}`",
                library.display()
            );
        }

        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=static=miniaudio");
    }
}

#[cfg(test)]
mod test {
    use crate::MaudioError;

    #[test]
    fn test_maudioerror_is_busy() {
        use maudio_sys::ffi as sys;

        let err = MaudioError::from_ma_result(sys::ma_result_MA_BUSY);
        assert!(err.is_busy());
    }
}
