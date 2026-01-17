use maudio_sys::ffi as sys;

use crate::Binding;

pub struct AudioBuffer {
    inner: *mut sys::ma_audio_buffer,
}

impl Binding for AudioBuffer {
    type Raw = *mut sys::ma_audio_buffer;

    fn from_ptr(raw: Self::Raw) -> Self {
        AudioBuffer { inner: raw }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

pub(crate) mod buffer_ffi {
    use crate::{Binding, MaRawResult, MaResult, data_source::sources::buffer::AudioBuffer};
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_audio_buffer_init(
        config: *const sys::ma_audio_buffer_config,
        buffer: *mut sys::ma_audio_buffer,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_init(config, buffer) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_audio_buffer_init_copy(
        config: *const sys::ma_audio_buffer_config,
        buffer: &mut AudioBuffer,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_init_copy(config, buffer.to_raw()) };
        MaRawResult::check(res)
    }

    // fn ma_audio_buffer_alloc_and_init(
    //     config: *const sys::ma_audio_buffer_config,
    //     buffer: *mut sys::ma_audio_buffer,
    // ) -> MaResult<()> {
    //     let res = unsafe {sys::ma_audio_buffer_alloc_and_init(config, buffer)};
    //     MaRawResult::check(res)
    // }

    #[inline]
    pub fn ma_audio_buffer_uninit(buffer: &mut AudioBuffer) {
        unsafe {
            sys::ma_audio_buffer_uninit(buffer.to_raw());
        }
    }

    #[inline]
    pub fn ma_audio_buffer_read_pcm_frames() {
        // unsafe { sys::ma_audio_buffer_read_pcm_frames(pAudioBuffer, pFramesOut, frameCount, loop_)};
    }

    #[inline]
    pub fn ma_audio_buffer_seek_to_pcm_frame(
        buffer: &mut AudioBuffer,
        frame_index: u64,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_seek_to_pcm_frame(buffer.to_raw(), frame_index) };
        MaRawResult::check(res)
    }

    // TODO
    #[inline]
    pub fn ma_audio_buffer_map(
        buffer: &mut AudioBuffer,
        frames_out: *mut *mut core::ffi::c_void,
        frame_count: *mut u64,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_map(buffer.to_raw(), frames_out, frame_count) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_audio_buffer_unmap(buffer: &mut AudioBuffer, frame_count: u64) -> MaResult<()> {
        let res = unsafe { sys::ma_audio_buffer_unmap(buffer.to_raw(), frame_count) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_audio_buffer_at_end(buffer: &AudioBuffer) -> bool {
        let res = unsafe { sys::ma_audio_buffer_at_end(buffer.to_raw() as *const _) };
        res == 1
    }

    #[inline]
    pub fn ma_audio_buffer_get_cursor_in_pcm_frames(buffer: &AudioBuffer) -> MaResult<u64> {
        let mut cursor = 0;
        let res = unsafe {
            sys::ma_audio_buffer_get_cursor_in_pcm_frames(buffer.to_raw() as *const _, &mut cursor)
        };
        MaRawResult::check(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_audio_buffer_get_length_in_pcm_frames(buffer: &AudioBuffer) -> MaResult<u64> {
        let mut length = 0;
        let res = unsafe {
            sys::ma_audio_buffer_get_length_in_pcm_frames(buffer.to_raw() as *const _, &mut length)
        };
        MaRawResult::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_audio_buffer_get_available_frames(buffer: &AudioBuffer) -> MaResult<u64> {
        let mut frames = 0;
        let res = unsafe {
            sys::ma_audio_buffer_get_available_frames(buffer.to_raw() as *const _, &mut frames)
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
