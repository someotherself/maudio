//! In-memory PCM audio buffer.
//!
//! An `AudioBuffer` stores decoded audio samples and can be read, seeked, or
//! used as a [`DataSource`](crate::data_source::DataSource) by the engine.
use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::formats::{Format, SampleBuffer, SampleBufferS24},
    data_source::{private_data_source, AsSourcePtr, DataSourceRef},
    engine::AllocationCallbacks,
    Binding, ErrorKinds, MaResult, MaudioError,
};

/// Owned in-memory PCM audio buffer.
///
/// This type owns the underlying buffer allocation
pub struct AudioBuffer<'a> {
    inner: *mut sys::ma_audio_buffer,
    pub format: Format,
    pub channels: u32,
    _alloc_keepalive: PhantomData<&'a AllocationCallbacks>,
}

impl Binding for AudioBuffer<'_> {
    type Raw = *mut sys::ma_audio_buffer;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

/// Borrowed (zero-copy) PCM audio buffer.
///
/// `AudioBufferRef` does not own the underlying samples. It references an
/// existing buffer, so the original backing data (and any associated allocation
/// state) must outlive `'a`.
///
/// Use this when you want to work with an existing buffer without copying.
pub struct AudioBufferRef<'a> {
    inner: *mut sys::ma_audio_buffer,
    pub format: Format,
    pub channels: u32,
    _data_marker: PhantomData<&'a [u8]>,
    _alloc_keepalive: PhantomData<&'a AllocationCallbacks>,
}

impl Binding for AudioBufferRef<'_> {
    type Raw = *mut sys::ma_audio_buffer;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

// Keeps the as_buffer_ptr method on AsAudioBufferPtr private
mod private_abuffer {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait AudioBufferProvider<T: ?Sized> {
        fn as_buffer_ptr(t: &T) -> *mut sys::ma_audio_buffer;
    }

    pub struct AudioBufferPtrPrivider;
    pub struct AudioBufferRefPtrPrivider;

    impl<'a> AudioBufferProvider<AudioBuffer<'a>> for AudioBufferPtrPrivider {
        fn as_buffer_ptr(t: &AudioBuffer<'a>) -> *mut sys::ma_audio_buffer {
            t.to_raw()
        }
    }

    impl<'a> AudioBufferProvider<AudioBufferRef<'a>> for AudioBufferRefPtrPrivider {
        fn as_buffer_ptr(t: &AudioBufferRef<'a>) -> *mut sys::ma_audio_buffer {
            t.to_raw()
        }
    }

    pub fn buffer_ptr<T: AsAudioBufferPtr + ?Sized>(t: &T) -> *mut sys::ma_audio_buffer {
        <T as AsAudioBufferPtr>::__PtrProvider::as_buffer_ptr(t)
    }
}

// Allows AudioBuffer to pass as a DataSource
#[doc(hidden)]
impl AsSourcePtr for AudioBuffer<'_> {
    type __PtrProvider = private_data_source::AudioBufferProvider;
}

// Allows AudioBufferRef to pass as a DataSource
#[doc(hidden)]
impl<'a> AsSourcePtr for AudioBufferRef<'a> {
    type __PtrProvider = private_data_source::AudioBufferRefProvider;
}

/// Allows both AudioBuffer and AudioBufferRef to access the same methods
pub trait AsAudioBufferPtr {
    #[doc(hidden)]
    type __PtrProvider: private_abuffer::AudioBufferProvider<Self>;
    fn format(&self) -> Format;
    fn channels(&self) -> u32;
}

impl AsAudioBufferPtr for AudioBuffer<'_> {
    #[doc(hidden)]
    type __PtrProvider = private_abuffer::AudioBufferPtrPrivider;

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl AsAudioBufferPtr for AudioBufferRef<'_> {
    #[doc(hidden)]
    type __PtrProvider = private_abuffer::AudioBufferRefPtrPrivider;

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl<T: AsAudioBufferPtr + AsSourcePtr + ?Sized> AudioBufferOps for T {}

// TODO: Figure out a better, typed way to deal with the formats in read_pcm without creating 12 structs
// TODO: Read pcm frames from DataSourceOps: AsSourcePtr is also available
/// AudioBufferOps trait contains shared methods for [`AudioBuffer`] and [`AudioBufferRef`]
pub trait AudioBufferOps: AsAudioBufferPtr + AsSourcePtr {
    fn read_pcm_frames_u8(
        &mut self,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(SampleBuffer<u8>, u64)> {
        debug_assert!(
            matches!(self.format(), Format::U8),
            "Cannot read U8 from buffer with {:?} ",
            self.format()
        );
        if !matches!(self.format(), Format::U8) {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        buffer_ffi::ma_audio_buffer_read_pcm_frames_u8(self, frame_count, looping)
    }

    fn read_pcm_frames_i16(
        &mut self,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(SampleBuffer<i16>, u64)> {
        debug_assert!(
            matches!(self.format(), Format::S16),
            "Cannot read I16 from buffer with {:?} ",
            self.format()
        );
        if !matches!(self.format(), Format::S16) {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        buffer_ffi::ma_audio_buffer_read_pcm_frames_i16(self, frame_count, looping)
    }

    fn read_pcm_frames_i32(
        &mut self,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(SampleBuffer<i32>, u64)> {
        debug_assert!(
            matches!(self.format(), Format::S32),
            "Cannot read I32 from buffer with {:?} ",
            self.format()
        );
        if !matches!(self.format(), Format::S32) {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        buffer_ffi::ma_audio_buffer_read_pcm_frames_i32(self, frame_count, looping)
    }

    fn read_pcm_frames_f32(
        &mut self,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(SampleBuffer<f32>, u64)> {
        debug_assert!(
            matches!(self.format(), Format::F32),
            "Cannot read F32 from buffer with {:?} ",
            self.format()
        );
        if !matches!(self.format(), Format::F32) {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        buffer_ffi::ma_audio_buffer_read_pcm_frames_f32(self, frame_count, looping)
    }

    fn read_pcm_frames_s24_packed(
        &mut self,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(SampleBufferS24, u64)> {
        debug_assert!(
            matches!(self.format(), Format::S24),
            "Cannot read S24 from buffer with {:?} ",
            self.format()
        );
        if !matches!(self.format(), Format::S24) {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        buffer_ffi::ma_audio_buffer_read_pcm_frames_s24(self, frame_count, looping)
    }

    fn seek_to_pcm(&mut self, frame_index: u64) -> MaResult<()> {
        buffer_ffi::ma_audio_buffer_seek_to_pcm_frame(self, frame_index)
    }

    fn ended(&self) -> bool {
        buffer_ffi::ma_audio_buffer_at_end(self)
    }

    fn cursor_pcm(&self) -> MaResult<u64> {
        buffer_ffi::ma_audio_buffer_get_cursor_in_pcm_frames(self)
    }

    fn length_pcm(&self) -> MaResult<u64> {
        buffer_ffi::ma_audio_buffer_get_length_in_pcm_frames(self)
    }

    fn available_frames(&self) -> MaResult<u64> {
        buffer_ffi::ma_audio_buffer_get_available_frames(self)
    }

    fn as_source(&self) -> DataSourceRef<'_> {
        debug_assert!(!private_abuffer::buffer_ptr(self).is_null());
        let ptr = private_abuffer::buffer_ptr(self).cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

impl<'a> AudioBufferRef<'a> {
    fn new_with_cfg_internal(config: &AudioBufferBuilder<'a>) -> MaResult<Self> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_audio_buffer>> =
            Box::new(MaybeUninit::uninit());
        buffer_ffi::ma_audio_buffer_init(config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_audio_buffer = Box::into_raw(mem) as *mut sys::ma_audio_buffer;

        Ok(Self {
            inner,
            format: config.inner.format.try_into()?,
            channels: config.inner.channels,
            _data_marker: PhantomData,
            _alloc_keepalive: PhantomData,
        })
    }
}

impl<'a> AudioBuffer<'a> {
    fn copy_with_cfg_internal(config: &AudioBufferBuilder) -> MaResult<Self> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_audio_buffer>> =
            Box::new(MaybeUninit::uninit());

        buffer_ffi::ma_audio_buffer_init_copy(config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_audio_buffer = Box::into_raw(mem) as *mut sys::ma_audio_buffer;

        Ok(Self {
            inner,
            format: config.inner.format.try_into()?,
            channels: config.inner.channels,
            _alloc_keepalive: PhantomData,
        })
    }
}

pub(crate) mod buffer_ffi {
    use crate::{
        audio::formats::{SampleBuffer, SampleBufferS24},
        data_source::{
            sources::buffer::{private_abuffer, AsAudioBufferPtr, AudioBuffer, AudioBufferBuilder},
            AsSourcePtr,
        },
        Binding, MaResult, MaudioError,
    };
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_audio_buffer_init(
        config: &AudioBufferBuilder,
        buffer: *mut sys::ma_audio_buffer,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_init(&config.to_raw() as *const _, buffer) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_audio_buffer_init_copy(
        config: &AudioBufferBuilder,
        buffer: *mut sys::ma_audio_buffer,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_init_copy(&config.to_raw() as *const _, buffer) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_audio_buffer_uninit<A: AsAudioBufferPtr + AsSourcePtr + ?Sized>(buffer: &mut A) {
        unsafe {
            sys::ma_audio_buffer_uninit(private_abuffer::buffer_ptr(buffer));
        }
    }

    pub fn ma_audio_buffer_read_pcm_frames_u8<A: AsAudioBufferPtr + ?Sized>(
        audio_buffer: &mut A,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(SampleBuffer<u8>, u64)> {
        let mut buffer = audio_buffer
            .format()
            .new_u8(audio_buffer.channels(), frame_count)?;
        let frames_read = ma_audio_buffer_read_pcm_frames_internal(
            audio_buffer,
            frame_count,
            buffer.as_mut_ptr(),
            looping,
        )?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_audio_buffer_read_pcm_frames_i16<A: AsAudioBufferPtr + ?Sized>(
        audio_buffer: &mut A,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(SampleBuffer<i16>, u64)> {
        let mut buffer = audio_buffer
            .format()
            .new_s16(audio_buffer.channels(), frame_count)?;
        let frames_read = ma_audio_buffer_read_pcm_frames_internal(
            audio_buffer,
            frame_count,
            buffer.as_mut_ptr(),
            looping,
        )?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_audio_buffer_read_pcm_frames_i32<A: AsAudioBufferPtr + ?Sized>(
        audio_buffer: &mut A,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(SampleBuffer<i32>, u64)> {
        let mut buffer = audio_buffer
            .format()
            .new_s32(audio_buffer.channels(), frame_count)?;
        let frames_read = ma_audio_buffer_read_pcm_frames_internal(
            audio_buffer,
            frame_count,
            buffer.as_mut_ptr(),
            looping,
        )?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_audio_buffer_read_pcm_frames_f32<A: AsAudioBufferPtr + ?Sized>(
        audio_buffer: &mut A,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(SampleBuffer<f32>, u64)> {
        let mut buffer = audio_buffer
            .format()
            .new_f32(audio_buffer.channels(), frame_count)?;
        let frames_read = ma_audio_buffer_read_pcm_frames_internal(
            audio_buffer,
            frame_count,
            buffer.as_mut_ptr(),
            looping,
        )?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_audio_buffer_read_pcm_frames_s24<A: AsAudioBufferPtr + ?Sized>(
        audio_buffer: &mut A,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(SampleBufferS24, u64)> {
        let mut buffer = audio_buffer
            .format()
            .new_s24(audio_buffer.channels(), frame_count)?;
        let frames_read = ma_audio_buffer_read_pcm_frames_internal(
            audio_buffer,
            frame_count,
            buffer.as_mut_ptr(),
            looping,
        )?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    #[inline]
    fn ma_audio_buffer_read_pcm_frames_internal<A: AsAudioBufferPtr + ?Sized>(
        audio_buffer: &mut A,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
        looping: bool,
    ) -> MaResult<u64> {
        let looping = looping as u32;
        let frames_read = unsafe {
            sys::ma_audio_buffer_read_pcm_frames(
                private_abuffer::buffer_ptr(audio_buffer),
                buffer,
                frame_count,
                looping,
            )
        };
        Ok(frames_read)
    }

    #[inline]
    pub fn ma_audio_buffer_seek_to_pcm_frame<A: AsAudioBufferPtr + ?Sized>(
        buffer: &mut A,
        frame_index: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_audio_buffer_seek_to_pcm_frame(private_abuffer::buffer_ptr(buffer), frame_index)
        };
        MaudioError::check(res)
    }

    // TODO Keep private for now
    #[inline]
    pub fn ma_audio_buffer_map(
        buffer: &mut AudioBuffer,
        frames_out: *mut *mut core::ffi::c_void,
        frame_count: *mut u64,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_map(buffer.to_raw(), frames_out, frame_count) };
        MaudioError::check(res)
    }

    // TODO Keep private for now
    #[inline]
    pub fn ma_audio_buffer_unmap(buffer: &mut AudioBuffer, frame_count: u64) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_unmap(buffer.to_raw(), frame_count) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_audio_buffer_at_end<A: AsAudioBufferPtr + ?Sized>(buffer: &A) -> bool {
        let res =
            unsafe { sys::ma_audio_buffer_at_end(private_abuffer::buffer_ptr(buffer) as *const _) };
        res == 1
    }

    #[inline]
    pub fn ma_audio_buffer_get_cursor_in_pcm_frames<A: AsAudioBufferPtr + ?Sized>(
        buffer: &A,
    ) -> MaResult<u64> {
        let mut cursor = 0;
        let res = unsafe {
            sys::ma_audio_buffer_get_cursor_in_pcm_frames(
                private_abuffer::buffer_ptr(buffer) as *const _,
                &mut cursor,
            )
        };
        MaudioError::check(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_audio_buffer_get_length_in_pcm_frames<A: AsAudioBufferPtr + ?Sized>(
        buffer: &A,
    ) -> MaResult<u64> {
        let mut length = 0;
        let res = unsafe {
            sys::ma_audio_buffer_get_length_in_pcm_frames(
                private_abuffer::buffer_ptr(buffer) as *const _,
                &mut length,
            )
        };
        MaudioError::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_audio_buffer_get_available_frames<A: AsAudioBufferPtr + ?Sized>(
        buffer: &A,
    ) -> MaResult<u64> {
        let mut frames = 0;
        let res = unsafe {
            sys::ma_audio_buffer_get_available_frames(
                private_abuffer::buffer_ptr(buffer) as *const _,
                &mut frames,
            )
        };
        MaudioError::check(res)?;
        Ok(frames)
    }
}

impl Drop for AudioBuffer<'_> {
    fn drop(&mut self) {
        buffer_ffi::ma_audio_buffer_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

impl<'a> Drop for AudioBufferRef<'a> {
    fn drop(&mut self) {
        buffer_ffi::ma_audio_buffer_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub struct AudioBufferBuilder<'a> {
    inner: sys::ma_audio_buffer_config,
    alloc_cb: Option<&'a AllocationCallbacks>,
    // This type and AudioBufferRef must keep a lifetime to the data provided to AudioBufferBuilder
    // Otherwise, the data can be dropped and result in a dangling pointer
    _marker: PhantomData<&'a [u8]>,
}

impl Binding for AudioBufferBuilder<'_> {
    type Raw = sys::ma_audio_buffer_config;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            alloc_cb: None,
            _marker: PhantomData,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a> AudioBufferBuilder<'a> {
    // Used for u8, i16, i32 and f32
    #[inline]
    fn from_typed<T>(format: Format, channels: u32, frames: u64, data: &'a [T]) -> MaResult<Self> {
        let expected =
            (frames as usize)
                .checked_mul(channels as usize)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "frames * channels",
                    lhs: frames,
                    rhs: channels as u64,
                }))?;

        if data.len() != expected {
            return Err(MaudioError::new_ma_error(ErrorKinds::BufferSizeMismatch {
                context: "AudioBufferBuilder::from_typed",
                expected,
                actual: data.len(),
            }));
        }

        Ok(Self::init(
            format,
            channels,
            frames,
            data.as_ptr() as *const _,
            None,
        ))
    }

    #[inline]
    fn from_s24_packed_impl(channels: u32, frames: u64, data: &'a [u8]) -> MaResult<Self> {
        let expected = (frames as usize)
            .checked_mul(channels as usize)
            .and_then(|v| v.checked_mul(3))
            .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                op: "frames * channels",
                lhs: frames,
                rhs: channels as u64,
            }))?;

        if data.len() != expected {
            return Err(MaudioError::new_ma_error(ErrorKinds::BufferSizeMismatch {
                context: "AudioBufferBuilder::from_s24_packed",
                expected,
                actual: data.len(),
            }));
        }

        Ok(Self::init(
            Format::S24,
            channels,
            frames,
            data.as_ptr() as *const _,
            None,
        ))
    }

    pub fn from_u8(channels: u32, frames: u64, data: &'a [u8]) -> MaResult<Self> {
        Self::from_typed(Format::U8, channels, frames, data)
    }

    pub fn from_s16(channels: u32, frames: u64, data: &'a [i16]) -> MaResult<Self> {
        Self::from_typed(Format::S16, channels, frames, data)
    }

    pub fn from_s32(channels: u32, frames: u64, data: &'a [i32]) -> MaResult<Self> {
        Self::from_typed(Format::S32, channels, frames, data)
    }

    pub fn from_f32(channels: u32, frames: u64, data: &'a [f32]) -> MaResult<Self> {
        Self::from_typed(Format::F32, channels, frames, data)
    }

    pub fn from_s24_packed(channels: u32, frames: u64, data: &'a [u8]) -> MaResult<Self> {
        Self::from_s24_packed_impl(channels, frames, data)
    }

    pub fn build_copy(&self) -> MaResult<AudioBuffer<'a>> {
        AudioBuffer::copy_with_cfg_internal(self)
    }

    pub fn build_ref(&self) -> MaResult<AudioBufferRef<'a>> {
        AudioBufferRef::new_with_cfg_internal(self)
    }

    pub(crate) fn init(
        format: Format,
        channels: u32,
        size_frames: u64,
        data: *const core::ffi::c_void,
        alloc_cb: Option<&'a AllocationCallbacks>,
    ) -> Self {
        let alloc: *const sys::ma_allocation_callbacks =
            alloc_cb.map_or(core::ptr::null(), |c| &c.inner as *const _);

        let ptr = buffer_config_ffi::ma_audio_buffer_config_init(
            format,
            channels,
            size_frames,
            data,
            alloc,
        );

        AudioBufferBuilder::from_ptr(ptr)
    }
}

pub(crate) mod buffer_config_ffi {
    use maudio_sys::ffi as sys;

    use crate::audio::formats::Format;

    pub fn ma_audio_buffer_config_init(
        format: Format,
        channels: u32,
        size_frames: u64,
        data: *const core::ffi::c_void,
        alloc_cb: *const sys::ma_allocation_callbacks,
    ) -> sys::ma_audio_buffer_config {
        unsafe {
            sys::ma_audio_buffer_config_init(format.into(), channels, size_frames, data, alloc_cb)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::data_source::sources::buffer::{AudioBufferBuilder, AudioBufferOps};

    fn ramp_f32_interleaved(channels: u32, frames: u64) -> Vec<f32> {
        let mut data = vec![0.0f32; (channels as usize) * (frames as usize)];
        for f in 0..frames as usize {
            for c in 0..channels as usize {
                // unique value per (frame, channel)
                data[f * channels as usize + c] = (f as f32) * 10.0 + (c as f32);
            }
        }
        data
    }

    fn ramp_u8_interleaved(channels: u32, frames: u64) -> Vec<u8> {
        let mut data = vec![0u8; (channels as usize) * (frames as usize)];
        for f in 0..frames as usize {
            for c in 0..channels as usize {
                // keep it simple + deterministic
                data[f * channels as usize + c] = (f as u8).wrapping_mul(10).wrapping_add(c as u8);
            }
        }
        data
    }

    // Helper that tries to be compatible with your SampleBuffer API.
    // Assumes SampleBuffer<T> exposes `.len()` as total samples (frames * channels).
    fn assert_sample_len<T>(
        samples: &crate::audio::formats::SampleBuffer<T>,
        channels: u32,
        frames: u64,
    ) {
        let expected = (channels as usize) * (frames as usize);
        assert_eq!(samples.len_samples(), expected);
    }

    #[test]
    fn test_audio_buffer_basic_init() {
        let mut data = Vec::new();
        data.resize_with(2 * 100, || 0.0f32);
        let _buffer = AudioBufferBuilder::from_f32(2, 100, &data)
            .unwrap()
            .build_copy()
            .unwrap();
    }

    #[test]
    fn test_audio_buffer_copy_basic_init_invariants() {
        let data = vec![0.0f32; 2 * 100];
        let buf = AudioBufferBuilder::from_f32(2, 100, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        assert_eq!(buf.cursor_pcm().unwrap(), 0);
        assert_eq!(buf.length_pcm().unwrap(), 100);
        assert_eq!(buf.available_frames().unwrap(), 100);
        assert!(!buf.ended());
    }

    #[test]
    fn test_audio_buffer_ref_basic_init_invariants() {
        let data = vec![0.0f32; 2 * 100];
        let buf = AudioBufferBuilder::from_f32(2, 100, &data)
            .unwrap()
            .build_ref()
            .unwrap();

        assert_eq!(buf.cursor_pcm().unwrap(), 0);
        assert_eq!(buf.length_pcm().unwrap(), 100);
        assert_eq!(buf.available_frames().unwrap(), 100);
        assert!(!buf.ended());

        // buf is dropped before data due to scope order; should compile.
        drop(buf);
        drop(data);
    }

    #[test]
    fn test_audio_buffer_read_advances_cursor_and_available() {
        let data = vec![0.0f32; 2 * 100];
        let mut buf = AudioBufferBuilder::from_f32(2, 100, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        let (_samples, frames_read) = buf.read_pcm_frames_f32(10, false).unwrap();
        assert_eq!(frames_read, 10);

        assert_eq!(buf.cursor_pcm().unwrap(), 10);
        assert_eq!(buf.available_frames().unwrap(), 90);
        assert!(!buf.ended());
    }

    #[test]
    fn test_audio_buffer_read_to_end_sets_ended() {
        let data = vec![0.0f32; 2 * 100];
        let mut buf = AudioBufferBuilder::from_f32(2, 100, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        let (_samples, frames_read) = buf.read_pcm_frames_f32(100, false).unwrap();
        assert_eq!(frames_read, 100);
        assert!(buf.ended());
        assert_eq!(buf.available_frames().unwrap(), 0);

        // Reading past the end should return 0 frames.
        let (_samples2, frames_read2) = buf.read_pcm_frames_f32(10, false).unwrap();
        assert_eq!(frames_read2, 0);
        assert!(buf.ended());
    }

    #[test]
    fn test_audio_buffer_read_zero_frames_is_noop() {
        let data = ramp_f32_interleaved(2, 16);
        let mut buf = AudioBufferBuilder::from_f32(2, 16, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        let (samples, frames_read) = buf.read_pcm_frames_f32(0, false).unwrap();
        assert_eq!(frames_read, 0);
        assert_sample_len(&samples, 2, 0);

        assert_eq!(buf.cursor_pcm().unwrap(), 0);
        assert_eq!(buf.available_frames().unwrap(), 16);
        assert!(!buf.ended());
    }

    #[test]
    fn test_audio_buffer_read_past_end_truncates_returned_buffer() {
        // frames=8, read 6, then request 6 again => only 2 available
        let data = ramp_f32_interleaved(2, 8);
        let mut buf = AudioBufferBuilder::from_f32(2, 8, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        let (_samples1, frames_read1) = buf.read_pcm_frames_f32(6, false).unwrap();
        assert_eq!(frames_read1, 6);
        assert_eq!(buf.cursor_pcm().unwrap(), 6);
        assert_eq!(buf.available_frames().unwrap(), 2);

        let (samples2, frames_read2) = buf.read_pcm_frames_f32(6, false).unwrap();
        assert_eq!(frames_read2, 2);
        assert_sample_len(&samples2, 2, 2);

        assert_eq!(buf.cursor_pcm().unwrap(), 8);
        assert_eq!(buf.available_frames().unwrap(), 0);
        assert!(buf.ended());
    }

    #[test]
    fn test_audio_buffer_seek_to_middle_updates_cursor_and_available() {
        let data = ramp_f32_interleaved(2, 32);
        let mut buf = AudioBufferBuilder::from_f32(2, 32, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        buf.seek_to_pcm(10).unwrap();
        assert_eq!(buf.cursor_pcm().unwrap(), 10);
        assert_eq!(buf.available_frames().unwrap(), 22);
        assert!(!buf.ended());

        let (_samples, frames_read) = buf.read_pcm_frames_f32(5, false).unwrap();
        assert_eq!(frames_read, 5);
        assert_eq!(buf.cursor_pcm().unwrap(), 15);
        assert_eq!(buf.available_frames().unwrap(), 17);
    }

    #[test]
    fn test_audio_buffer_seek_to_end_sets_ended_state() {
        let data = ramp_f32_interleaved(2, 12);
        let mut buf = AudioBufferBuilder::from_f32(2, 12, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        buf.seek_to_pcm(12).unwrap();
        assert_eq!(buf.cursor_pcm().unwrap(), 12);
        assert_eq!(buf.available_frames().unwrap(), 0);
        assert!(buf.ended());

        // reading at end should return 0 frames
        let (samples, frames_read) = buf.read_pcm_frames_f32(1, false).unwrap();
        assert_eq!(frames_read, 0);
        assert_sample_len(&samples, 2, 0);
        assert!(buf.ended());
    }

    #[test]
    fn test_audio_buffer_seek_past_end_errors() {
        let data = ramp_f32_interleaved(2, 12);
        let mut buf = AudioBufferBuilder::from_f32(2, 12, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        // miniaudio should reject seeking beyond length
        assert!(buf.seek_to_pcm(13).is_err());
    }

    #[test]
    fn test_audio_buffer_read_u8_advances_cursor_and_truncates() {
        let data = ramp_u8_interleaved(2, 8);
        let mut buf = AudioBufferBuilder::from_u8(2, 8, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        let (samples1, frames_read1) = buf.read_pcm_frames_u8(6, false).unwrap();
        assert_eq!(frames_read1, 6);
        assert_sample_len(&samples1, 2, 6);
        assert_eq!(buf.cursor_pcm().unwrap(), 6);
        assert_eq!(buf.available_frames().unwrap(), 2);

        let (samples2, frames_read2) = buf.read_pcm_frames_u8(10, false).unwrap();
        assert_eq!(frames_read2, 2);
        assert_sample_len(&samples2, 2, 2);
        assert_eq!(buf.cursor_pcm().unwrap(), 8);
        assert_eq!(buf.available_frames().unwrap(), 0);
        assert!(buf.ended());
    }

    #[test]
    fn test_audio_buffer_looping_read_does_not_return_zero_when_past_end() {
        // Conservative looping test:
        // - In non-looping, this would return 0 after consuming the buffer.
        // - With looping=true, requesting more should give >0 frames read.
        let data = ramp_f32_interleaved(2, 4);
        let mut buf = AudioBufferBuilder::from_f32(2, 4, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        // Consume all frames.
        let (_s0, r0) = buf.read_pcm_frames_f32(4, false).unwrap();
        assert_eq!(r0, 4);
        assert!(buf.ended());

        // Looping read should produce some frames (ideally 2).
        let (_s1, r1) = buf.read_pcm_frames_f32(2, true).unwrap();
        assert!(r1 > 0, "looping read should return >0 frames");
    }

    #[test]
    fn test_builder_rejects_wrong_len_u8() {
        let data = vec![0u8; 2 * 10 - 1]; // should be 20
        assert!(AudioBufferBuilder::from_u8(2, 10, &data).is_err());
    }

    #[test]
    fn test_builder_rejects_wrong_len_s16() {
        let data = vec![0i16; 2 * 10 + 1]; // should be 20
        assert!(AudioBufferBuilder::from_s16(2, 10, &data).is_err());
    }

    #[test]
    fn test_builder_rejects_wrong_len_s32() {
        let data = vec![0i32; 2 * 10 - 2]; // should be 20
        assert!(AudioBufferBuilder::from_s32(2, 10, &data).is_err());
    }

    #[test]
    fn test_builder_rejects_wrong_len_f32() {
        let data = vec![0.0f32; 2 * 10 + 3]; // should be 20
        assert!(AudioBufferBuilder::from_f32(2, 10, &data).is_err());
    }

    #[test]
    fn test_builder_s24_packed_requires_frames_channels_times_3_bytes() {
        // frames=4, channels=2 => 4*2*3 = 24 bytes
        let ok = vec![0u8; 24];
        assert!(AudioBufferBuilder::from_s24_packed(2, 4, &ok).is_ok());

        let bad = vec![0u8; 23];
        assert!(AudioBufferBuilder::from_s24_packed(2, 4, &bad).is_err());
    }

    #[test]
    fn test_builder_checked_mul_overflow_returns_err() {
        // This intentionally tries to overflow the usize multiplication in from_typed.
        // It should return BufferSizeError instead of panicking/wrapping.
        //
        // Pick frames/channels so frames*channels > usize::MAX. Use a tiny slice so we don't allocate.
        let data = [0.0f32; 1];
        let too_many_frames = u64::MAX;
        let channels = 2u32;

        assert!(AudioBufferBuilder::from_f32(channels, too_many_frames, &data).is_err());
    }
}
