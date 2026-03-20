//! Audio device identifier definitions.
use std::rc::Rc;

use maudio_sys::ffi as sys;

use crate::AsRawRef;

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
    inner: Rc<DeviceIdInner>,
}

struct DeviceIdInner {
    inner: sys::ma_device_id,
}

impl AsRawRef for DeviceId {
    type Raw = sys::ma_device_id;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner.inner
    }
}

impl DeviceId {
    pub(crate) fn new(id: sys::ma_device_id) -> Self {
        Self {
            inner: Rc::new(DeviceIdInner { inner: id }),
        }
    }
}

impl PartialEq for DeviceId {
    fn eq(&self, other: &Self) -> bool {
        unsafe { sys::ma_device_id_equal(self.as_raw_ptr(), other.as_raw_ptr()) != 0 }
    }
}
impl Eq for DeviceId {}
