use maudio_sys::ffi as sys;

use crate::{device::Device, AsRawRef};

pub struct DeviceBuilder {
    inner: sys::ma_device_config,
}

impl AsRawRef for DeviceBuilder {
    type Raw = sys::ma_device_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl DeviceBuilder {
    pub fn new(device_type: sys::ma_device_type) -> Self {
        let ptr = unsafe { sys::ma_device_config_init(device_type) };
        Self { inner: ptr }
    }

    pub fn format(self) -> Self {
        todo!()
    }

    pub fn device_id(self) -> Self {
        todo!()
    }

    // ma_channel_mix_mode (inner.playback.channelMixMode) Can be set?
    pub fn mix_mode(self) -> Self {
        todo!()
    }

    pub fn channels(self) -> Self {
        todo!()
    }

    pub fn sample_rate(self) -> Self {
        todo!()
    }

    pub fn callback(self) -> Self {
        todo!()
    }

    pub fn user_data(self) -> Self {
        todo!()
    }

    pub fn build() -> Device {
        todo!()
    }
}
