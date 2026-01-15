use maudio_sys::ffi as sys;

use crate::audio::formats::Format;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataFormat {
    pub format: Format,
    pub channels: u32,
    pub sample_rate: u32,
    /// Channel order/map for each channel, length == channels (when available).
    pub channel_map: Vec<sys::ma_channel>,
}
