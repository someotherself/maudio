use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    Binding, ErrorKinds, MaResult, MaudioError, audio::formats::{Format, Sample, Samples}, engine::AllocationCallbacks
};

// TODO: Move alloc_cb to an Arc inside struct and remove lifetime
pub struct AudioBuffer {
    inner: *mut sys::ma_audio_buffer,
    format: Format,
    channels: u32,
    // It may be helpful to avoid lifetimes on this type
    _alloc_keepalive: Option<std::sync::Arc<AllocationCallbacks>>,
}

impl Binding for AudioBuffer {
    type Raw = *mut sys::ma_audio_buffer;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

pub struct AudioBufferRef<'a> {
    inner: *mut sys::ma_audio_buffer,
    format: Format,
    channels: u32,
    _marker: PhantomData<&'a [u8]>,
    _alloc_marker: PhantomData<&'a AllocationCallbacks>,
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

pub trait AsAudioBufferPtr {
    fn as_buffer_ptr(&self) -> *mut sys::ma_audio_buffer;
    fn format(&self) -> Format;
    fn channels(&self) -> u32;
}

impl AsAudioBufferPtr for AudioBuffer {
    fn as_buffer_ptr(&self) -> *mut sys::ma_audio_buffer {
        self.to_raw()
    }

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl AsAudioBufferPtr for AudioBufferRef<'_> {
    fn as_buffer_ptr(&self) -> *mut sys::ma_audio_buffer {
        self.to_raw()
    }

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl<T: AsAudioBufferPtr + ?Sized> AudioBufferOps for T {}

pub trait AudioBufferOps: AsAudioBufferPtr {
    // TODO: Add read_pcm_frames for specific formats?

    fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Samples, u64)> {
        buffer_ffi::ma_audio_buffer_read_pcm_frames(self, frame_count, false)
    }

    fn read_pcm_frames_loop(&mut self, frame_count: u64) -> MaResult<(Samples, u64)> {
        buffer_ffi::ma_audio_buffer_read_pcm_frames(self, frame_count, true)
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
}

impl<'a> AudioBufferRef<'a> {
    fn new_with_cfg_internal(config: &AudioBufferBuilder<'a>) -> MaResult<Self> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_audio_buffer>> = Box::new_uninit();
        buffer_ffi::ma_audio_buffer_init(config, mem.as_mut_ptr())?;

        let ptr: Box<sys::ma_audio_buffer> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_audio_buffer = Box::into_raw(ptr);

        Ok(Self {
            inner,
            format: config.inner.format.try_into()?,
            channels: config.inner.channels,
            _marker: PhantomData,
            _alloc_marker: PhantomData,
        })
    }
}

impl AudioBuffer {
    fn new_with_cfg_internal(config: &AudioBufferBuilder) -> MaResult<Self> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_audio_buffer>> = Box::new_uninit();
        buffer_ffi::ma_audio_buffer_init_copy(config, mem.as_mut_ptr())?;

        let ptr: Box<sys::ma_audio_buffer> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_audio_buffer = Box::into_raw(ptr);

        Ok(Self {
            inner,
            format: config.inner.format.try_into()?,
            channels: config.inner.channels,
            _alloc_keepalive: None
        })
    }
}

pub(crate) mod buffer_ffi {
    use crate::{
        Binding, MaRawResult, MaResult,
        audio::formats::Samples,
        data_source::sources::buffer::{AsAudioBufferPtr, AudioBuffer, AudioBufferBuilder},
    };
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_audio_buffer_init(
        config: &AudioBufferBuilder,
        buffer: *mut sys::ma_audio_buffer,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_init(&config.to_raw() as *const _, buffer) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_audio_buffer_init_copy(
        config: &AudioBufferBuilder,
        buffer: *mut sys::ma_audio_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_audio_buffer_init_copy(&config.to_raw() as *const _, buffer)
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_audio_buffer_uninit<A: AsAudioBufferPtr + ?Sized>(buffer: &mut A) {
        unsafe {
            sys::ma_audio_buffer_uninit(buffer.as_buffer_ptr());
        }
    }

    #[inline]
    pub fn ma_audio_buffer_read_pcm_frames<A: AsAudioBufferPtr + ?Sized>(
        audio_buffer: &mut A,
        frame_count: u64,
        looping: bool,
    ) -> MaResult<(Samples, u64)> {
        let channels = audio_buffer.channels();
        let format = audio_buffer.format();
        let mut buffer = format.with_len(channels, frame_count)?;

        let looping = looping as u32;
        let frames_read = unsafe {
            sys::ma_audio_buffer_read_pcm_frames(
                audio_buffer.as_buffer_ptr(),
                buffer.as_mut_ptr(),
                frame_count,
                looping,
            )
        };
        buffer.truncate_to_frames(frames_read, channels);
        Ok((buffer, frames_read))
    }

    #[inline]
    pub fn ma_audio_buffer_seek_to_pcm_frame<A: AsAudioBufferPtr + ?Sized>(
        buffer: &mut A,
        frame_index: u64,
    ) -> MaResult<()> {
        let res =
            unsafe { sys::ma_audio_buffer_seek_to_pcm_frame(buffer.as_buffer_ptr(), frame_index) };
        MaRawResult::check(res)
    }

    // TODO Keep private for now
    #[inline]
    pub fn ma_audio_buffer_map(
        buffer: &mut AudioBuffer,
        frames_out: *mut *mut core::ffi::c_void,
        frame_count: *mut u64,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_map(buffer.to_raw(), frames_out, frame_count) };
        MaRawResult::check(res)
    }

    // TODO Keep private for now
    #[inline]
    pub fn ma_audio_buffer_unmap(buffer: &mut AudioBuffer, frame_count: u64) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_unmap(buffer.to_raw(), frame_count) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_audio_buffer_at_end<A: AsAudioBufferPtr + ?Sized>(buffer: &A) -> bool {
        let res = unsafe { sys::ma_audio_buffer_at_end(buffer.as_buffer_ptr() as *const _) };
        res == 1
    }

    #[inline]
    pub fn ma_audio_buffer_get_cursor_in_pcm_frames<A: AsAudioBufferPtr + ?Sized>(
        buffer: &A,
    ) -> MaResult<u64> {
        let mut cursor = 0;
        let res = unsafe {
            sys::ma_audio_buffer_get_cursor_in_pcm_frames(
                buffer.as_buffer_ptr() as *const _,
                &mut cursor,
            )
        };
        MaRawResult::check(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_audio_buffer_get_length_in_pcm_frames<A: AsAudioBufferPtr + ?Sized>(
        buffer: &A,
    ) -> MaResult<u64> {
        let mut length = 0;
        let res = unsafe {
            sys::ma_audio_buffer_get_length_in_pcm_frames(
                buffer.as_buffer_ptr() as *const _,
                &mut length,
            )
        };
        MaRawResult::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_audio_buffer_get_available_frames<A: AsAudioBufferPtr + ?Sized>(
        buffer: &A,
    ) -> MaResult<u64> {
        let mut frames = 0;
        let res = unsafe {
            sys::ma_audio_buffer_get_available_frames(
                buffer.as_buffer_ptr() as *const _,
                &mut frames,
            )
        };
        MaRawResult::check(res)?;
        Ok(frames)
    }
}

impl Drop for AudioBuffer {
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
    _marker: PhantomData<&'a [u8]>
}

impl<'a> Binding for AudioBufferBuilder<'a> {
    type Raw = sys::ma_audio_buffer_config;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            alloc_cb: None,
            _marker: PhantomData
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a> AudioBufferBuilder<'a> {
    pub fn from<T: Sample>(channels: u32, frames: u64, data: &'a [T]) -> MaResult<Self> {
        let expected = (frames as usize).saturating_mul(channels as usize);

        if data.len() != expected {
            return Err(MaudioError::new_ma_error(ErrorKinds::BufferSizeError));
        }

        let buffer =
            AudioBufferBuilder::init(T::FORMAT, channels, frames, data.as_ptr() as *const _, None);
        Ok(buffer)
    }

    pub fn from_s24_packed(channels: u32, frames: u64, data: &'a [u8]) -> MaResult<Self> {
        let expected_bytes = (frames as usize)
            .saturating_mul(channels as usize)
            .saturating_mul(3);
        if data.len() != expected_bytes {
            return Err(MaudioError::new_ma_error(ErrorKinds::BufferSizeError));
        }

        let buffer = AudioBufferBuilder::init(
            Format::S24,
            channels,
            frames,
            data.as_ptr() as *const _,
            None,
        );
        Ok(buffer)
    }

    pub fn build_copy(self) -> MaResult<AudioBuffer> {
        AudioBuffer::new_with_cfg_internal(&self)
    }

    pub fn build_ref(self) -> MaResult<AudioBufferRef<'a>> {
        AudioBufferRef::new_with_cfg_internal(&self)
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
    use crate::{audio::formats::Samples, data_source::sources::buffer::{AudioBufferBuilder, AudioBufferOps}};

    #[test]
    fn test_audio_buffer_basic_init() {
        let mut data = Vec::new();
        data.resize_with(2 * 100, || 0.0f32);
        let _buffer = AudioBufferBuilder::from(2, 100, &data)
            .unwrap()
            .build_copy()
            .unwrap();
    }

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

    #[test]
    fn test_audio_buffer_copy_basic_init_invariants() {
        let data = vec![0.0f32; 2 * 100];
        let buf = AudioBufferBuilder::from(2, 100, &data).unwrap().build_copy().unwrap();

        assert_eq!(buf.cursor_pcm().unwrap(), 0);
        assert_eq!(buf.length_pcm().unwrap(), 100);
        assert_eq!(buf.available_frames().unwrap(), 100);
        assert_eq!(buf.ended(), false);
    }

    #[test]
    fn test_audio_buffer_ref_basic_init_invariants() {
        let data = vec![0.0f32; 2 * 100];
        let buf = AudioBufferBuilder::from(2, 100, &data).unwrap().build_ref().unwrap();

        assert_eq!(buf.cursor_pcm().unwrap(), 0);
        assert_eq!(buf.length_pcm().unwrap(), 100);
        assert_eq!(buf.available_frames().unwrap(), 100);
        assert_eq!(buf.ended(), false);

        // buf is dropped before data due to scope order; should compile.
        drop(buf);
        drop(data);
    }

    #[test]
    fn test_audio_buffer_read_advances_cursor_and_available() {
        let data = vec![0.0f32; 2 * 100];
        let mut buf = AudioBufferBuilder::from(2, 100, &data).unwrap().build_copy().unwrap();

        let (_samples, frames_read) = buf.read_pcm_frames(10).unwrap();
        assert_eq!(frames_read, 10);

        assert_eq!(buf.cursor_pcm().unwrap(), 10);
        assert_eq!(buf.available_frames().unwrap(), 90);
        assert!(!buf.ended());
    }

    #[test]
    fn test_audio_buffer_read_to_end_sets_ended() {
        let data = vec![0.0f32; 2 * 100];
        let mut buf = AudioBufferBuilder::from(2, 100, &data).unwrap().build_copy().unwrap();

        let (_samples, frames_read) = buf.read_pcm_frames(100).unwrap();
        assert_eq!(frames_read, 100);
        assert!(buf.ended());
        assert_eq!(buf.available_frames().unwrap(), 0);

        // Reading past the end should return 0 frames.
        let (_samples2, frames_read2) = buf.read_pcm_frames(10).unwrap();
        assert_eq!(frames_read2, 0);
        assert!(buf.ended());
    }

    #[test]
    fn test_audio_buffer_seek_changes_cursor_and_ended() {
        let data = vec![0.0f32; 2 * 100];
        let mut buf = AudioBufferBuilder::from(2, 100, &data).unwrap().build_copy().unwrap();

        buf.seek_to_pcm(50).unwrap();
        assert_eq!(buf.cursor_pcm().unwrap(), 50);
        assert_eq!(buf.available_frames().unwrap(), 50);
        assert!(!buf.ended());

        buf.seek_to_pcm(100).unwrap();
        assert_eq!(buf.cursor_pcm().unwrap(), 100);
        assert_eq!(buf.available_frames().unwrap(), 0);
        assert!(buf.ended());
    }

    #[test]
    fn test_audio_buffer_read_returns_expected_interleaved_samples_f32() {
        let data = ramp_f32_interleaved(2, 8);
        let mut buf = AudioBufferBuilder::from(2, 8, &data).unwrap().build_copy().unwrap();

        let (samples, frames_read) = buf.read_pcm_frames(3).unwrap();
        assert_eq!(frames_read, 3);

        match samples {
            Samples::F32(v) => {
                // Expect first 3 frames interleaved:
                // frame0: [0*10+0, 0*10+1] = [0,1]
                // frame1: [10,11]
                // frame2: [20,21]
                assert_eq!(v, vec![0.0, 1.0, 10.0, 11.0, 20.0, 21.0]);
            }
            _ => panic!("expected Samples::F32"),
        }
    }

    #[test]
    fn test_audio_buffer_looping_wraps_and_does_not_end() {
        // 2ch, 4 frames. Channel 0 ramps 0..3, channel 1 all zeros.
        let mut data = vec![0.0f32; 2 * 4];
        for i in 0..4usize {
            data[i * 2 + 0] = i as f32;
            data[i * 2 + 1] = 0.0;
        }

        let mut buf = AudioBufferBuilder::from(2, 4, &data).unwrap().build_copy().unwrap();

        // Ask for more frames than exist, with looping.
        let (samples, frames_read) = buf.read_pcm_frames_loop(6).unwrap();
        assert_eq!(frames_read, 6);
        assert!(!buf.ended());

        match samples {
            Samples::F32(v) => {
                // Extract channel 0.
                let ch0: Vec<f32> = v.chunks_exact(2).map(|fr| fr[0]).collect();
                assert_eq!(ch0, vec![0.0, 1.0, 2.0, 3.0, 0.0, 1.0]);
            }
            _ => panic!("expected Samples::F32"),
        }
    }

    #[test]
    fn test_audio_buffer_read_then_seek_then_read() {
        let data = ramp_f32_interleaved(2, 10);
        let mut buf = AudioBufferBuilder::from(2, 10, &data).unwrap().build_copy().unwrap();

        let (_s, r) = buf.read_pcm_frames(4).unwrap();
        assert_eq!(r, 4);
        assert_eq!(buf.cursor_pcm().unwrap(), 4);

        buf.seek_to_pcm(2).unwrap();
        assert_eq!(buf.cursor_pcm().unwrap(), 2);

        let (samples, r2) = buf.read_pcm_frames(2).unwrap();
        assert_eq!(r2, 2);
        match samples {
            Samples::F32(v) => {
                // frames 2 and 3:
                assert_eq!(v, vec![20.0, 21.0, 30.0, 31.0]);
            }
            _ => panic!("expected Samples::F32"),
        }
    }
}
