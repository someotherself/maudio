use maudio_sys::ffi as sys;

use crate::AsRawRef;

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
