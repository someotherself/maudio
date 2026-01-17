use std::{marker::PhantomData, ops::Range};

use maudio_sys::ffi as sys;

use crate::{Binding, MaResult, audio::formats::Format};

pub mod sources;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataFormat {
    pub format: Format,
    pub channels: u32,
    pub sample_rate: u32,
    /// Channel order/map for each channel, length == channels (when available).
    pub channel_map: Vec<sys::ma_channel>,
}

pub struct DataSource {
    inner: *mut sys::ma_data_source,
}

pub type GetNextCallback = sys::ma_data_source_get_next_proc;

impl Binding for DataSource {
    type Raw = *mut sys::ma_data_source;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

pub trait AsSourcePtr {
    fn as_source_ptr(&self) -> *mut sys::ma_data_source;
}

impl AsSourcePtr for DataSource {
    fn as_source_ptr(&self) -> *mut sys::ma_data_source {
        self.to_raw()
    }
}

impl<'a> AsSourcePtr for DataSourceRef<'a> {
    fn as_source_ptr(&self) -> *mut sys::ma_data_source {
        self.to_raw()
    }
}

impl<T: AsSourcePtr + ?Sized> DataSourceOps for T {}

pub trait DataSourceOps: AsSourcePtr {
    fn read_pcm_frames(&mut self, frame_count: u64, channels: u32) -> MaResult<(Vec<f32>, u64)> {
        data_source_ffi::ma_data_source_read_pcm_frames(self, frame_count, channels)
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

    fn data_format(&mut self) -> MaResult<DataFormat> {
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
        Binding, MaRawResult, MaResult,
        data_source::{AsSourcePtr, DataFormat, DataSource, DataSourceRef, GetNextCallback},
    };

    #[inline]
    pub fn ma_data_source_init(
        config: *const sys::ma_data_source_config,
        source: *mut sys::ma_data_source,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_data_source_init(config, source) };
        MaRawResult::check(res)
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
                source.as_source_ptr(),
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
                frame_count,
                &mut frames_read,
            )
        };
        MaRawResult::check(res)?;
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
                source.as_source_ptr(),
                frame_count,
                &mut frames_seeked,
            )
        };
        MaRawResult::check(res)?;
        Ok(frames_seeked)
    }

    #[inline]
    pub fn ma_data_source_seek_to_pcm_frame<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        frame_index: u64,
    ) -> MaResult<()> {
        let res =
            unsafe { sys::ma_data_source_seek_to_pcm_frame(source.as_source_ptr(), frame_index) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_data_source_seek_seconds<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        seconds: f32,
    ) -> MaResult<f32> {
        let mut seconds_seeked = 0.0f32;
        let res = unsafe {
            sys::ma_data_source_seek_seconds(source.as_source_ptr(), seconds, &mut seconds_seeked)
        };
        MaRawResult::check(res)?;
        Ok(seconds_seeked)
    }

    #[inline]
    pub fn ma_data_source_seek_to_second<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        seek_point: f32,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_data_source_seek_to_second(source.as_source_ptr(), seek_point) };
        MaRawResult::check(res)
    }

    pub fn ma_data_source_get_data_format<S: AsSourcePtr + ?Sized>(
        source: &mut S,
    ) -> MaResult<DataFormat> {
        let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        let mut channels: sys::ma_uint32 = 0;
        let mut sample_rate: sys::ma_uint32 = 0;
        let mut channel_map = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];
        let res = unsafe {
            sys::ma_data_source_get_data_format(
                source.as_source_ptr(),
                &mut format_raw,
                &mut channels,
                &mut sample_rate,
                channel_map.as_mut_ptr(),
                channel_map.len(),
            )
        };
        MaRawResult::check(res)?;
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
            sys::ma_data_source_get_cursor_in_pcm_frames(source.as_source_ptr(), &mut cursor)
        };
        MaRawResult::check(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_data_source_get_length_in_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &mut S,
    ) -> MaResult<u64> {
        let mut length = 0;
        let res = unsafe {
            sys::ma_data_source_get_length_in_pcm_frames(source.as_source_ptr(), &mut length)
        };
        MaRawResult::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_data_source_get_cursor_in_seconds<S: AsSourcePtr + ?Sized>(
        source: &mut S,
    ) -> MaResult<f32> {
        let mut cursor = 0.0f32;
        let res = unsafe {
            sys::ma_data_source_get_cursor_in_seconds(source.as_source_ptr(), &mut cursor)
        };
        MaRawResult::check(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_data_source_get_length_in_seconds<S: AsSourcePtr + ?Sized>(
        source: &mut S,
    ) -> MaResult<f32> {
        let mut length = 0.0f32;
        let res = unsafe {
            sys::ma_data_source_get_length_in_seconds(source.as_source_ptr(), &mut length)
        };
        MaRawResult::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_data_source_set_looping<S: AsSourcePtr + ?Sized>(
        source: &mut S,
        is_looping: bool,
    ) -> MaResult<()> {
        let is_looping = is_looping as u32;
        let res = unsafe { sys::ma_data_source_set_looping(source.as_source_ptr(), is_looping) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_data_source_is_looping<S: AsSourcePtr + ?Sized>(source: &S) -> bool {
        let res = unsafe { sys::ma_data_source_is_looping(source.as_source_ptr() as *const _) };
        res == 1
    }

    #[inline]
    pub fn ma_data_source_get_range_in_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &S,
    ) -> core::ops::Range<u64> {
        let mut begin = 0;
        let mut end = 0;
        unsafe {
            sys::ma_data_source_get_range_in_pcm_frames(
                source.as_source_ptr() as *const _,
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
            sys::ma_data_source_set_loop_point_in_pcm_frames(source.as_source_ptr(), begin, end)
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_data_source_get_loop_point_in_pcm_frames<S: AsSourcePtr + ?Sized>(
        source: &S,
    ) -> core::ops::Range<u64> {
        let mut begin = 0;
        let mut end: u64 = 0;
        unsafe {
            sys::ma_data_source_get_loop_point_in_pcm_frames(
                source.as_source_ptr() as *const _,
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
            sys::ma_data_source_set_current(source.as_source_ptr(), current.as_source_ptr())
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_data_source_get_current<'a, S: AsSourcePtr + ?Sized>(
        source: &'a S,
    ) -> Option<DataSourceRef<'a>> {
        let ptr = unsafe { sys::ma_data_source_get_current(source.as_source_ptr() as *const _) };
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
        let res =
            unsafe { sys::ma_data_source_set_next(source.as_source_ptr(), next.as_source_ptr()) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_data_source_get_next<'a, S: AsSourcePtr + ?Sized>(
        source: &'a S,
    ) -> Option<DataSourceRef<'a>> {
        let ptr = unsafe { sys::ma_data_source_get_next(source.as_source_ptr() as *const _) };
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
        let res =
            unsafe { sys::ma_data_source_set_next_callback(source.as_source_ptr(), get_next_cb) };
        MaRawResult::check(res)
    }

    // TODO
    #[inline]
    pub fn ma_data_source_get_next_callback<S: AsSourcePtr + ?Sized>(
        source: &S,
    ) -> GetNextCallback {
        unsafe { sys::ma_data_source_get_next_callback(source.as_source_ptr() as *const _) }
    }
}

pub struct DataSourceRef<'a> {
    inner: *mut sys::ma_data_source,
    _marker: PhantomData<&'a ()>,
}

impl<'a> Binding for DataSourceRef<'a> {
    type Raw = *mut sys::ma_data_source;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}
