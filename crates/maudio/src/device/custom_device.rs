//! Custom backend device handle
use std::sync::OnceLock;

use maudio_sys::ffi as sys;

use crate::{
    backend::custom_backend::CustomBackend, device::device_ffi, pcm_frames::MaSampleFormat,
    Binding, MaResult,
};

/// A handle used by a [`CustomBackend`] to interact with an initialized
/// `maudio` device.
///
/// The handle provides access to the backend's context and device state. It
/// also provides the bridge through which the backend forwards playback and
/// capture buffers to `maudio`.
///
/// A backend will typically store a clone of this handle
///
/// # Forwarding audio data
///
/// The underlying audio API will normally invoke a callback whenever it needs
/// more playback data or has new captured data available. From that callback,
/// the backend must call
/// [`BackendDeviceHandle::handle_backend_data_callback`].
///
/// For a playback device, the backend passes a mutable output buffer. `maudio`
/// fills this buffer by running the device callback or engine:
///
/// ```ignore
/// device.handle_backend_data_callback::<f32, f32>(
///     Some(output),
///     None,
/// )?;
/// ```
///
/// For a capture device, the backend passes the captured input buffer:
///
/// ```ignore
/// device.handle_backend_data_callback::<f32, f32>(
///     None,
///     Some(input),
/// )?;
/// ```
///
/// A duplex backend passes both buffers in the same call:
///
/// ```ignore
/// device.handle_backend_data_callback::<f32, f32>(
///     Some(output),
///     Some(input),
/// )?;
/// ```
///
/// The buffer sample types must match the formats selected by the backend and
/// reported through the corresponding [`DeviceDescriptor`](crate::backend::custom_context::DeviceDescriptor) values during
/// [`CustomBackend::init_device`].
///
/// # Lifetime
///
/// This handle is valid only for the lifetime of the custom device. The
/// `'device` lifetime allows it to be stored in [`CustomBackend::Device`] or
/// in an audio callback while preventing it from outliving the associated
/// `maudio` device.
pub struct BackendDeviceHandle<'device, B: CustomBackend> {
    pub(crate) inner: *mut sys::ma_device,
    pub(crate) backend_device: &'device OnceLock<B::Device<'device>>,
    pub(crate) backend_context: &'device OnceLock<B::Context>,
}

impl<'device, B: CustomBackend> Clone for BackendDeviceHandle<'device, B> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner,
            backend_device: self.backend_device,
            backend_context: self.backend_context,
        }
    }
}

impl<'device, B: CustomBackend> Binding for BackendDeviceHandle<'device, B> {
    type Raw = *mut sys::ma_device;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

unsafe impl<'device, B: CustomBackend> Send for BackendDeviceHandle<'device, B> {}

impl<'device, B: CustomBackend> BackendDeviceHandle<'device, B> {
    /// Clones this handle for use in an API that requires `'static`.
    ///
    /// SAFETY:
    /// The returned handle does not keep the underlying `ma_device` alive.
    ///
    /// The caller must ensure that:
    /// - The callback registered with the custom backend is destroyed / unregisted.
    /// - This handle does not outlive the maudio Device or Engine
    ///
    /// ### More notes
    /// The maudio `Device` or `Engine` owns the custom backend and drop it.
    /// However, droping the backend may not not necessarily destroy it (if ref counted)
    /// or it may not unregister callbacks it registered with an external API automatically.
    /// An implicit `unregister_callback` may be necessary.
    ///
    /// The backend stores the device handle in its device state so it can call
    /// maudio's audio callback. The `'device` lifetime prevents that handle from
    /// escaping into longer-lived storage after the maudio `Device` or `Engine` that
    /// owns it is destroyed. Using the handle after that point would access an
    /// invalid `ma_device` and may cause undefined behavior.
    ///
    /// If the backend API's has a `Send + 'static` requirement, this will be useful.
    pub unsafe fn clone_static_unchecked(&self) -> BackendDeviceHandle<'static, B> {
        unsafe {
            std::mem::transmute::<BackendDeviceHandle<'device, B>, BackendDeviceHandle<'static, B>>(
                self.clone(),
            )
        }
    }

    /// Provides a reference to the [`CustomBackend::Device`]
    pub fn backend_device(&self) -> Option<&B::Device<'device>> {
        self.backend_device.get()
    }

    /// Forwards audio buffers from a callback-driven backend to `maudio`.
    ///
    /// This method is intended for backends whose underlying audio API delivers
    /// data through a callback. The backend should call it whenever that API
    /// requests playback data or provides captured audio.
    ///
    /// Calling this method runs `maudio`'s device processing synchronously:
    ///
    /// - playback data is written into `output`;
    /// - captured data is read from `input`;
    /// - a duplex device may provide both buffers.
    ///
    /// Pass `None` for buffers that do not apply to the device type.
    ///
    /// `F` and `R` must match the native playback and capture formats reported by
    /// the backend through the corresponding [`DeviceDescriptor`](crate::backend::custom_context::DeviceDescriptor) values.
    ///
    /// # Data-delivery model
    ///
    /// This uses miniaudio's callback-driven data-delivery model. The callback may
    /// be invoked asynchronously by the underlying audio system, but this method
    /// itself is synchronous.
    ///
    /// Miniaudio also supports blocking read/write backends and backends that
    /// provide their own audio-thread data loop. Those data-delivery models are
    /// not currently exposed by `maudio`'s custom backend API.
    ///
    /// # Real-time safety
    ///
    /// This method is normally called from an audio thread. Code executed by the
    /// device callback should avoid blocking, allocation, and other operations
    /// unsuitable for real-time audio processing.
    pub fn handle_backend_data_callback<F: MaSampleFormat, R: MaSampleFormat>(
        &self,
        output: Option<&mut [F::StorageUnit]>,
        input: Option<&[R::StorageUnit]>,
    ) -> MaResult<()> {
        device_ffi::ma_device_handle_backend_data_callback::<F, R>(self.to_raw(), output, input)
    }

    /// Provides a reference to the [`CustomBackend::Context`]
    pub fn backend_context(&self) -> &B::Context {
        self.backend_context.get().unwrap()
    }
}
