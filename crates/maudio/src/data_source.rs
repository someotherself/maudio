//! Interface for reading from a data source
use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    audio::{
        channels::Channel,
        formats::{Format, SampleBuffer},
        sample_rate::SampleRate,
    },
    engine::resource::{
        rm_buffer::ResourceManagerBuffer, rm_source::ResourceManagerSource,
        rm_stream::ResourceManagerStream, AsRmPtr,
    },
    pcm_frames::PcmFormat,
    Binding, MaResult, MaudioError,
};

pub mod sources;

/// Describes an audio stream’s PCM format and layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataFormat {
    /// Sample format (e.g. `S16`, `F32`).
    pub format: Format,
    /// Number of interleaved channels.
    pub channels: u32,
    /// Sample rate in Hz.
    pub sample_rate: SampleRate,
    /// Channel order/map for each channel, length == channels (when available).
    pub channel_map: Option<Vec<Channel>>,
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
            pulsewave::{PulseWave, PulseWaveOps},
            waveform::{WaveForm, WaveFormOps},
        },
        engine::{
            node_graph::nodes::source::source_node::AttachedSourceNode,
            resource::{
                rm_source::ResourceManagerSource, rm_stream::ResourceManagerStream, AsRmPtr,
            },
        },
        pcm_frames::PcmFormat,
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
    pub struct PulseWaveProvider;
    pub struct WaveFormProvider;
    pub struct WaveFormF32Provider;
    pub struct AttachedSourceNodeProvider;
    pub struct ResourceManagerSourceProvider;
    pub struct ResourceManagerBufferProvider;
    pub struct ResourceManagerStreamProvider;

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

    impl<F: PcmFormat> DataSourcePtrProvider<AudioBuffer<F>> for AudioBufferProvider {
        #[inline]
        fn as_source_ptr(t: &AudioBuffer<F>) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl<F: PcmFormat> DataSourcePtrProvider<AudioBufferRef<'_, F>> for AudioBufferRefProvider {
        #[inline]
        fn as_source_ptr(t: &AudioBufferRef<F>) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl<F: PcmFormat, S> DataSourcePtrProvider<Decoder<F, S>> for DecoderProvider {
        #[inline]
        fn as_source_ptr(t: &Decoder<F, S>) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl<F: PcmFormat> DataSourcePtrProvider<PulseWave<F>> for PulseWaveProvider {
        #[inline]
        fn as_source_ptr(t: &PulseWave<F>) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl<F: PcmFormat> DataSourcePtrProvider<WaveForm<F>> for WaveFormProvider {
        #[inline]
        fn as_source_ptr(t: &WaveForm<F>) -> *mut sys::ma_data_source {
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

    impl<R: AsRmPtr> DataSourcePtrProvider<ResourceManagerSource<'_, R>>
        for ResourceManagerSourceProvider
    {
        #[inline]
        fn as_source_ptr(t: &ResourceManagerSource<'_, R>) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl<R: AsRmPtr> DataSourcePtrProvider<ResourceManagerBuffer<'_, R>>
        for ResourceManagerBufferProvider
    {
        #[inline]
        fn as_source_ptr(t: &ResourceManagerBuffer<'_, R>) -> *mut sys::ma_data_source {
            t.as_source().to_raw()
        }
    }

    impl<R: AsRmPtr> DataSourcePtrProvider<ResourceManagerStream<'_, R>>
        for ResourceManagerStreamProvider
    {
        #[inline]
        fn as_source_ptr(t: &ResourceManagerStream<'_, R>) -> *mut sys::ma_data_source {
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

pub trait SharedSource {
    type Format: PcmFormat;
}

// The types that DataSourceOps is implemented for are listed here.
impl<R: AsRmPtr> DataSourceOps for ResourceManagerSource<'_, R> {}
impl<R: AsRmPtr> DataSourceOps for ResourceManagerBuffer<'_, R> {}
impl<R: AsRmPtr> DataSourceOps for ResourceManagerStream<'_, R> {}

pub trait DataSourceOps: AsSourcePtr + SharedSource {
    fn read_pcm_frames_into(
        &mut self,
        dst: &mut [<Self::Format as PcmFormat>::PcmUnit],
    ) -> MaResult<usize> {
        let channels = self.data_format()?.channels;
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        data_source_ffi::ma_data_source_read_pcm_frames_into::<Self::Format, Self>(
            self, channels, dst,
        )
    }

    fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<SampleBuffer<Self::Format>> {
        // Is there a better way to get the channels?
        let channels = self.data_format()?.channels;
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        data_source_ffi::ma_data_source_read_pcm_frames::<Self::Format, Self>(
            self,
            frame_count,
            channels,
        )
    }

    fn seek_pcm_frames(&mut self, frame_count: u64) -> MaResult<u64> {
        data_source_ffi::ma_data_source_seek_pcm_frames(self, frame_count)
    }

    fn seek_to_pcm_frame(&mut self, frame_index: u64) -> MaResult<()> {
        data_source_ffi::ma_data_source_seek_to_pcm_frame(self, frame_index)
    }

    fn seek_seconds(&mut self, seconds: f32) -> MaResult<f32> {
        data_source_ffi::ma_data_source_seek_seconds(self, seconds)
    }

    fn seek_to_second(&mut self, seek_point: f32) -> MaResult<()> {
        data_source_ffi::ma_data_source_seek_to_second(self, seek_point)
    }

    fn data_format(&self) -> MaResult<DataFormat> {
        data_source_ffi::ma_data_source_get_data_format(self)
    }

    fn cursor_in_pcm_frames(&mut self) -> MaResult<u64> {
        data_source_ffi::ma_data_source_get_cursor_in_pcm_frames(self)
    }

    fn length_in_pcm_frames(&mut self) -> MaResult<u64> {
        data_source_ffi::ma_data_source_get_length_in_pcm_frames(self)
    }

    fn cursor_in_seconds(&mut self) -> MaResult<f32> {
        data_source_ffi::ma_data_source_get_cursor_in_seconds(self)
    }

    fn length_in_seconds(&mut self) -> MaResult<f32> {
        data_source_ffi::ma_data_source_get_length_in_seconds(self)
    }

    fn set_looping(&mut self, is_looping: bool) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_looping(self, is_looping)
    }

    fn looping(&self) -> bool {
        data_source_ffi::ma_data_source_is_looping(self)
    }

    fn range_in_pcm_frames(&self) -> core::ops::Range<u64> {
        data_source_ffi::ma_data_source_get_range_in_pcm_frames(self)
    }

    fn set_loop_point_in_pcm_frames(&mut self, begin: u64, end: u64) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_loop_point_in_pcm_frames(self, begin, end)
    }

    fn loop_point_in_pcm_frames(&self) -> core::ops::Range<u64> {
        data_source_ffi::ma_data_source_get_loop_point_in_pcm_frames(self)
    }

    fn set_current<S: AsSourcePtr + ?Sized>(&mut self, current: &mut S) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_current(self, current)
    }

    fn current(&self) -> Option<DataSourceRef<'_>> {
        data_source_ffi::ma_data_source_get_current(self)
    }

    // TODO:
    // fn set_next<S: AsSourcePtr + ?Sized>(&mut self, next: &mut S) -> MaResult<()> {
    //     data_source_ffi::ma_data_source_set_next(self, next)
    // }

    // TODO:
    // fn next(&self) -> Option<DataSourceRef<'_>> {
    //     data_source_ffi::ma_data_source_get_next(self)
    // }

    // TODO:
    // fn set_next_callback(&mut self, get_next_cb: GetNextCallback) -> MaResult<()> {
    //     data_source_ffi::ma_data_source_set_next_callback(self, get_next_cb)
    // }

    // TODO:
    // fn next_callback(&self) -> GetNextCallback {
    //     data_source_ffi::ma_data_source_get_next_callback(self)
    // }
}

pub(crate) mod data_source_ffi {
    use core::f32;
    use std::marker::PhantomData;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::{channels::Channel, formats::SampleBuffer},
        data_source::{
            private_data_source, AsSourcePtr, DataFormat, DataSource, DataSourceRef,
            GetNextCallback,
        },
        pcm_frames::{PcmFormat, PcmFormatInternal},
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

    pub fn ma_data_source_read_pcm_frames_into<F: PcmFormat, S: AsSourcePtr + ?Sized>(
        source: &mut S,
        channels: u32,
        dst: &mut [F::PcmUnit],
    ) -> MaResult<usize> {
        let frame_count = (dst.len() / channels as usize / F::VEC_PCM_UNITS_PER_FRAME) as u64;

        match F::DIRECT_READ {
            true => {
                // Read directly into destination
                let frames_read = ma_data_source_read_pcm_frames_internal(
                    source,
                    frame_count,
                    dst.as_mut_ptr() as *mut core::ffi::c_void,
                )?;
                Ok(frames_read as usize)
            }
            false => {
                let tmp_len = SampleBuffer::<F>::required_len(
                    frame_count as usize,
                    channels,
                    F::VEC_STORE_UNITS_PER_FRAME,
                )?;

                let mut tmp = vec![F::StorageUnit::default(); tmp_len];
                let frames_read = ma_data_source_read_pcm_frames_internal(
                    source,
                    frame_count,
                    tmp.as_mut_ptr() as *mut core::ffi::c_void,
                )?;

                let _ = <F as PcmFormatInternal>::read_from_storage_internal(
                    &tmp,
                    dst,
                    frames_read as usize,
                    channels as usize,
                )?;

                Ok(frames_read as usize)
            }
        }
    }

    pub fn ma_data_source_read_pcm_frames<F: PcmFormat, S: AsSourcePtr + ?Sized>(
        source: &mut S,
        frame_count: u64,
        channels: u32,
    ) -> MaResult<SampleBuffer<F>> {
        let mut buffer = SampleBuffer::<F>::new_zeroed(frame_count as usize, channels)?;

        let frames_read = ma_data_source_read_pcm_frames_internal(
            source,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<F>::from_storage(buffer, frames_read as usize, channels)
    }

    #[inline]
    pub fn ma_data_source_read_pcm_frames_internal<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_data_source_read_pcm_frames(
                private_data_source::source_ptr(source),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_read)
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
        let mut channels: u32 = 0;
        let mut sample_rate: u32 = 0;
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
            channels,
            sample_rate: sample_rate.try_into()?,
            channel_map: Some(channel_map),
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
