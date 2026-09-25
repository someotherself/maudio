//! Audio device identifier definitions.
use std::sync::Arc;

use maudio_sys::ffi as sys;

use crate::{AsRawRef, ErrorKinds, MaResult, MaudioError};

/// Identifies an audio device reported by [`Context`](crate::context::Context) enumeration.
///
/// A `DeviceId` is typically obtained from [`DeviceInfo`](crate::device::device_info::DeviceInfo) or [`DeviceBasicInfo`](crate::device::device_info::DeviceBasicInfo) and then
/// passed back to device configuration when opening a specific playback or capture device.
///
/// This is a thin value wrapper over miniaudio's `ma_device_id`. It does not own any external
/// resources and can be cheaply copied.
#[repr(transparent)]
#[derive(Clone)]
pub struct DeviceId {
    pub(crate) inner: Arc<DeviceIdInner>,
}

pub(crate) struct DeviceIdInner {
    pub(crate) id: sys::ma_device_id,
    pub(crate) custom_state: CustomDeviceId,
}

unsafe impl Send for DeviceIdInner {}
unsafe impl Sync for DeviceIdInner {}

impl AsRawRef for DeviceId {
    type Raw = sys::ma_device_id;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner.id
    }
}

impl DeviceId {
    pub fn custom_from_id(id: i32) -> Self {
        let inner = sys::ma_device_id {
            custom: sys::ma_device_id__bindgen_ty_1 { i: id },
        };
        Self {
            inner: Arc::new(DeviceIdInner {
                id: inner,
                custom_state: CustomDeviceId::Id(id),
            }),
        }
    }

    pub fn custom_from_name(name: impl ToString) -> MaResult<Self> {
        let name = name.to_string();
        if name.len() >= 256 {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Name length out of range",
            )));
        };

        // Convert string to c_char
        let bytes = name.as_bytes();

        let mut buffer = [0 as core::ffi::c_char; 256];

        for (dest, &src) in buffer.iter_mut().zip(bytes) {
            *dest = src as core::ffi::c_char;
        }

        let inner = sys::ma_device_id {
            custom: sys::ma_device_id__bindgen_ty_1 { s: buffer },
        };

        Ok(Self {
            inner: Arc::new(DeviceIdInner {
                id: inner,
                custom_state: CustomDeviceId::Name(name.to_string()),
            }),
        })
    }

    pub fn get_custom_name(&self) -> Option<String> {
        if let CustomDeviceId::Name(name) = &self.inner.custom_state {
            Some(name.clone())
        } else {
            None
        }
    }

    pub fn get_custom_id(&self) -> Option<i32> {
        if let CustomDeviceId::Id(id) = &self.inner.custom_state {
            Some(*id)
        } else {
            None
        }
    }

    pub(crate) fn from_raw(id: &sys::ma_device_id, name: impl ToString) -> Self {
        Self {
            inner: Arc::new(DeviceIdInner {
                id: *id,
                custom_state: CustomDeviceId::Name(name.to_string()),
            }),
        }
    }
}

impl PartialEq for DeviceId {
    fn eq(&self, other: &Self) -> bool {
        unsafe { sys::ma_device_id_equal(self.as_raw_ptr(), other.as_raw_ptr()) != 0 }
    }
}
impl Eq for DeviceId {}

// Reasoning:
//
// The miniaudio's ma_device_id is an untagged union
// In a custom backend, we need tp *unsafely* access fields on `ma_device_id.custom`
//
// The custom field only exists if the DeviceId was created by a custom context.
// However, it is technically possible for a user to create a DeviceId using
// a built in context, and pass that to a device / engine builder to create a
// custom backend.
// Then, the ma_device_id.custom state will not exist
//
// Aditionally, any extra fields / state we add to DeviceId or DeviceIdInner does not survive
// the FFI roundtrip, which means we cannot safety access the fields on ma_device_id
// without causing undefined behavior.
//
// The workaround is:
// 1. Anytime a user enumerates and selects a DeviceId, capture extra state on it
// 2. If the user creates a device or engine with a custom backend, a provided the
// DeviceId, capture it from the builder and save it on the pUserData of the custom context
//
// This bypasses the ma_device_id given to use in CustomBackend::init_device and lets us
// access CustomDeviceId
//
// Workaround:
//
// We save CustomDeviceId on the context builder. However, the deviceid can be passed
// to the device or engine either before or after the context is built (with the custom_backed method)
//
// We try to save the CustomDeviceId when the context is built, and then again when the engine or device is built.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum CustomDeviceId {
    #[default]
    Default,
    Name(String),
    Id(i32),
}
