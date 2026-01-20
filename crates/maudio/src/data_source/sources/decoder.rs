use std::{marker::PhantomData, path::Path, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    Binding, MaResult,
    audio::{
        formats::{Format, SampleBuffer, SampleBufferS24},
        sample_rate::SampleRate,
    },
    data_source::DataFormat,
};

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

pub trait AsDecoderPtr {
    fn as_decoder_ptr(&self) -> *mut sys::ma_decoder;
    fn format(&self) -> Format;
    fn channels(&self) -> u32;
}

impl AsDecoderPtr for Decoder {
    fn as_decoder_ptr(&self) -> *mut sys::ma_decoder {
        self.inner
    }

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl AsDecoderPtr for DecoderRef<'_> {
    fn as_decoder_ptr(&self) -> *mut sys::ma_decoder {
        self.inner
    }

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl<T: AsDecoderPtr + ?Sized> DecoderOps for T {}

pub trait DecoderOps: AsDecoderPtr {
    fn read_pcm_frames_u8(&mut self, frame_count: u64) -> MaResult<(SampleBuffer<u8>, u64)> {
        decoder_ffi::ma_decoder_read_pcm_frames_u8(self, frame_count)
    }

    fn read_pcm_frames_s16(&mut self, frame_count: u64) -> MaResult<(SampleBuffer<u16>, u64)> {
        decoder_ffi::ma_decoder_read_pcm_frames_s16(self, frame_count)
    }

    fn read_pcm_frames_s24(&mut self, frame_count: u64) -> MaResult<(SampleBufferS24, u64)> {
        decoder_ffi::ma_decoder_read_pcm_frames_s24(self, frame_count)
    }

    fn read_pcm_frames_s32(&mut self, frame_count: u64) -> MaResult<(SampleBuffer<u32>, u64)> {
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
}

impl Decoder {
    fn init_from_memory(data: &[u8], config: &DecoderBuilder) -> MaResult<Self> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new_uninit();
        let data_arc: Arc<[u8]> = Arc::from(data);

        decoder_ffi::ma_decoder_init_memory(
            data_arc.as_ptr() as *const _,
            data.len(),
            config,
            mem.as_mut_ptr(),
        )?;

        let ptr: Box<sys::ma_decoder> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_decoder = Box::into_raw(ptr);

        Ok(Self {
            inner,
            format: config.format,
            channels: config.channels,
            sample_rate: config.sample_rate,
            data: Some(data_arc),
        })
    }

    fn init_from_file(path: &Path, config: &DecoderBuilder) -> MaResult<Self> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new_uninit();

        Decoder::init_from_file_internal(path, config, mem.as_mut_ptr())?;

        let ptr: Box<sys::ma_decoder> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_decoder = Box::into_raw(ptr);

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

            decoder_ffi::ma_decoder_init_file_w(path, config, decoder)?;
            Ok(())
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }
}

impl<'a> DecoderRef<'a> {
    fn from_memory(data: &'a [u8], config: &DecoderBuilder) -> MaResult<DecoderRef<'a>> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new_uninit();

        decoder_ffi::ma_decoder_init_memory(
            data.as_ptr() as *const _,
            data.len(),
            config,
            mem.as_mut_ptr(),
        )?;

        let ptr: Box<sys::ma_decoder> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_decoder = Box::into_raw(ptr);

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
    use crate::data_source::sources::decoder::{AsDecoderPtr, DecoderBuilder};
    use crate::{Binding, MaRawResult, MaResult, data_source::DataFormat};

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
        let res = unsafe { sys::ma_decoder_uninit(decoder.as_decoder_ptr()) };
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
        decoder: &mut Decoder,
        path: &[u16],
        config: &DecoderBuilder,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_decoder_init_file_w(
                path.as_ptr(),
                &config.to_raw() as *const _,
                decoder.to_raw(),
            )
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
    ) -> MaResult<(SampleBuffer<u16>, u64)> {
        let mut buffer = decoder.format().new_s16(decoder.channels(), frame_count)?;
        let frames_read =
            ma_decoder_read_pcm_frames_internal(decoder, frame_count, buffer.as_mut_ptr())?;
        buffer.truncate_to_frames(frames_read as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_decoder_read_pcm_frames_s32<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        frame_count: u64,
    ) -> MaResult<(SampleBuffer<u32>, u64)> {
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
                decoder.as_decoder_ptr(),
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
        let res =
            unsafe { sys::ma_decoder_seek_to_pcm_frame(decoder.as_decoder_ptr(), frame_index) };
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
                decoder.as_decoder_ptr(),
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
            sys::ma_decoder_get_cursor_in_pcm_frames(decoder.as_decoder_ptr(), &mut cursor)
        };
        MaRawResult::check(res)?;
        Ok(cursor)
    }

    pub fn ma_decoder_get_length_in_pcm_frames<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
    ) -> MaResult<u64> {
        let mut length = 0;
        let res = unsafe {
            sys::ma_decoder_get_length_in_pcm_frames(decoder.as_decoder_ptr(), &mut length)
        };
        MaRawResult::check(res)?;
        Ok(length)
    }

    pub fn ma_decoder_get_available_frames<D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
    ) -> MaResult<u64> {
        let mut frames = 0;
        let res =
            unsafe { sys::ma_decoder_get_available_frames(decoder.as_decoder_ptr(), &mut frames) };
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
    use std::{fs, time::{SystemTime, UNIX_EPOCH}};

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
        out.extend_from_slice(&16u32.to_le_bytes());        // PCM fmt chunk size
        out.extend_from_slice(&1u16.to_le_bytes());         // AudioFormat = 1 (PCM)
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
        let builder = DecoderBuilder::new(
            Format::F32,
            1,
            SampleRate::Sr48000,
        );

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
        assert_eq!(buf.len_samples(), 10 * 1);

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

        let builder = DecoderBuilder::new(
            Format::S16,
            1,
            SampleRate::Sr48000,
        );

        // Create a short-lived slice binding and ensure decoder works beyond it.
        let mut dec_ref = {
            let slice: &[u8] = &wav;
            DecoderRef::from_memory(slice, &builder).unwrap()
        };

        let (buf, read) = dec_ref.read_pcm_frames_s16(12).unwrap();
        assert_eq!(read, 12);
        assert_eq!(buf.len_samples(), 12 * 1);
    }

    #[test]
    fn test_decoder_from_file_reads_and_reports_length() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let path = unique_tmp_path("wav");
        fs::write(&path, &wav).unwrap();

        let builder = DecoderBuilder::new(
            Format::F32,
            1,
            SampleRate::Sr48000,
        );

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
        (Format::U8,  "u8"),
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
            Format::S24 => 5 * 1 * 3,
            _ => 5 * 1,
        };

        assert_eq!(len_units, expected_units);
    }
}
}