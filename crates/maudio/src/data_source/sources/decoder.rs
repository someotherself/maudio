//! Streaming audio decoder.
//!
//! A `Decoder` reads encoded audio data and decodes it into PCM frames on demand.
//! It does not load the entire audio stream into memory.
//!
//! Use a decoder when working with large audio files or when streaming audio
//! data. For small, fully-loaded audio, consider using [`AudioBuffer`](crate::data_source::sources::buffer::AudioBuffer) instead.
//!
//! A decoder implements [`DataSource`](crate::data_source::DataSource), allowing it to be used directly by
//! sounds and node graphs.
use std::{marker::PhantomData, mem::MaybeUninit, path::Path, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    audio::{
        formats::{Format, SampleBuffer, SampleBufferS24},
        sample_rate::SampleRate,
    },
    data_source::{private_data_source, AsSourcePtr, DataFormat, DataSourceRef},
    Binding, MaResult,
};

/// Owned streaming audio decoder.
///
/// This decoder is self-contained: it keeps any required input data alive for
/// as long as the decoder exists.
///
/// Use this when you need an easy-to-store decoder without borrowing.
pub struct Decoder {
    inner: *mut sys::ma_decoder,
    format: Format,
    channels: u32,
    sample_rate: SampleRate,
    // Holds data to avoid lifetimes on this type
    data: Option<Arc<[u8]>>,
}

impl Binding for Decoder {
    type Raw = *mut sys::ma_decoder;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

/// Borrowed (zero-copy) streaming audio decoder.
///
/// `DecoderRef` does not own the underlying input data. It references existing
/// audio data, so the backing data must remain alive for as long as this decoder
/// is used (`'a`).
///
/// Use this when decoding from already-owned data without copying.
pub struct DecoderRef<'a> {
    inner: *mut sys::ma_decoder,
    format: Format,
    channels: u32,
    sample_rate: SampleRate,
    // We need to keep the data alive
    _marker_data: PhantomData<&'a [u8]>,
}

impl Binding for DecoderRef<'_> {
    type Raw = *mut sys::ma_decoder;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

// Keeps the as_decoder_ptr method on AsAudioBufferPtr private
mod private_decoder {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait DecoderPtrProvider<T: ?Sized> {
        fn as_decoder_ptr(t: &T) -> *mut sys::ma_decoder;
    }

    pub struct DecoderProvider;
    pub struct DecoderRefProvider;

    impl DecoderPtrProvider<Decoder> for DecoderProvider {
        fn as_decoder_ptr(t: &Decoder) -> *mut sys::ma_decoder {
            t.to_raw()
        }
    }

    impl<'a> DecoderPtrProvider<DecoderRef<'a>> for DecoderRefProvider {
        fn as_decoder_ptr(t: &DecoderRef<'a>) -> *mut sys::ma_decoder {
            t.to_raw()
        }
    }

    pub fn decoder_ptr<T: AsDecoderPtr + ?Sized>(t: &T) -> *mut sys::ma_decoder {
        <T as AsDecoderPtr>::__PtrProvider::as_decoder_ptr(t)
    }
}

// Allows Decoder to pass as a DataSource
#[doc(hidden)]
impl AsSourcePtr for Decoder {
    type __PtrProvider = private_data_source::DecoderProvider;
}

// Allows DecoderRef to pass as a DataSource
#[doc(hidden)]
impl<'a> AsSourcePtr for DecoderRef<'a> {
    type __PtrProvider = private_data_source::DecoderRefProvider;
}

/// Allows both Decoder and DecoderRef to access the same methods
pub trait AsDecoderPtr {
    #[doc(hidden)]
    type __PtrProvider: private_decoder::DecoderPtrProvider<Self>;
    fn format(&self) -> Format;
    fn channels(&self) -> u32;
}

impl AsDecoderPtr for Decoder {
    #[doc(hidden)]
    type __PtrProvider = private_decoder::DecoderProvider;

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl AsDecoderPtr for DecoderRef<'_> {
    #[doc(hidden)]
    type __PtrProvider = private_decoder::DecoderRefProvider;

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl<T: AsDecoderPtr + AsSourcePtr> DecoderOps for T {}

/// DecoderOps trait contains shared methods for [`Decoder`] and [`DecoderRef`]
pub trait DecoderOps: AsDecoderPtr + AsSourcePtr {
    fn read_pcm_frames_u8(&mut self, frame_count: u64) -> MaResult<(SampleBuffer<u8>, u64)> {
        decoder_ffi::ma_decoder_read_pcm_frames_u8(self, frame_count)
    }

    fn read_pcm_frames_s16(&mut self, frame_count: u64) -> MaResult<(SampleBuffer<i16>, u64)> {
        decoder_ffi::ma_decoder_read_pcm_frames_s16(self, frame_count)
    }

    fn read_pcm_frames_s24(&mut self, frame_count: u64) -> MaResult<(SampleBufferS24, u64)> {
        decoder_ffi::ma_decoder_read_pcm_frames_s24(self, frame_count)
    }

    fn read_pcm_frames_s32(&mut self, frame_count: u64) -> MaResult<(SampleBuffer<i32>, u64)> {
        decoder_ffi::ma_decoder_read_pcm_frames_s32(self, frame_count)
    }

    fn read_pcm_frames_f32(&mut self, frame_count: u64) -> MaResult<(SampleBuffer<f32>, u64)> {
        decoder_ffi::ma_decoder_read_pcm_frames_f32(self, frame_count)
    }

    fn seek_to_pcm_frame(&mut self, frame_index: u64) -> MaResult<()> {
        decoder_ffi::ma_decoder_seek_to_pcm_frame(self, frame_index)
    }

    fn data_format(&mut self) -> MaResult<DataFormat> {
        decoder_ffi::ma_decoder_get_data_format(self)
    }

    fn cursor_pcm(&mut self) -> MaResult<u64> {
        decoder_ffi::ma_decoder_get_cursor_in_pcm_frames(self)
    }

    fn length_pcm(&mut self) -> MaResult<u64> {
        decoder_ffi::ma_decoder_get_length_in_pcm_frames(self)
    }

    fn available_frames(&mut self) -> MaResult<u64> {
        decoder_ffi::ma_decoder_get_available_frames(self)
    }

    fn as_source(&self) -> DataSourceRef<'_> {
        debug_assert!(!private_decoder::decoder_ptr(self).is_null());
        let ptr = private_decoder::decoder_ptr(self).cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

impl Decoder {
    fn init_from_memory(data: &[u8], config: &DecoderBuilder) -> MaResult<Self> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());
        let data_arc: Arc<[u8]> = Arc::from(data);

        decoder_ffi::ma_decoder_init_memory(
            data_arc.as_ptr() as *const _,
            data.len(),
            config,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;

        Ok(Self {
            inner,
            format: config.format,
            channels: config.channels,
            sample_rate: config.sample_rate,
            data: Some(data_arc),
        })
    }

    fn init_from_file(path: &Path, config: &DecoderBuilder) -> MaResult<Self> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        Decoder::init_from_file_internal(path, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;

        Ok(Self {
            inner,
            format: config.format,
            channels: config.channels,
            sample_rate: config.sample_rate,
            data: None,
        })
    }

    fn init_from_file_internal(
        path: &Path,
        config: &DecoderBuilder,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            use crate::engine::cstring_from_path;

            let path = cstring_from_path(path)?;
            decoder_ffi::ma_decoder_init_file(path, config, decoder)?;
            Ok(())
        }

        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated;

            let path = wide_null_terminated(path);

            decoder_ffi::ma_decoder_init_file_w(&path, config, decoder)?;
            Ok(())
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }
}

impl<'a> DecoderRef<'a> {
    fn from_memory(data: &'a [u8], config: &DecoderBuilder) -> MaResult<DecoderRef<'a>> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        decoder_ffi::ma_decoder_init_memory(
            data.as_ptr() as *const _,
            data.len(),
            config,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;

        Ok(Self {
            inner,
            format: config.format,
            channels: config.channels,
            sample_rate: config.sample_rate,
            _marker_data: PhantomData,
        })
    }
}

pub(crate) mod decoder_ffi {
    use maudio_sys::ffi as sys;

    use crate::audio::formats::{SampleBuffer, SampleBufferS24};
    use crate::data_source::sources::decoder::{private_decoder, AsDecoderPtr, DecoderBuilder};
    use crate::{data_source::DataFormat, Binding, MaRawResult, MaResult};

    #[inline]
    pub fn ma_decoder_init(
        on_read: sys::ma_decoder_read_proc,
        on_seek: sys::ma_decoder_seek_proc,
        user_data: *mut core::ffi::c_void,
        config: &DecoderBuilder,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_decoder_init(
                on_read,
                on_seek,
                user_data,
                &config.to_raw() as *const _,
                decoder,
            )
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_decoder_uninit<D: AsDecoderPtr + ?Sized>(decoder: &mut D) -> MaResult<()> {
        let res = unsafe { sys::ma_decoder_uninit(private_decoder::decoder_ptr(decoder)) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_decoder_init_memory(
        data: *const core::ffi::c_void,
        data_size: usize,
        config: &DecoderBuilder,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_decoder_init_memory(data, data_size, &config.to_raw() as *const _, decoder)
        };
        MaRawResult::check(res)
    }

    #[inline]
    #[cfg(unix)]
    pub fn ma_decoder_init_file(
        path: std::ffi::CString,
        config: &DecoderBuilder,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_decoder_init_file(path.as_ptr(), &config.to_raw() as *const _, decoder)
        };
        MaRawResult::check(res)
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_decoder_init_file_w(
        path: &[u16],
        config: &DecoderBuilder,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_decoder_init_file_w(path.as_ptr(), &config.to_raw() as *const _, decoder)
        };
        MaRawResult::check(res)
    }

    // TODO: For later in the roadmap
    #[inline]
    fn ma_decoder_init_vfs() -> MaResult<()> {
        todo!()
    }

    // TODO: For later in the roadmap
    #[inline]
    fn ma_decoder_init_vfs_w() -> MaResult<()> {
        todo!()
    }

    pub fn ma_decoder_read_pcm_frames_u8<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        frame_count: u64,
    ) -> MaResult<(SampleBuffer<u8>, u64)> {
        let mut buffer = decoder.format().new_u8(decoder.channels(), frame_count)?;
        let frames_read =
            ma_decoder_read_pcm_frames_internal(decoder, frame_count, buffer.as_mut_ptr())?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_decoder_read_pcm_frames_s16<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        frame_count: u64,
    ) -> MaResult<(SampleBuffer<i16>, u64)> {
        let mut buffer = decoder.format().new_s16(decoder.channels(), frame_count)?;
        let frames_read =
            ma_decoder_read_pcm_frames_internal(decoder, frame_count, buffer.as_mut_ptr())?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_decoder_read_pcm_frames_s32<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        frame_count: u64,
    ) -> MaResult<(SampleBuffer<i32>, u64)> {
        let mut buffer = decoder.format().new_s32(decoder.channels(), frame_count)?;
        let frames_read =
            ma_decoder_read_pcm_frames_internal(decoder, frame_count, buffer.as_mut_ptr())?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_decoder_read_pcm_frames_f32<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        frame_count: u64,
    ) -> MaResult<(SampleBuffer<f32>, u64)> {
        let mut buffer = decoder.format().new_f32(decoder.channels(), frame_count)?;
        let frames_read =
            ma_decoder_read_pcm_frames_internal(decoder, frame_count, buffer.as_mut_ptr())?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_decoder_read_pcm_frames_s24<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        frame_count: u64,
    ) -> MaResult<(SampleBufferS24, u64)> {
        let mut buffer = decoder.format().new_s24(decoder.channels(), frame_count)?;
        let frames_read =
            ma_decoder_read_pcm_frames_internal(decoder, frame_count, buffer.as_mut_ptr())?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    #[inline]
    fn ma_decoder_read_pcm_frames_internal<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_decoder_read_pcm_frames(
                private_decoder::decoder_ptr(decoder),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaRawResult::check(res)?;
        Ok(frames_read)
    }

    #[inline]
    pub fn ma_decoder_seek_to_pcm_frame<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        frame_index: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_decoder_seek_to_pcm_frame(private_decoder::decoder_ptr(decoder), frame_index)
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_decoder_get_data_format<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
    ) -> MaResult<DataFormat> {
        let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        let mut channels: sys::ma_uint32 = 0;
        let mut sample_rate: sys::ma_uint32 = 0;

        let mut channel_map = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];

        let res = unsafe {
            sys::ma_decoder_get_data_format(
                private_decoder::decoder_ptr(decoder),
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

    pub fn ma_decoder_get_cursor_in_pcm_frames<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
    ) -> MaResult<u64> {
        let mut cursor = 0;
        let res = unsafe {
            sys::ma_decoder_get_cursor_in_pcm_frames(
                private_decoder::decoder_ptr(decoder),
                &mut cursor,
            )
        };
        MaRawResult::check(res)?;
        Ok(cursor)
    }

    pub fn ma_decoder_get_length_in_pcm_frames<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
    ) -> MaResult<u64> {
        let mut length = 0;
        let res = unsafe {
            sys::ma_decoder_get_length_in_pcm_frames(
                private_decoder::decoder_ptr(decoder),
                &mut length,
            )
        };
        MaRawResult::check(res)?;
        Ok(length)
    }

    pub fn ma_decoder_get_available_frames<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
    ) -> MaResult<u64> {
        let mut frames = 0;
        let res = unsafe {
            sys::ma_decoder_get_available_frames(private_decoder::decoder_ptr(decoder), &mut frames)
        };
        MaRawResult::check(res)?;
        Ok(frames)
    }

    // TODO: Not implemented. Look into usefulness
    fn ma_decode_from_vfs(
        vfs: *mut sys::ma_vfs,
        path: *const core::ffi::c_char,
        config: *mut sys::ma_decoder_config,
        frame_count_out: *mut u64,
        frames_out: *mut *mut core::ffi::c_void,
    ) {
        let _res =
            unsafe { sys::ma_decode_from_vfs(vfs, path, config, frame_count_out, frames_out) };
    }

    // TODO: Not implemented. Look into usefulness
    fn ma_decode_file(
        path: *const core::ffi::c_char,
        config: *mut sys::ma_decoder_config,
        frame_count_out: *mut u64,
        frames_out: *mut *mut core::ffi::c_void,
    ) {
        let _res = unsafe { sys::ma_decode_file(path, config, frame_count_out, frames_out) };
    }

    // TODO: Not implemented. Look into usefulness
    fn ma_decode_memory(
        data: *const core::ffi::c_void,
        data_size: usize,
        config: *mut sys::ma_decoder_config,
        frame_count_out: *mut u64,
        frames_out: *mut *mut core::ffi::c_void,
    ) {
        let _res =
            unsafe { sys::ma_decode_memory(data, data_size, config, frame_count_out, frames_out) };
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = decoder_ffi::ma_decoder_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

impl Drop for DecoderRef<'_> {
    fn drop(&mut self) {
        let _ = decoder_ffi::ma_decoder_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub struct DecoderBuilder {
    inner: sys::ma_decoder_config,
    format: Format,
    channels: u32,
    sample_rate: SampleRate,
}

impl Binding for DecoderBuilder {
    type Raw = sys::ma_decoder_config;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl DecoderBuilder {
    pub fn new(out_format: Format, out_channels: u32, out_sample_rate: SampleRate) -> Self {
        decoder_b_ffi::ma_decoder_config_init(out_format, out_channels, out_sample_rate)
    }
}

pub(crate) mod decoder_b_ffi {
    use crate::{
        audio::{formats::Format, sample_rate::SampleRate},
        data_source::sources::decoder::DecoderBuilder,
    };
    use maudio_sys::ffi as sys;

    pub fn ma_decoder_config_init(
        out_format: Format,
        out_channels: u32,
        out_sample_rate: SampleRate,
    ) -> DecoderBuilder {
        let ptr = unsafe {
            sys::ma_decoder_config_init(out_format.into(), out_channels, out_sample_rate.into())
        };
        DecoderBuilder {
            inner: ptr,
            format: out_format,
            channels: out_channels,
            sample_rate: out_sample_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    // --- Helpers -------------------------------------------------------------

    fn unique_tmp_path(ext: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("miniaudio_decoder_test_{nanos}.{ext}"));
        p
    }

    /// Build a minimal PCM 16-bit little-endian WAV file.
    fn wav_i16_le(channels: u16, sample_rate: u32, samples_interleaved: &[i16]) -> Vec<u8> {
        assert!(channels > 0);
        assert_eq!(samples_interleaved.len() % channels as usize, 0);

        let bits_per_sample: u16 = 16;
        let block_align: u16 = channels * (bits_per_sample / 8);
        let byte_rate: u32 = sample_rate * block_align as u32;
        let data_bytes_len: u32 = (samples_interleaved.len() * 2) as u32;

        // RIFF chunk size = 4 ("WAVE") + (8 + fmt) + (8 + data)
        let riff_chunk_size: u32 = 4 + (8 + 16) + (8 + data_bytes_len);

        let mut out = Vec::with_capacity((8 + riff_chunk_size) as usize);

        // RIFF header
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&riff_chunk_size.to_le_bytes());
        out.extend_from_slice(b"WAVE");

        // fmt chunk
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
        out.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = 1 (PCM)
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits_per_sample.to_le_bytes());

        // data chunk
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_bytes_len.to_le_bytes());

        // samples
        for s in samples_interleaved {
            out.extend_from_slice(&s.to_le_bytes());
        }

        out
    }

    fn tiny_test_wav_mono(frames: usize) -> Vec<u8> {
        let mut samples = Vec::with_capacity(frames);
        // deterministic ramp
        for i in 0..frames {
            samples.push(((i as i32 * 300) % i16::MAX as i32) as i16);
        }
        wav_i16_le(1, 48_000, &samples)
    }

    // --- Tests ---------------------------------------------------------------

    #[test]
    fn test_decoder_from_memory_f32_read_seek_cursor_length_available() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        // Adjust these if your constructors differ:
        let builder = DecoderBuilder::new(Format::F32, 1, SampleRate::Sr48000);

        let mut dec = Decoder::init_from_memory(&wav, &builder).unwrap();

        // length/cursor/available should be sane at start
        let len = dec.length_pcm().unwrap();
        assert_eq!(len as usize, frames_total);

        let cursor0 = dec.cursor_pcm().unwrap();
        assert_eq!(cursor0, 0);

        let avail0 = dec.available_frames().unwrap();
        assert_eq!(avail0 as usize, frames_total);

        // data format should match output config
        let df = dec.data_format().unwrap();
        assert_eq!(df.channels, 1);
        assert_eq!(df.sample_rate, 48_000);
        assert_eq!(df.format, Format::F32);

        // read a few frames
        let (buf, read) = dec.read_pcm_frames_f32(10).unwrap();
        assert_eq!(read, 10);
        assert_eq!(buf.len_samples(), 10);

        let cursor1 = dec.cursor_pcm().unwrap();
        assert_eq!(cursor1, 10);

        let avail1 = dec.available_frames().unwrap();
        assert_eq!(avail1 as usize, frames_total - 10);

        // seek and read again
        dec.seek_to_pcm_frame(0).unwrap();
        assert_eq!(dec.cursor_pcm().unwrap(), 0);

        let (_buf2, read2) = dec.read_pcm_frames_f32(7).unwrap();
        assert_eq!(read2, 7);
        assert_eq!(dec.cursor_pcm().unwrap(), 7);
    }

    #[test]
    fn test_decoder_ref_from_memory_keeps_data_alive_and_decodes() {
        let frames_total: usize = 32;
        let wav = tiny_test_wav_mono(frames_total);

        let builder = DecoderBuilder::new(Format::S16, 1, SampleRate::Sr48000);

        // Create a short-lived slice binding and ensure decoder works beyond it.
        let mut dec_ref = {
            let slice: &[u8] = &wav;
            DecoderRef::from_memory(slice, &builder).unwrap()
        };

        let (buf, read) = dec_ref.read_pcm_frames_s16(12).unwrap();
        assert_eq!(read, 12);
        assert_eq!(buf.len_samples(), 12);
    }

    #[test]
    fn test_decoder_from_file_reads_and_reports_length() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let path = unique_tmp_path("wav");
        fs::write(&path, &wav).unwrap();

        let builder = DecoderBuilder::new(Format::F32, 1, SampleRate::Sr48000);

        let mut dec = Decoder::init_from_file(&path, &builder).unwrap();

        assert_eq!(dec.length_pcm().unwrap() as usize, frames_total);
        assert_eq!(dec.cursor_pcm().unwrap(), 0);

        let (_buf, read) = dec.read_pcm_frames_f32(1000).unwrap();
        assert_eq!(read as usize, frames_total);

        // cleanup
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn test_decoder_read_variants_return_expected_lengths() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);

        let cases = [
            (Format::U8, "u8"),
            (Format::S16, "s16"),
            (Format::S24, "s24"),
            (Format::S32, "s32"),
            (Format::F32, "f32"),
        ];

        for (fmt, _name) in cases {
            let builder = DecoderBuilder::new(fmt, 1, SampleRate::Sr48000);
            let mut dec = Decoder::init_from_memory(&wav, &builder).unwrap();

            let (len_units, read) = match fmt {
                Format::U8 => {
                    let (b, r) = dec.read_pcm_frames_u8(5).unwrap();
                    (b.len_samples(), r)
                }
                Format::S16 => {
                    let (b, r) = dec.read_pcm_frames_s16(5).unwrap();
                    (b.len_samples(), r)
                }
                Format::S24 => {
                    let (b, r) = dec.read_pcm_frames_s24(5).unwrap();
                    (b.len_samples(), r)
                }
                Format::S32 => {
                    let (b, r) = dec.read_pcm_frames_s32(5).unwrap();
                    (b.len_samples(), r)
                }
                Format::F32 => {
                    let (b, r) = dec.read_pcm_frames_f32(5).unwrap();
                    (b.len_samples(), r)
                }
            };

            assert_eq!(read, 5);

            // Key: expected "units" depends on how the buffer stores samples.
            let expected_units = match fmt {
                Format::S24 => 15,
                _ => 5,
            };

            assert_eq!(len_units, expected_units);
        }
    }
}
