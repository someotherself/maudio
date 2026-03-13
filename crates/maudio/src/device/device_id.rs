use maudio_sys::ffi as sys;

use crate::AsRawRef;

/// Identifies an audio device reported by [`Context`](crate::context::Context) enumeration.
///
/// A `DeviceId` is typically obtained from [`DeviceInfo`] or [`DeviceBasicInfo`] and then
/// passed back to device configuration when opening a specific playback or capture device.
///
/// This is a thin value wrapper over miniaudio's `ma_device_id`. It does not own any external
/// resources and can be cheaply copied.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct DeviceId {
    inner: sys::ma_device_id,
}

impl AsRawRef for DeviceId {
    type Raw = sys::ma_device_id;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl DeviceId {
    pub(crate) fn new(id: sys::ma_device_id) -> Self {
        Self { inner: id }
    }
}

impl PartialEq for DeviceId {
    fn eq(&self, other: &Self) -> bool {
        unsafe { sys::ma_device_id_equal(&self.inner, &other.inner) != 0 }
    }
}
impl Eq for DeviceId {}
