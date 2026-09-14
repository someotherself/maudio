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
    #[allow(unused)]
    store: DeviceIdStore,
}

unsafe impl Send for DeviceIdInner {}
unsafe impl Sync for DeviceIdInner {}

impl AsRawRef for DeviceId {
    type Raw = sys::ma_device_id;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner.id
    }
}

#[derive(Default)]
enum DeviceIdStore {
    #[default]
    Native,
    Id,
    Name,
}

impl DeviceId {
    pub fn custom_from_id(id: i32) -> Self {
        let inner = sys::ma_device_id {
            custom: sys::ma_device_id__bindgen_ty_1 { i: id },
        };
        Self {
            inner: Arc::new(DeviceIdInner {
                id: inner,
                store: DeviceIdStore::Id,
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
                store: DeviceIdStore::Name,
            }),
        })
    }

    pub fn get_custom_name(&self) -> Option<String> {
        if matches!(self.inner.store, DeviceIdStore::Name) {
            let name = unsafe { std::ffi::CStr::from_ptr(self.inner.id.custom.s.as_ptr()) };

            Some(name.to_string_lossy().into_owned())
        } else {
            None
        }
    }

    pub(crate) fn from_raw(id: &sys::ma_device_id) -> Self {
        Self {
            inner: Arc::new(DeviceIdInner {
                id: *id,
                store: DeviceIdStore::default(),
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
