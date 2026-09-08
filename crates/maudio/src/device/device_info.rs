//! Describes properties and capabilities of audio devices.
use core::slice;
use std::ffi::CStr;

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    device::{device_id::DeviceId, device_type::DeviceType},
    AsRawRef, MaResult,
};

const EXCLUSIVE_MODE: u32 = 1 << 1; // 0b0000_0010

/// Detailed information about a playback or capture device returned by enumeration.
///
/// `DeviceInfo` is an owned snapshot of the information reported by the backend at the time of
/// enumeration. It includes the device ID, a display name, and any native formats reported by
/// the backend.
///
/// This type is mostly used together with [`ContextOps::get_devices()`](crate::context::ContextOps::get_devices).
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
    /// Returns the stable identifier for this device.
    ///
    /// The returned ID can be stored and later supplied to device configuration to request
    /// this specific device instead of the system default.
    pub fn device_id(&self) -> DeviceId {
        DeviceId::from_raw(&self.inner.id)
    }

    /// Returns the backend-provided display name for this device.
    ///
    /// This name is intended for user-facing display and device selection UI.
    ///
    /// Invalid UTF-8 is replaced with an empty string.
    pub fn device_name(&self) -> &str {
        unsafe { CStr::from_ptr(self.inner.name.as_ptr()) }
            .to_str()
            .unwrap_or_default()
    }

    /// Returns the number of native data formats reported by the backend.
    ///
    /// The exact amount and quality of this information depends on the active backend and
    /// operating system. Some backends may report limited format information.
    pub fn format_count(&self) -> usize {
        self.inner.nativeDataFormatCount as usize
    }

    /// Returns the native data formats reported for this device.
    ///
    /// These formats represent configurations the backend reports as directly supported by
    /// the device. Availability and completeness depend on the backend.
    ///
    /// Any raw format entries that cannot be represented safely are skipped.
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

pub struct DeviceInfoBuilder {
    id: DeviceId,
    name: String, // Needs to be max 255
    is_default: bool,
    data_formats: Vec<sys::ma_device_info__bindgen_ty_1>,
}

impl DeviceInfoBuilder {
    pub fn from_index(id: i32, name: String) -> Self {
        let id = DeviceId::custom_from_id(id);
        Self {
            id,
            name,
            is_default: false,
            data_formats: Vec::new(),
        }
    }

    pub fn from_name(name: impl ToString) -> MaResult<Self> {
        let name = name.to_string();
        let id = DeviceId::custom_from_name(name.clone())?;
        Ok(Self {
            id,
            name,
            is_default: false,
            data_formats: Vec::new(),
        })
    }

    pub fn new(id: DeviceId, name: String) -> Self {
        Self {
            id,
            name,
            is_default: false,
            data_formats: Vec::new(),
        }
    }

    pub fn default(&mut self, yes: bool) -> &mut Self {
        self.is_default = yes;
        self
    }

    pub fn add_data_formats(
        &mut self,
        format: Format,
        channels: u32,
        sample_rate: SampleRate,
        exclusive_mode: bool,
    ) -> &mut Self {
        let flags = if exclusive_mode { EXCLUSIVE_MODE } else { 0 };
        let df = sys::ma_device_info__bindgen_ty_1 {
            format: format.into(),
            channels,
            sampleRate: sample_rate.into(),
            flags,
        };
        self.data_formats.push(df);
        self
    }

    pub fn build(&self) -> DeviceInfo {
        // convert the name to c_char
        let bytes = self.name.as_bytes();

        let mut buffer = [0 as core::ffi::c_char; 256];

        for (dest, &src) in buffer.iter_mut().zip(bytes) {
            *dest = src as core::ffi::c_char;
        }

        let zero_df: sys::ma_device_info__bindgen_ty_1 = sys::ma_device_info__bindgen_ty_1 {
            format: 0,
            channels: 0,
            sampleRate: 0,
            flags: 0,
        };
        let mut formats = [zero_df; 64];

        for (idx, &f) in self.data_formats.iter().enumerate() {
            formats[idx] = f;
        }

        let info = sys::ma_device_info {
            id: self.id.inner.id,
            name: buffer,
            isDefault: self.is_default as u32,
            nativeDataFormatCount: self.data_formats.len() as u32,
            nativeDataFormats: formats,
        };

        DeviceInfo { inner: info }
    }
}

/// A lightweight borrowed view of a device entry.
///
/// Unlike [`DeviceInfo`], this type borrows its underlying ID and name data instead of owning
/// a full copy. It is useful for short-lived enumeration APIs that expose references into an
/// existing device list.
///
/// The returned references are only valid for as long as the underlying enumeration data lives.
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

    /// Returns the ID of this device.
    pub fn id(&self) -> DeviceId {
        DeviceId::from_raw(self.id)
    }

    /// Returns the backend-provided display name of this device.
    pub fn name(&self) -> &'a str {
        unsafe { CStr::from_ptr(self.name.as_ptr()) }
            .to_str()
            .unwrap_or_default()
    }

    /// Returns `true` if this is the current default device for its direction.
    pub fn is_default(&self) -> bool {
        self.is_default
    }
}

/// An owned, grouped collection of enumerated audio devices.
///
/// Playback and capture devices are stored separately because operating system backends report
/// them independently, but [`iter`](Self::iter) can be used to traverse all devices in one pass.
pub struct Devices {
    pub playback: Vec<DeviceInfo>,
    pub capture: Vec<DeviceInfo>,
}

impl Devices {
    pub(crate) fn from_owned(playback: Vec<DeviceInfo>, capture: Vec<DeviceInfo>) -> Self {
        Self { playback, capture }
    }

    /// Iterates over all playback devices.
    pub fn playback(&self) -> impl Iterator<Item = (DeviceType, &DeviceInfo)> {
        self.playback.iter().map(|d| (DeviceType::Playback, d))
    }

    /// Iterates over all capture devices.
    pub fn capture(&self) -> impl Iterator<Item = (DeviceType, &DeviceInfo)> {
        self.capture.iter().map(|d| (DeviceType::Capture, d))
    }

    /// Iterates over all devices, yielding the device type together with its info.
    pub fn iter(&self) -> impl Iterator<Item = (DeviceType, &DeviceInfo)> {
        self.playback().chain(self.capture())
    }
}

/// A single native format reported by a device during enumeration.
///
/// This describes one backend-reported combination of sample format, channel count and sample
/// rate that the device may support natively.
///
/// Support for `exclusive` mode is backend specific and is primarily relevant to WASAPI.
#[allow(unused)]
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
            println!("Playback device: {}", info.device_name());
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
