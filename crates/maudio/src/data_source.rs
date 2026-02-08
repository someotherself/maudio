//! Interface for reading from a data source
use std::{marker::PhantomData, ops::Range};

use maudio_sys::ffi as sys;

use crate::{
    audio::{channels::Channel, formats::Format},
    Binding, MaResult,
};

pub mod sources;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataFormat {
    pub format: Format,
    pub channels: u32,
    pub sample_rate: u32,
    /// Channel order/map for each channel, length == channels (when available).
    pub channel_map: Vec<Channel>,
}

#[derive(PartialEq)]
pub struct DataSource {
    inner: *mut sys::ma_data_source,
}

pub type GetNextCallback = sys::ma_data_source_get_next_proc;

impl Binding for DataSource {
    type Raw = *mut sys::ma_data_source;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self { inner: raw }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

// Keeps the methods on AsSourcePtr private
pub(crate) mod private_data_source {
    use crate::{
        data_source::sources::{
            buffer::{AudioBuffer, AudioBufferOps, AudioBufferRef},
            decoder::{Decoder, DecoderOps},
            pulsewave::{
                PulseWaveF32, PulseWaveI16, PulseWaveI32, PulseWaveOps, PulseWaveS24, PulseWaveU8,
            },
            waveform::{
                WaveFormF32, WaveFormI16, WaveFormI32, WaveFormOps, WaveFormS24, WaveFormU8,
            },
        },
        engine::node_graph::nodes::source::source_node::AttachedSourceNode,
        util::s24::PcmFormat,
    };

    use super::*;
    use maudio_sys::ffi as sys;

    pub trait DataSourcePtrProvider<T: ?Sized> {
        fn as_source_ptr(t: &T) -> *mut sys::ma_data_source;
    }

    pub struct DataSourceProvider;
    pub struct DataSourceRefProvider;
    pub struct AudioBufferProvider;
    pub struct AudioBufferRefProvider;
    pub struct DecoderProvider;
    pub struct DecoderRefProvider;
    pub struct PulseWaveU8Provider;
    pub struct PulseWaveI16Provider;
    pub struct PulseWaveI32Provider;
    pub struct PulseWaveS24Provider;
    pub struct PulseWaveF32Provider;
    pub struct WaveFormU8Provider;
    pub struct WaveFormI16Provider;
    pub struct WaveFormI32Provider;
    pub struct WaveFormS24Provider;
    pub struct WaveFormF32Provider;
    pub struct AttachedSourceNodeProvider;

    // DataSource

    impl DataSourcePtrProvider<DataSource> for DataSourceProvider {
        #[inline]
        fn as_source_ptr(t: &DataSource) -> *mut sys::ma_data_source {
            t.to_raw()
        }
    }

    impl DataSourcePtrProvider<DataSourceRef<'_>> for DataSourceRefProvider {
        #[inline]
        fn as_source_ptr(t: &DataSourceRef) -> *mut sys::ma_data_source {
            t.to_raw()
        }
    }

    impl DataSourcePtrProvider<AudioBuffer<'_>> for AudioBufferProvider {
        #[inline]
        fn as_source_ptr(t: &AudioBuffer) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl<'a> DataSourcePtrProvider<AudioBufferRef<'a>> for AudioBufferRefProvider {
        #[inline]
        fn as_source_ptr(t: &AudioBufferRef) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl<F: PcmFormat, S> DataSourcePtrProvider<Decoder<F, S>> for DecoderProvider {
        #[inline]
        fn as_source_ptr(t: &Decoder<F, S>) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl DataSourcePtrProvider<PulseWaveU8> for PulseWaveU8Provider {
        #[inline]
        fn as_source_ptr(t: &PulseWaveU8) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl DataSourcePtrProvider<PulseWaveI16> for PulseWaveI16Provider {
        #[inline]
        fn as_source_ptr(t: &PulseWaveI16) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl DataSourcePtrProvider<PulseWaveI32> for PulseWaveI32Provider {
        #[inline]
        fn as_source_ptr(t: &PulseWaveI32) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl DataSourcePtrProvider<PulseWaveS24> for PulseWaveS24Provider {
        #[inline]
        fn as_source_ptr(t: &PulseWaveS24) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl DataSourcePtrProvider<PulseWaveF32> for PulseWaveF32Provider {
        #[inline]
        fn as_source_ptr(t: &PulseWaveF32) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl DataSourcePtrProvider<WaveFormU8> for WaveFormU8Provider {
        #[inline]
        fn as_source_ptr(t: &WaveFormU8) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl DataSourcePtrProvider<WaveFormI16> for WaveFormI16Provider {
        #[inline]
        fn as_source_ptr(t: &WaveFormI16) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl DataSourcePtrProvider<WaveFormI32> for WaveFormI32Provider {
        #[inline]
        fn as_source_ptr(t: &WaveFormI32) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl DataSourcePtrProvider<WaveFormS24> for WaveFormS24Provider {
        #[inline]
        fn as_source_ptr(t: &WaveFormS24) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl DataSourcePtrProvider<WaveFormF32> for WaveFormF32Provider {
        #[inline]
        fn as_source_ptr(t: &WaveFormF32) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl<S: AsSourcePtr> DataSourcePtrProvider<AttachedSourceNode<'_, S>>
        for AttachedSourceNodeProvider
    {
        #[inline]
        fn as_source_ptr(t: &AttachedSourceNode<'_, S>) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    pub fn source_ptr<T: AsSourcePtr + ?Sized>(t: &T) -> *mut sys::ma_data_source {
        <T as AsSourcePtr>::__PtrProvider::as_source_ptr(t)
    }
}

#[doc(hidden)]
pub trait AsSourcePtr {
    type __PtrProvider: private_data_source::DataSourcePtrProvider<Self>;
}

#[doc(hidden)]
impl AsSourcePtr for DataSource {
    type __PtrProvider = private_data_source::DataSourceProvider;
}

#[doc(hidden)]
impl<'a> AsSourcePtr for DataSourceRef<'a> {
    type __PtrProvider = private_data_source::DataSourceRefProvider;
}

impl<T: AsSourcePtr + ?Sized> DataSourceOps for T {}

// TODO: Some of these methods should be on the specific types instead.
// TODO: Check them all and decide which should be public here.
/// The DataSourceOps trait contains shared methods for [`DataSource`], [`DataSourceRef`] and all data source types which can be cast to a `ma_data_source`
pub trait DataSourceOps: AsSourcePtr {
    // fn read_pcm_frames(&mut self, frame_count: u64, channels: u32) -> MaResult<(Vec<f32>, u64)> {
    //     data_source_ffi::ma_data_source_read_pcm_frames(self, frame_count, channels)
    // }

    // fn seek_pcm_frames(&mut self, frame_count: u64) -> MaResult<u64> {
    //     data_source_ffi::ma_data_source_seek_pcm_frames(self, frame_count)
    // }

    // fn seek_to_pcm_frame(&mut self, frame_index: u64) -> MaResult<()> {
    //     data_source_ffi::ma_data_source_seek_to_pcm_frame(self, frame_index)
    // }

    // fn seek_seconds(&mut self, seconds: f32) -> MaResult<f32> {
    //     data_source_ffi::ma_data_source_seek_seconds(self, seconds)
    // }

    // fn seek_to_second(&mut self, seek_point: f32) -> MaResult<()> {
    //     data_source_ffi::ma_data_source_seek_to_second(self, seek_point)
    // }

    fn data_format(&self) -> MaResult<DataFormat> {
        data_source_ffi::ma_data_source_get_data_format(self)
    }

    // fn cursor_in_pcm_frames(&mut self) -> MaResult<u64> {
    //     data_source_ffi::ma_data_source_get_cursor_in_pcm_frames(self)
    // }

    // fn length_in_pcm_frames(&mut self) -> MaResult<u64> {
    //     data_source_ffi::ma_data_source_get_length_in_pcm_frames(self)
    // }

    // fn cursor_in_seconds(&mut self) -> MaResult<f32> {
    //     data_source_ffi::ma_data_source_get_cursor_in_seconds(self)
    // }

    // fn length_in_seconds(&mut self) -> MaResult<f32> {
    //     data_source_ffi::ma_data_source_get_length_in_seconds(self)
    // }

    fn set_looping(&mut self, is_looping: bool) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_looping(self, is_looping)
    }

    fn looping(&self) -> bool {
        data_source_ffi::ma_data_source_is_looping(self)
    }

    // ???
    fn range_in_pcm_frames(&self) -> Range<u64> {
        data_source_ffi::ma_data_source_get_range_in_pcm_frames(self)
    }

    fn set_loop_point_in_pcm_frames(&mut self, begin: u64, end: u64) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_loop_point_in_pcm_frames(self, begin, end)
    }

    fn loop_point_in_pcm_frames(&self) -> Range<u64> {
        data_source_ffi::ma_data_source_get_loop_point_in_pcm_frames(self)
    }

    fn set_current<S: AsSourcePtr + ?Sized>(&mut self, current: &mut S) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_current(self, current)
    }

    fn current(&self) -> Option<DataSourceRef<'_>> {
        data_source_ffi::ma_data_source_get_current(self)
    }

    fn set_next<S: AsSourcePtr + ?Sized>(&mut self, next: &mut S) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_next(self, next)
    }

    fn next(&self) -> Option<DataSourceRef<'_>> {
        data_source_ffi::ma_data_source_get_next(self)
    }

    fn set_next_callback(&mut self, get_next_cb: GetNextCallback) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_next_callback(self, get_next_cb)
    }

    fn next_callback(&self) -> GetNextCallback {
        data_source_ffi::ma_data_source_get_next_callback(self)
    }
}

pub(crate) mod data_source_ffi {
    use core::f32;
    use std::marker::PhantomData;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::channels::Channel,
        data_source::{
            private_data_source, AsSourcePtr, DataFormat, DataSource, DataSourceRef,
            GetNextCallback,
        },
        Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_data_source_init(
        config: *const sys::ma_data_source_config,
        source: *mut sys::ma_data_source,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_data_source_init(config, source) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_data_source_uninit(source: &mut DataSource) {
        unsafe {
            sys::ma_data_source_uninit(source.to_raw());
        }
    }

    #[inline]
    pub fn ma_data_source_read_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        frame_count: u64,
        channels: u32,
    ) -> MaResult<(Vec<f32>, u64)> {
        let mut buffer = vec![0.0f32; (frame_count * channels as u64) as usize];
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_data_source_read_pcm_frames(
                private_data_source::source_ptr(source),
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        Ok((buffer, frames_read))
    }

    #[inline]
    pub fn ma_data_source_seek_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        frame_count: u64,
    ) -> MaResult<u64> {
        let mut frames_seeked = 0;
        let res = unsafe {
            sys::ma_data_source_seek_pcm_frames(
                private_data_source::source_ptr(source),
                frame_count,
                &mut frames_seeked,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_seeked)
    }

    #[inline]
    pub fn ma_data_source_seek_to_pcm_frame<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        frame_index: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_data_source_seek_to_pcm_frame(
                private_data_source::source_ptr(source),
                frame_index,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_data_source_seek_seconds<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        seconds: f32,
    ) -> MaResult<f32> {
        let mut seconds_seeked = 0.0f32;
        let res = unsafe {
            sys::ma_data_source_seek_seconds(
                private_data_source::source_ptr(source),
                seconds,
                &mut seconds_seeked,
            )
        };
        MaudioError::check(res)?;
        Ok(seconds_seeked)
    }

    #[inline]
    pub fn ma_data_source_seek_to_second<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        seek_point: f32,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_data_source_seek_to_second(private_data_source::source_ptr(source), seek_point)
        };
        MaudioError::check(res)
    }

    pub fn ma_data_source_get_data_format<S: AsSourcePtr + ?Sized>(
        source: &S,
    ) -> MaResult<DataFormat> {
        let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        let mut channels: sys::ma_uint32 = 0;
        let mut sample_rate: sys::ma_uint32 = 0;
        let mut channel_map_raw = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];
        let res = unsafe {
            sys::ma_data_source_get_data_format(
                private_data_source::source_ptr(source),
                &mut format_raw,
                &mut channels,
                &mut sample_rate,
                channel_map_raw.as_mut_ptr(),
                channel_map_raw.len(),
            )
        };
        MaudioError::check(res)?;
        // Could cast when passing the ptr to miniaudio, but copying should be fine here
        let mut channel_map: Vec<Channel> =
            channel_map_raw.into_iter().map(Channel::from_raw).collect();
        channel_map.truncate(channels as usize);

        Ok(DataFormat {
            format: format_raw.try_into()?,
            channels: channels as u32,
            sample_rate: sample_rate as u32,
            channel_map,
        })
    }

    #[inline]
    pub fn ma_data_source_get_cursor_in_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &mut S,
    ) -> MaResult<u64> {
        let mut cursor = 0;
        let res = unsafe {
            sys::ma_data_source_get_cursor_in_pcm_frames(
                private_data_source::source_ptr(source),
                &mut cursor,
            )
        };
        MaudioError::check(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_data_source_get_length_in_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &mut S,
    ) -> MaResult<u64> {
        let mut length = 0;
        let res = unsafe {
            sys::ma_data_source_get_length_in_pcm_frames(
                private_data_source::source_ptr(source),
                &mut length,
            )
        };
        MaudioError::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_data_source_get_cursor_in_seconds<S: AsSourcePtr + ?Sized>(
        source: &mut S,
    ) -> MaResult<f32> {
        let mut cursor = 0.0f32;
        let res = unsafe {
            sys::ma_data_source_get_cursor_in_seconds(
                private_data_source::source_ptr(source),
                &mut cursor,
            )
        };
        MaudioError::check(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_data_source_get_length_in_seconds<S: AsSourcePtr + ?Sized>(
        source: &mut S,
    ) -> MaResult<f32> {
        let mut length = 0.0f32;
        let res = unsafe {
            sys::ma_data_source_get_length_in_seconds(
                private_data_source::source_ptr(source),
                &mut length,
            )
        };
        MaudioError::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_data_source_set_looping<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        is_looping: bool,
    ) -> MaResult<()> {
        let is_looping = is_looping as u32;
        let res = unsafe {
            sys::ma_data_source_set_looping(private_data_source::source_ptr(source), is_looping)
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_data_source_is_looping<S: AsSourcePtr + ?Sized>(source: &S) -> bool {
        let res = unsafe {
            sys::ma_data_source_is_looping(private_data_source::source_ptr(source) as *const _)
        };
        res == 1
    }

    #[inline]
    pub fn ma_data_source_set_range_in_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &S,
        range: core::ops::Range<u64>,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_data_source_set_range_in_pcm_frames(
                private_data_source::source_ptr(source),
                range.start,
                range.end,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_data_source_get_range_in_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &S,
    ) -> core::ops::Range<u64> {
        let mut begin = 0;
        let mut end = 0;
        unsafe {
            sys::ma_data_source_get_range_in_pcm_frames(
                private_data_source::source_ptr(source) as *const _,
                &mut begin,
                &mut end,
            );
        }
        begin..end
    }

    #[inline]
    pub fn ma_data_source_set_loop_point_in_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        begin: u64,
        end: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_data_source_set_loop_point_in_pcm_frames(
                private_data_source::source_ptr(source),
                begin,
                end,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_data_source_get_loop_point_in_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &S,
    ) -> core::ops::Range<u64> {
        let mut begin = 0;
        let mut end: u64 = 0;
        unsafe {
            sys::ma_data_source_get_loop_point_in_pcm_frames(
                private_data_source::source_ptr(source) as *const _,
                &mut begin,
                &mut end,
            );
        };
        begin..end
    }

    #[inline]
    pub fn ma_data_source_set_current<S: AsSourcePtr + ?Sized, C: AsSourcePtr + ?Sized>(
        source: &mut S,
        current: &mut C,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_data_source_set_current(
                private_data_source::source_ptr(source),
                private_data_source::source_ptr(current),
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_data_source_get_current<'a, S: AsSourcePtr + ?Sized>(
        source: &'a S,
    ) -> Option<DataSourceRef<'a>> {
        let ptr = unsafe {
            sys::ma_data_source_get_current(private_data_source::source_ptr(source) as *const _)
        };
        if ptr.is_null() {
            None
        } else {
            Some(DataSourceRef {
                inner: ptr,
                _marker: PhantomData,
            })
        }
    }

    #[inline]
    pub fn ma_data_source_set_next<S: AsSourcePtr + ?Sized, N: AsSourcePtr + ?Sized>(
        source: &mut S,
        next: &mut N,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_data_source_set_next(
                private_data_source::source_ptr(source),
                private_data_source::source_ptr(next),
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_data_source_get_next<'a, S: AsSourcePtr + ?Sized>(
        source: &'a S,
    ) -> Option<DataSourceRef<'a>> {
        let ptr = unsafe {
            sys::ma_data_source_get_next(private_data_source::source_ptr(source) as *const _)
        };
        if ptr.is_null() {
            None
        } else {
            Some(DataSourceRef {
                inner: ptr,
                _marker: PhantomData,
            })
        }
    }

    // TODO
    #[inline]
    pub fn ma_data_source_set_next_callback<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        get_next_cb: GetNextCallback,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_data_source_set_next_callback(
                private_data_source::source_ptr(source),
                get_next_cb,
            )
        };
        MaudioError::check(res)
    }

    // TODO
    #[inline]
    pub fn ma_data_source_get_next_callback<S: AsSourcePtr + ?Sized>(
        source: &S,
    ) -> GetNextCallback {
        unsafe {
            sys::ma_data_source_get_next_callback(
                private_data_source::source_ptr(source) as *const _
            )
        }
    }
}

#[derive(PartialEq, Copy, Clone)]
pub struct DataSourceRef<'a> {
    inner: *mut sys::ma_data_source,
    _marker: PhantomData<&'a ()>,
}

impl<'a> Binding for DataSourceRef<'a> {
    type Raw = *mut sys::ma_data_source;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            _marker: PhantomData,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}
