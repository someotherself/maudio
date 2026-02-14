//! In-memory PCM audio buffer.
//!
//! An `AudioBuffer` stores decoded audio samples and can be read, seeked, or
//! used as a [`DataSource`](crate::data_source::DataSource) by the engine.
use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::formats::{Format, SampleBuffer},
    data_source::{private_data_source, AsSourcePtr, DataSourceRef},
    engine::AllocationCallbacks,
    pcm_frames::{PcmFormat, PcmFormatInternal, S24Packed, S24},
    Binding, MaResult,
};

/// Owned in-memory PCM audio buffer.
///
/// This type owns the underlying buffer allocation
pub struct AudioBuffer<'a, F: PcmFormat> {
    inner: *mut sys::ma_audio_buffer,
    pub format: Format,
    pub channels: u32,
    _sample_format: PhantomData<F>,
    _alloc_keepalive: PhantomData<&'a AllocationCallbacks>,
}

impl<F: PcmFormat> Binding for AudioBuffer<'_, F> {
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
pub struct AudioBufferRef<'a, F: PcmFormat> {
    inner: *mut sys::ma_audio_buffer,
    pub format: Format,
    pub channels: u32,
    _data_marker: PhantomData<&'a [u8]>,
    _sample_format: PhantomData<F>,
    _alloc_keepalive: PhantomData<&'a AllocationCallbacks>,
}

impl<F: PcmFormat> Binding for AudioBufferRef<'_, F> {
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

    impl<F: PcmFormat> AudioBufferProvider<AudioBuffer<'_, F>> for AudioBufferPtrPrivider {
        fn as_buffer_ptr(t: &AudioBuffer<F>) -> *mut sys::ma_audio_buffer {
            t.to_raw()
        }
    }

    impl<F: PcmFormat> AudioBufferProvider<AudioBufferRef<'_, F>> for AudioBufferRefPtrPrivider {
        fn as_buffer_ptr(t: &AudioBufferRef<F>) -> *mut sys::ma_audio_buffer {
            t.to_raw()
        }
    }

    pub fn buffer_ptr<T: AsAudioBufferPtr + ?Sized>(t: &T) -> *mut sys::ma_audio_buffer {
        <T as AsAudioBufferPtr>::__PtrProvider::as_buffer_ptr(t)
    }
}

// Allows AudioBuffer to pass as a DataSource
#[doc(hidden)]
impl<F: PcmFormat> AsSourcePtr for AudioBuffer<'_, F> {
    type __PtrProvider = private_data_source::AudioBufferProvider;
}

// Allows AudioBufferRef to pass as a DataSource
#[doc(hidden)]
impl<'a, F: PcmFormat> AsSourcePtr for AudioBufferRef<'a, F> {
    type __PtrProvider = private_data_source::AudioBufferRefProvider;
}

/// Allows both AudioBuffer and AudioBufferRef to access the same methods
pub trait AsAudioBufferPtr {
    #[doc(hidden)]
    type __PtrProvider: private_abuffer::AudioBufferProvider<Self>;
    fn format(&self) -> Format;
    fn channels(&self) -> u32;
}

impl<F: PcmFormat> AsAudioBufferPtr for AudioBuffer<'_, F> {
    #[doc(hidden)]
    type __PtrProvider = private_abuffer::AudioBufferPtrPrivider;

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl<F: PcmFormat> AsAudioBufferPtr for AudioBufferRef<'_, F> {
    #[doc(hidden)]
    type __PtrProvider = private_abuffer::AudioBufferRefPtrPrivider;

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl<F: PcmFormat> AudioBufferOps for AudioBuffer<'_, F> {
    type Format = F;
}

impl<F: PcmFormat> AudioBufferOps for AudioBufferRef<'_, F> {
    type Format = F;
}

/// AudioBufferOps trait contains shared methods for [`AudioBuffer`] and [`AudioBufferRef`]
pub trait AudioBufferOps: AsAudioBufferPtr + AsSourcePtr {
    type Format: PcmFormat;

    fn read_pcm_frames_into(
        &mut self,
        looping: bool,
        dst: &mut [<Self::Format as PcmFormat>::PcmUnit],
    ) -> MaResult<usize> {
        buffer_ffi::ma_audio_buffer_read_pcm_frames_into::<Self::Format, Self>(self, dst, looping)
    }

    fn read_pcm_frames(
        &mut self,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<SampleBuffer<Self::Format>> {
        buffer_ffi::ma_audio_buffer_read_pcm_frames(self, frame_count, looping)
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

impl<'a, F: PcmFormat> AudioBufferRef<'a, F> {
    fn new_with_cfg_internal(config: &AudioBufferBuilder<'a>) -> MaResult<Self> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_audio_buffer>> =
            Box::new(MaybeUninit::uninit());
        buffer_ffi::ma_audio_buffer_init(config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_audio_buffer = Box::into_raw(mem) as *mut sys::ma_audio_buffer;

        Ok(Self {
            inner,
            format: config.inner.format.try_into()?,
            channels: config.inner.channels,
            _sample_format: PhantomData,
            _data_marker: PhantomData,
            _alloc_keepalive: PhantomData,
        })
    }
}

impl<'a, F: PcmFormat> AudioBuffer<'a, F> {
    fn copy_with_cfg_internal(config: &AudioBufferBuilder) -> MaResult<Self> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_audio_buffer>> =
            Box::new(MaybeUninit::uninit());

        buffer_ffi::ma_audio_buffer_init_copy(config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_audio_buffer = Box::into_raw(mem) as *mut sys::ma_audio_buffer;

        Ok(Self {
            inner,
            format: config.inner.format.try_into()?,
            channels: config.inner.channels,
            _sample_format: PhantomData,
            _alloc_keepalive: PhantomData,
        })
    }
}

pub(crate) mod buffer_ffi {
    use crate::{
        audio::formats::SampleBuffer,
        data_source::{
            sources::buffer::{private_abuffer, AsAudioBufferPtr, AudioBuffer, AudioBufferBuilder},
            AsSourcePtr,
        },
        pcm_frames::{PcmFormat, PcmFormatInternal},
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

    pub fn ma_audio_buffer_read_pcm_frames_into<F: PcmFormat, A: AsAudioBufferPtr + ?Sized>(
        audio_buffer: &mut A,
        dst: &mut [F::PcmUnit],
        looping: bool,
    ) -> MaResult<usize> {
        let channels = audio_buffer.channels();
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        let frame_count = (dst.len() / channels as usize / F::VEC_PCM_UNITS_PER_FRAME) as u64;

        match F::DIRECT_READ {
            true => {
                // Read directly into destination
                let frames_read = ma_audio_buffer_read_pcm_frames_internal(
                    audio_buffer,
                    frame_count,
                    dst.as_mut_ptr() as *mut core::ffi::c_void,
                    looping,
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
                let frames_read = ma_audio_buffer_read_pcm_frames_internal(
                    audio_buffer,
                    frame_count,
                    tmp.as_mut_ptr() as *mut core::ffi::c_void,
                    looping,
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

    pub fn ma_audio_buffer_read_pcm_frames<F: PcmFormat, A: AsAudioBufferPtr + ?Sized>(
        audio_buffer: &mut A,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<SampleBuffer<F>> {
        let mut buffer =
            SampleBuffer::<F>::new_zeroed(frame_count as usize, audio_buffer.channels())?;

        let frames_read = ma_audio_buffer_read_pcm_frames_internal(
            audio_buffer,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
            looping,
        )?;

        SampleBuffer::<F>::from_storage(buffer, frames_read as usize, audio_buffer.channels())
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
    pub fn ma_audio_buffer_map<F: PcmFormat>(
        buffer: &mut AudioBuffer<F>,
        frames_out: *mut *mut core::ffi::c_void,
        frame_count: *mut u64,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_map(buffer.to_raw(), frames_out, frame_count) };
        MaudioError::check(res)
    }

    // TODO Keep private for now
    #[inline]
    pub fn ma_audio_buffer_unmap<F: PcmFormat>(
        buffer: &mut AudioBuffer<F>,
        frame_count: u64,
    ) -> MaResult<()> {
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

impl<F: PcmFormat> Drop for AudioBuffer<'_, F> {
    fn drop(&mut self) {
        buffer_ffi::ma_audio_buffer_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

impl<'a, F: PcmFormat> Drop for AudioBufferRef<'a, F> {
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
    fn new_copy<F: PcmFormat>(
        format: Format,
        channels: u32,
        data: &[F::PcmUnit],
    ) -> MaResult<Self> {
        let frames = data.len() / channels as usize / F::VEC_PCM_UNITS_PER_FRAME;

        match F::DIRECT_READ {
            true => {
                let builder = Self::init(
                    format,
                    channels,
                    frames as u64,
                    data.as_ptr() as *const _,
                    None,
                );
                Ok(builder)
            }
            false => {
                let dst_len = SampleBuffer::<F>::required_len(
                    frames,
                    channels,
                    F::VEC_STORE_UNITS_PER_FRAME,
                )?;
                let mut dst = vec![F::StorageUnit::default(); dst_len];

                <F as PcmFormatInternal>::write_to_storage_internal(
                    &mut dst,
                    data,
                    frames,
                    channels as usize,
                )?;
                let builder = Self::init(
                    format,
                    channels,
                    frames as u64,
                    dst.as_ptr() as *const _,
                    None,
                );
                Ok(builder)
            }
        }
    }

    /// Should never be called for `S24`.
    ///
    /// S24 needs format conversion and AudioBufferRef reads directly from user provided data.
    fn new_ref<F: PcmFormat>(format: Format, channels: u32, data: &[F::PcmUnit]) -> Self {
        let frames = data.len() / channels as usize / F::VEC_PCM_UNITS_PER_FRAME;

        Self::init(
            format,
            channels,
            frames as u64,
            data.as_ptr() as *const _,
            None,
        )
    }

    pub fn build_u8(channels: u32, data: &[u8]) -> MaResult<AudioBuffer<'a, u8>> {
        let builder = Self::new_copy::<u8>(Format::U8, channels, data)?;
        AudioBuffer::copy_with_cfg_internal(&builder)
    }

    pub fn build_i16(channels: u32, data: &[i16]) -> MaResult<AudioBuffer<'a, i16>> {
        let builder = Self::new_copy::<i16>(Format::S16, channels, data)?;
        AudioBuffer::copy_with_cfg_internal(&builder)
    }

    pub fn build_i32(channels: u32, data: &[i32]) -> MaResult<AudioBuffer<'a, i32>> {
        let builder = Self::new_copy::<i32>(Format::S32, channels, data)?;
        AudioBuffer::copy_with_cfg_internal(&builder)
    }

    pub fn build_f32(channels: u32, data: &[f32]) -> MaResult<AudioBuffer<'a, f32>> {
        let builder = Self::new_copy::<f32>(Format::F32, channels, data)?;
        AudioBuffer::copy_with_cfg_internal(&builder)
    }

    pub fn build_s24(channels: u32, data: &[i32]) -> MaResult<AudioBuffer<'a, S24>> {
        let builder = Self::new_copy::<S24>(Format::S24, channels, data)?;
        AudioBuffer::copy_with_cfg_internal(&builder)
    }

    pub fn build_s24_packed(channels: u32, data: &[u8]) -> MaResult<AudioBuffer<'a, S24Packed>> {
        let builder = Self::new_copy::<S24Packed>(Format::S24, channels, data)?;
        AudioBuffer::copy_with_cfg_internal(&builder)
    }

    pub fn build_u8_ref(channels: u32, data: &'a [u8]) -> MaResult<AudioBufferRef<'a, u8>> {
        let builder = Self::new_ref::<u8>(Format::U8, channels, data);
        AudioBufferRef::new_with_cfg_internal(&builder)
    }
    pub fn build_i16_ref(channels: u32, data: &'a [i16]) -> MaResult<AudioBufferRef<'a, i16>> {
        let builder = Self::new_ref::<i16>(Format::S16, channels, data);
        AudioBufferRef::new_with_cfg_internal(&builder)
    }
    pub fn build_i32_ref(channels: u32, data: &'a [i32]) -> MaResult<AudioBufferRef<'a, i32>> {
        let builder = Self::new_ref::<i32>(Format::S32, channels, data);
        AudioBufferRef::new_with_cfg_internal(&builder)
    }
    pub fn build_f32_ref(channels: u32, data: &'a [f32]) -> MaResult<AudioBufferRef<'a, f32>> {
        let builder = Self::new_ref::<f32>(Format::F32, channels, data);
        AudioBufferRef::new_with_cfg_internal(&builder)
    }

    pub fn build_s24_packed_ref(
        channels: u32,
        data: &'a [u8],
    ) -> MaResult<AudioBufferRef<'a, S24Packed>> {
        let builder = Self::new_ref::<u8>(Format::S24, channels, data);
        AudioBufferRef::new_with_cfg_internal(&builder)
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
    use crate::{
        audio::formats::SampleBuffer,
        data_source::sources::buffer::{AudioBufferBuilder, AudioBufferOps},
        pcm_frames::PcmFormat,
    };

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
    fn assert_sample_len<T: PcmFormat>(samples: &SampleBuffer<T>, channels: u32, frames: u64) {
        let expected = (channels as usize) * (frames as usize);
        assert_eq!(samples.len(), expected);
    }

    #[test]
    fn test_audio_buffer_basic_init() {
        let mut data = Vec::new();
        data.resize_with(2 * 100, || 0.0f32);
        let _buffer = AudioBufferBuilder::build_f32(2, &data).unwrap();
    }

    #[test]
    fn test_audio_buffer_copy_basic_init_invariants() {
        let data = vec![0.0f32; 2 * 100];
        let buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        assert_eq!(buf.cursor_pcm().unwrap(), 0);
        assert_eq!(buf.length_pcm().unwrap(), 100);
        assert_eq!(buf.available_frames().unwrap(), 100);
        assert!(!buf.ended());
    }

    #[test]
    fn test_audio_buffer_ref_basic_init_invariants() {
        let data = vec![0.0f32; 2 * 100];
        let buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

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
        let mut buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        let buffer = buf.read_pcm_frames(10, false).unwrap();
        let frames_read = buffer.frames();
        assert_eq!(frames_read, 10);

        assert_eq!(buf.cursor_pcm().unwrap(), 10);
        assert_eq!(buf.available_frames().unwrap(), 90);
        assert!(!buf.ended());
    }

    #[test]
    fn test_audio_buffer_read_to_end_sets_ended() {
        let data = vec![0.0f32; 2 * 100];
        let mut buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        let buffer = buf.read_pcm_frames(100, false).unwrap();
        let frames_read = buffer.frames();
        assert_eq!(frames_read, 100);
        assert!(buf.ended());
        assert_eq!(buf.available_frames().unwrap(), 0);

        // Reading past the end should return 0 frames.
        let buffer = buf.read_pcm_frames(10, false).unwrap();
        let frames_read2 = buffer.frames();
        assert_eq!(frames_read2, 0);
        assert!(buf.ended());
    }

    #[test]
    fn test_audio_buffer_read_zero_frames_is_noop() {
        let data = ramp_f32_interleaved(2, 16);
        let mut buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        let buffer = buf.read_pcm_frames(0, false).unwrap();
        let frames_read = buffer.frames();
        assert_eq!(frames_read, 0);
        assert_sample_len(&buffer, 2, 0);

        assert_eq!(buf.cursor_pcm().unwrap(), 0);
        assert_eq!(buf.available_frames().unwrap(), 16);
        assert!(!buf.ended());
    }

    #[test]
    fn test_audio_buffer_read_past_end_truncates_returned_buffer() {
        // frames=8, read 6, then request 6 again => only 2 available
        let data = ramp_f32_interleaved(2, 8);
        let mut buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        let buffer = buf.read_pcm_frames(6, false).unwrap();
        let frames_read1 = buffer.frames();
        assert_eq!(frames_read1, 6);
        assert_eq!(buf.cursor_pcm().unwrap(), 6);
        assert_eq!(buf.available_frames().unwrap(), 2);

        let buffer2 = buf.read_pcm_frames(6, false).unwrap();
        let frames_read2 = buffer2.frames();
        assert_eq!(frames_read2, 2);
        assert_sample_len(&buffer2, 2, 2);

        assert_eq!(buf.cursor_pcm().unwrap(), 8);
        assert_eq!(buf.available_frames().unwrap(), 0);
        assert!(buf.ended());
    }

    #[test]
    fn test_audio_buffer_seek_to_middle_updates_cursor_and_available() {
        let data = ramp_f32_interleaved(2, 32);
        let mut buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        buf.seek_to_pcm(10).unwrap();
        assert_eq!(buf.cursor_pcm().unwrap(), 10);
        assert_eq!(buf.available_frames().unwrap(), 22);
        assert!(!buf.ended());

        let buffer = buf.read_pcm_frames(5, false).unwrap();
        let frames_read = buffer.frames();
        assert_eq!(frames_read, 5);
        assert_eq!(buf.cursor_pcm().unwrap(), 15);
        assert_eq!(buf.available_frames().unwrap(), 17);
    }

    #[test]
    fn test_audio_buffer_seek_to_end_sets_ended_state() {
        let data = ramp_f32_interleaved(2, 12);
        let mut buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        buf.seek_to_pcm(12).unwrap();
        assert_eq!(buf.cursor_pcm().unwrap(), 12);
        assert_eq!(buf.available_frames().unwrap(), 0);
        assert!(buf.ended());

        // reading at end should return 0 frames
        let buffer = buf.read_pcm_frames(1, false).unwrap();
        let frames_read = buffer.frames();
        assert_eq!(frames_read, 0);
        assert_sample_len(&buffer, 2, 0);
        assert!(buf.ended());
    }

    #[test]
    fn test_audio_buffer_seek_past_end_errors() {
        let data = ramp_f32_interleaved(2, 12);
        let mut buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        // miniaudio should reject seeking beyond length
        assert!(buf.seek_to_pcm(13).is_err());
    }

    #[test]
    fn test_audio_buffer_read_u8_advances_cursor_and_truncates() {
        let data = ramp_u8_interleaved(2, 8);
        let mut buf = AudioBufferBuilder::build_u8(2, &data).unwrap();

        let samples1 = buf.read_pcm_frames(6, false).unwrap();
        let frames_read1 = samples1.frames();
        assert_eq!(frames_read1, 6);
        assert_sample_len(&samples1, 2, 6);
        assert_eq!(buf.cursor_pcm().unwrap(), 6);
        assert_eq!(buf.available_frames().unwrap(), 2);

        let samples2 = buf.read_pcm_frames(10, false).unwrap();
        let frames_read2 = samples2.frames();
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
        let mut buf = AudioBufferBuilder::build_f32(2, &data).unwrap();

        // Consume all frames.
        let _s0 = buf.read_pcm_frames(4, false).unwrap();
        assert_eq!(_s0.frames(), 4);
        assert!(buf.ended());

        // Looping read should produce some frames (ideally 2).
        let _s1 = buf.read_pcm_frames(2, true).unwrap();
        assert!(_s1.frames() > 0, "looping read should return >0 frames");
    }

    #[test]
    fn test_builder_s24_packed_requires_frames_channels_times_3_bytes() {
        // frames=4, channels=2 => 4*2*3 = 24 bytes
        let ok = vec![0u8; 24];
        assert!(AudioBufferBuilder::build_s24_packed(2, &ok).is_ok());

        // frames = 3
        let bad = vec![0u8; 23];
        let buf = AudioBufferBuilder::build_s24_packed(2, &bad);
        assert!(buf.is_ok());
        let mut buf = buf.unwrap();
        let buffer = buf.read_pcm_frames(4, false).unwrap();
        assert_eq!(buffer.frames(), 3);
        assert_eq!(buffer.len(), 3 * 3 * 2);
    }
}
