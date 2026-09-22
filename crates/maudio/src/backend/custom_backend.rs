//! Custom audio backends
//!
//! An audio backend connects maudio / miniaudio to the system or library
//! responsible for audio input and output. It discovers audio devices,
//! opens them, starts and stops playback or capture, and transfers audio
//! buffers between that system and maudio.
//!
//! By default, miniaudio’s built-in backends are being used. These should
//! be sufficient for most applications and cover most platforms. The custom
//! backend API is intended for cases where an application needs to provide
//! its own device implementation.
//!
//! For example, a custom backend can be used to:
//! - support an audio API or platform that miniaudio does not support;
//! - integrate with an existing audio framework, such as SDL;
//! - route audio through an application-owned device or audio subsystem;
//! - implement a virtual or specialized audio device;
//! - control device creation and lifetime more directly.
//!
//! The custom backend only handles device-level audio I/O. The rest of
//! the audio pipeline remains managed by maudio, including format conversion,
//! channel conversion, resampling, data sources, and engine or node-graph processing.
//!
//! A backend is implemented using the `CustomBackend` trait. The implementation
//! supplies backend-specific context and device state and handles operations
//! such as device enumeration, initialization, starting, stopping, and forwarding
//! audio callbacks to maudio.
//!
//! Writing a backend requires interacting with device lifetimes, real-time
//! callbacks,buffer formats, and potentially platform-specific APIs.
//! It is therefore a lower-level extension point than most of maudio.
use crate::{
    backend::custom_context::{BackendDeviceConfig, DeviceDescriptor},
    device::{
        custom_device::BackendDeviceHandle, device_id::DeviceId, device_info::DeviceInfo,
        device_type::DeviceType,
    },
    logging::LogRef,
    ErrorKinds, MaResult, MaudioError,
};

/// Trait that allows the implementation of a custom audio backend
///
/// A custom backend connects `maudio`'s device layer to an external audio
/// system, such as a platform API, an audio library, or a virtual device.
///
/// The backend is responsible for:
///
/// - initializing and storing backend-wide state in [`Self::Context`];
/// - opening playback and capture devices;
/// - starting and stopping those devices;
/// - forwarding audio buffers between the backend and `maudio`;
/// - optionally enumerating devices and reporting device information.
///
/// # Required functionality
///
/// A implementation of `CustomBackend` only needs at minimum, to
/// have the [`CustomBackend::init_context`] function implemented.
/// However, this only initializes a context, and brings little to no functionality.
///
/// A backend that supports playback or capture will normally implement at
/// least:
///
/// - [`Self::init_context`];
/// - [`Self::init_device`];
/// - [`Self::device_start`];
/// - [`Self::device_stop`].
///
/// During device initialization, the backend must also arrange for its audio
/// callback to forward buffers to `maudio` through
/// [`BackendDeviceHandle::handle_backend_data_callback`].
///
/// # Backend state
///
/// [`Self::Context`] contains state shared by devices created through the
/// backend. This commonly includes a handle to an initialized audio subsystem
/// and any state needed for device discovery / enumeration.
///
/// [`Self::Device`] contains the backend-specific resources associated with an
/// opened `maudio` device. Depending on [`DeviceType`], it may contain a
/// playback stream, a capture stream, or both.
///
/// The `'device` lifetime allows this state to retain a
/// [`BackendDeviceHandle`] for forwarding audio callbacks.
///
/// # Lifecycle
///
/// A backend is used approximately in the following order:
///
/// 1. [`Self::init_context`] initializes the audio subsystem.
/// 2. Device enumeration or information queries may be performed.
/// 3. [`Self::init_device`] opens the requested device streams.
/// 4. [`Self::device_start`] and [`Self::device_stop`] control processing.
/// 5. The device state is dropped.
/// 6. The context state is dropped after its devices are no longer in use.
pub trait CustomBackend {
    /// State shared by devices created through this backend.
    ///
    /// This usually owns the backend's audio subsystem or platform context.
    type Context;
    /// Backend-specific state for an initialized device.
    ///
    /// For duplex devices, this may contain separate playback and capture
    /// resources.
    type Device<'device>;

    /// Initializes the backend and returns its shared context state.
    ///
    /// This is called before any device operations are performed.
    fn init_context(log: Option<&LogRef>) -> MaResult<Self::Context>;

    /// Enumerates the playback and capture devices known to the backend.
    ///
    /// `report` should be called once for every discovered device. Enumeration
    /// must stop when it returns `false`.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`].
    fn enumerate_devices<F>(
        _context: &mut Self::Context,
        mut _f: F,
        _log: Option<&LogRef>,
    ) -> MaResult<()>
    where
        F: FnMut(DeviceType, &DeviceInfo) -> bool,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Queries information about a device previously identified by the
    /// backend.
    ///
    /// `device_type` identifies whether `device_id` refers to a playback or
    /// capture device.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`].
    fn context_query_device_info(
        _context: &mut Self::Context,
        _device_type: DeviceType,
        _device_id: DeviceId,
        _log: Option<&LogRef>,
    ) -> MaResult<DeviceInfo> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Opens and initializes the backend resources for a device.
    ///
    /// `playback` and `capture` are present according to the requested
    /// [`DeviceType`]. The backend may update these descriptors with the
    /// format, channel count, sample rate, and period configuration actually
    /// selected by the underlying audio system.
    ///
    /// The returned value is stored as this device's backend-specific state.
    ///
    /// The backend must arrange for audio buffers produced by its callback to
    /// be forwarded through
    /// [`BackendDeviceHandle::handle_backend_data_callback`].
    ///
    /// # Configuration and descriptors
    ///
    /// The function takes in 2 difference configurations:
    /// - [`BackendDeviceConfig`]
    /// - [`DeviceDescriptor`]
    ///
    /// At the start of the function, the information is identical in both.
    ///
    /// - [`BackendDeviceConfig`] contains device-wide configuration, such as the
    ///   device type, performance profile, requested sample rate, and period
    ///   configuration. This is information typically provided by the user
    ///   during to the [`DeviceBuilder`](crate::device::device_builder::DeviceBuilder)
    /// - [`DeviceDescriptor`] contains the configuration for one direction of the
    ///   device. Playback and capture have separate descriptors because they may
    ///   use different formats, channel counts, channel maps, or device IDs.
    ///
    /// Each descriptor is both an input and an output parameter.
    ///
    /// On entry, it describes the configuration requested by `maudio`. Some
    /// properties may be unspecified, allowing the backend to select an
    /// appropriate native value. The backend should attempt to match the requested
    /// configuration where practical.
    ///
    /// Each descriptor is both an input and an output parameter.
    ///
    /// On entry, it describes the configuration requested by `maudio`. Some
    /// properties may be unspecified, allowing the backend to select an
    /// appropriate native value. The backend should attempt to match the requested
    /// configuration where practical.
    ///
    /// `maudio` uses the resulting descriptors to configure conversion between the
    /// backend's native representation and the application-facing device
    /// configuration. This can include sample-format conversion, channel
    /// conversion, and resampling.
    ///
    /// The sample types passed to
    /// [`BackendDeviceHandle::handle_backend_data_callback`] must match the final
    /// formats reported by these descriptors.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`].
    fn init_device<'device>(
        _device: BackendDeviceHandle<'device, Self>,
        _config: BackendDeviceConfig,
        _playback: Option<&mut DeviceDescriptor>,
        _capture: Option<&mut DeviceDescriptor>,
        _log: Option<&LogRef>,
    ) -> MaResult<Self::Device<'device>>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Starts playback or capture on an initialized device.
    ///
    /// The backend-specific device state can be accessed through
    /// [`BackendDeviceHandle::backend_device`].
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`].
    fn device_start<'device>(
        _device: &BackendDeviceHandle<'device, Self>,
        _log: Option<&LogRef>,
    ) -> MaResult<()>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Stops playback or capture on an initialized device.
    ///
    /// The device should remain initialized and be capable of being started
    /// again.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`].
    fn device_stop<'device>(
        _device: &BackendDeviceHandle<'device, Self>,
        _log: Option<&LogRef>,
    ) -> MaResult<()>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Returns information about an initialized device.
    ///
    /// Unlike [`Self::context_query_device_info`], this method queries a device
    /// that has already been opened and may use its backend-specific device
    /// state.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`].
    fn device_get_info<'device>(
        _device: BackendDeviceHandle<'device, Self>,
        _context: &'device Self::Context,
        _device_type: DeviceType,
        _log: Option<&LogRef>,
    ) -> MaResult<DeviceInfo>
    where
        Self: Sized,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }
}
