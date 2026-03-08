use core::slice;
use std::ffi::CStr;

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    device::{device_id::DeviceId, device_type::DeviceType},
    AsRawRef, MaResult,
};

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct DeviceInfo {
    inner: sys::ma_device_info,
}

impl AsRawRef for DeviceInfo {
    type Raw = sys::ma_device_info;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl DeviceInfo {
    pub(crate) fn new(info: sys::ma_device_info) -> Self {
        Self { inner: info }
    }
}

impl DeviceInfo {
    pub fn device_id(&self) -> DeviceId {
        DeviceId::new(self.inner.id)
    }

    pub fn device_name(&self) -> &str {
        unsafe { CStr::from_ptr(self.inner.name.as_ptr()) }
            .to_str()
            .unwrap_or_default()
    }

    pub fn format_counts(&self) -> usize {
        self.inner.nativeDataFormatCount as usize
    }

    pub fn device_formats(&self) -> Vec<DeviceFormat> {
        let count = self.inner.nativeDataFormatCount as usize;
        let raw: &[sys::ma_device_info__bindgen_ty_1] = unsafe {
            slice::from_raw_parts(
                &self.inner.nativeDataFormats as *const sys::ma_device_info__bindgen_ty_1,
                count,
            )
        };
        raw.iter()
            .filter_map(|r| DeviceFormat::try_from_raw(r).ok())
            .collect()
    }
}

pub struct DeviceBasicInfo<'a> {
    id: &'a sys::ma_device_id,
    name: &'a core::ffi::CStr,
    is_default: bool,
}

impl<'a> DeviceBasicInfo<'a> {
    pub(crate) fn new(
        id: &'a sys::ma_device_id,
        name: &'a core::ffi::CStr,
        is_default: u32,
    ) -> Self {
        Self {
            id,
            name,
            is_default: is_default == 1,
        }
    }

    pub fn id(&self) -> &'a DeviceId {
        unsafe { &*(self.id as *const sys::ma_device_id as *const DeviceId) }
    }

    pub fn name(&self) -> &'a str {
        unsafe { CStr::from_ptr(self.name.as_ptr()) }
            .to_str()
            .unwrap_or_default()
    }

    pub fn is_default(&self) -> bool {
        self.is_default
    }
}

pub struct Devices {
    pub playback: Vec<DeviceInfo>,
    pub capture: Vec<DeviceInfo>,
}

impl Devices {
    pub(crate) fn from_owned(playback: Vec<DeviceInfo>, capture: Vec<DeviceInfo>) -> Self {
        Self { playback, capture }
    }

    pub fn playback(&self) -> impl Iterator<Item = (DeviceType, &DeviceInfo)> {
        self.playback.iter().map(|d| (DeviceType::PlayBack, d))
    }

    pub fn capture(&self) -> impl Iterator<Item = (DeviceType, &DeviceInfo)> {
        self.capture.iter().map(|d| (DeviceType::Capture, d))
    }

    pub fn iter(&self) -> impl Iterator<Item = (DeviceType, &DeviceInfo)> {
        self.playback().chain(self.capture())
    }
}

pub struct DeviceFormat {
    format: Format,
    channels: u32,
    sample_rate: SampleRate,
    exclusive: bool, // only used by MA_DATA_FORMAT_FLAG_EXCLUSIVE_MODE (wasapi)
}

impl DeviceFormat {
    fn try_from_raw(r: &sys::ma_device_info__bindgen_ty_1) -> MaResult<Self> {
        Ok(Self {
            format: Format::try_from(r.format)?,
            channels: r.channels,
            sample_rate: r.sampleRate.try_into()?,
            exclusive: (r.flags & sys::MA_DATA_FORMAT_FLAG_EXCLUSIVE_MODE) != 0,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::context::{ContextBuilder, ContextOps};

    #[test]
    fn test_devices_iter() {
        let ctx = ContextBuilder::new().build().unwrap();
        let devices = ctx.get_devices().unwrap();
        for (_, info) in devices.capture() {
            println!("Capture device: {}", info.device_name());
        }
        for (_, info) in devices.playback() {
            println!("PlayBack device: {}", info.device_name());
        }
        let total_devices = devices.iter().count();
        assert_eq!(
            total_devices,
            devices.capture.len() + devices.playback.len()
        );
        for (device_type, info) in devices.iter() {
            println!("{} device: {}", device_type, info.device_name());
        }
    }
}
