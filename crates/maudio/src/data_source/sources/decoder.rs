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
        formats::{Format, SampleBuffer},
        sample_rate::SampleRate,
    },
    data_source::{private_data_source, AsSourcePtr, DataFormat, DataSourceRef},
    util::pcm_frames::{PcmFormat, S24Packed, S24},
    Binding, MaResult,
};

pub struct Decoder<F: PcmFormat, S> {
    inner: *mut sys::ma_decoder,
    channels: u32,
    sample_rate: SampleRate,
    format: Format,
    _sample_format: PhantomData<F>,
    source_data: S,
}

pub struct Borrowed<'a>(&'a [u8]);
pub struct Owned(Arc<[u8]>);
pub struct External; // Used when source is a Path

impl<F: PcmFormat, S> Binding for Decoder<F, S> {
    type Raw = *mut sys::ma_decoder;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat, S> Decoder<F, S> {
    #[inline]
    fn new(
        inner: *mut sys::ma_decoder,
        config: &DecoderBuilder,
        format: Format,
        source_data: S,
    ) -> Self {
        Self {
            inner,
            channels: config.channels,
            sample_rate: config.sample_rate,
            format,
            _sample_format: PhantomData,
            source_data,
        }
    }

    fn init_ref_internal(data: &[u8], config: &DecoderBuilder) -> MaResult<*mut sys::ma_decoder> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        decoder_ffi::ma_decoder_init_memory(
            data.as_ptr() as *const _,
            data.len(),
            config,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(inner)
    }

    fn init_copy_internal(
        data: Arc<[u8]>,
        config: &DecoderBuilder,
    ) -> MaResult<*mut sys::ma_decoder> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        decoder_ffi::ma_decoder_init_memory(
            data.as_ptr() as *const _,
            data.len(),
            config,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(inner)
    }

    fn init_file_internal(path: &Path, config: &DecoderBuilder) -> MaResult<*mut sys::ma_decoder> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        Decoder::<F, S>::init_from_file_internal(path, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(inner)
    }

    fn init_u8_file(path: &Path, config: &DecoderBuilder) -> MaResult<Decoder<u8, External>> {
        let inner = Decoder::<u8, External>::init_file_internal(path, config)?;
        Ok(Decoder::new(inner, config, Format::U8, External))
    }

    fn init_i16_file(path: &Path, config: &DecoderBuilder) -> MaResult<Decoder<i16, External>> {
        let inner = Decoder::<i16, External>::init_file_internal(path, config)?;
        Ok(Decoder::new(inner, config, Format::S16, External))
    }

    fn init_i32_file(path: &Path, config: &DecoderBuilder) -> MaResult<Decoder<i32, External>> {
        let inner = Decoder::<i32, External>::init_file_internal(path, config)?;
        Ok(Decoder::new(inner, config, Format::S32, External))
    }

    fn init_s24_file(
        path: &Path,
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<S24Packed, External>> {
        let inner = Decoder::<S24Packed, External>::init_file_internal(path, config)?;
        Ok(Decoder::new(inner, config, Format::S24, External))
    }

    fn init_f32_file(path: &Path, config: &DecoderBuilder) -> MaResult<Decoder<f32, External>> {
        let inner = Decoder::<f32, External>::init_file_internal(path, config)?;
        Ok(Decoder::new(inner, config, Format::F32, External))
    }

    fn init_u8_ref_from_memory<'a>(
        data: &'a [u8],
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<u8, Borrowed<'a>>> {
        let inner = Self::init_ref_internal(data, config)?;
        Ok(Decoder::new(inner, config, Format::U8, Borrowed(data)))
    }

    fn init_i16_ref_from_memory<'a>(
        data: &'a [u8],
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<i16, Borrowed<'a>>> {
        let inner = Self::init_ref_internal(data, config)?;
        Ok(Decoder::new(inner, config, Format::S16, Borrowed(data)))
    }

    fn init_s24_ref_from_memory<'a>(
        data: &'a [u8],
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<S24, Borrowed<'a>>> {
        let inner = Self::init_ref_internal(data, config)?;
        Ok(Decoder::new(inner, config, Format::S24, Borrowed(data)))
    }

    fn init_s24_packed_ref_from_memory<'a>(
        data: &'a [u8],
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<S24Packed, Borrowed<'a>>> {
        let inner = Self::init_ref_internal(data, config)?;
        Ok(Decoder::new(inner, config, Format::S24, Borrowed(data)))
    }

    fn init_i32_ref_from_memory<'a>(
        data: &'a [u8],
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<i32, Borrowed<'a>>> {
        let inner = Self::init_ref_internal(data, config)?;
        Ok(Decoder::new(inner, config, Format::S32, Borrowed(data)))
    }

    fn init_f32_ref_from_memory<'a>(
        data: &'a [u8],
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<f32, Borrowed<'a>>> {
        let inner = Self::init_ref_internal(data, config)?;
        Ok(Decoder::new(inner, config, Format::F32, Borrowed(data)))
    }

    fn init_u8_copy_from_memory<D: Into<Arc<[u8]>>>(
        data: D,
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<u8, Owned>> {
        let data_arc = data.into();
        let inner = Self::init_copy_internal(data_arc.clone(), config)?;
        Ok(Decoder::new(inner, config, Format::U8, Owned(data_arc)))
    }

    fn init_i16_copy_from_memory<D: Into<Arc<[u8]>>>(
        data: D,
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<i16, Owned>> {
        let data_arc = data.into();
        let inner = Self::init_copy_internal(data_arc.clone(), config)?;
        Ok(Decoder::new(inner, config, Format::S16, Owned(data_arc)))
    }

    fn init_s24_copy_from_memory<D: Into<Arc<[u8]>>>(
        data: D,
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<S24Packed, Owned>> {
        let data_arc = data.into();
        let inner = Self::init_copy_internal(data_arc.clone(), config)?;
        Ok(Decoder::new(inner, config, Format::S24, Owned(data_arc)))
    }

    fn init_i32_copy_from_memory<D: Into<Arc<[u8]>>>(
        data: D,
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<i32, Owned>> {
        let data_arc = data.into();
        let inner = Self::init_copy_internal(data_arc.clone(), config)?;
        Ok(Decoder::new(inner, config, Format::S32, Owned(data_arc)))
    }

    fn init_f32_copy_from_memory<D: Into<Arc<[u8]>>>(
        data: D,
        config: &DecoderBuilder,
    ) -> MaResult<Decoder<f32, Owned>> {
        let data_arc = data.into();
        let inner = Self::init_copy_internal(data_arc.clone(), config)?;
        Ok(Decoder::new(inner, config, Format::F32, Owned(data_arc)))
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

// Keeps the as_decoder_ptr method on AsAudioBufferPtr private
mod private_decoder {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait DecoderPtrProvider<T: ?Sized> {
        fn as_decoder_ptr(t: &T) -> *mut sys::ma_decoder;
    }

    pub struct DecoderProvider;
    pub struct DecoderRefProvider;

    impl<F: PcmFormat, S> DecoderPtrProvider<Decoder<F, S>> for DecoderProvider {
        fn as_decoder_ptr(t: &Decoder<F, S>) -> *mut sys::ma_decoder {
            t.to_raw()
        }
    }

    pub fn decoder_ptr<T: AsDecoderPtr + ?Sized>(t: &T) -> *mut sys::ma_decoder {
        <T as AsDecoderPtr>::__PtrProvider::as_decoder_ptr(t)
    }
}

// Allows Decoder to pass as a DataSource
#[doc(hidden)]
impl<F: PcmFormat, S> AsSourcePtr for Decoder<F, S> {
    type __PtrProvider = private_data_source::DecoderProvider;
}

pub trait AsDecoderPtr {
    #[doc(hidden)]
    type __PtrProvider: private_decoder::DecoderPtrProvider<Self>;
    fn format(&self) -> Format;
    fn channels(&self) -> u32;
}

impl<F: PcmFormat, S> AsDecoderPtr for Decoder<F, S> {
    #[doc(hidden)]
    type __PtrProvider = private_decoder::DecoderProvider;

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

impl<T: AsDecoderPtr + AsSourcePtr> DecoderOps for T {}

impl<F: PcmFormat, S> Decoder<F, S> {
    pub fn read_pcm_frames(
        &mut self,
        frame_count: u64,
    ) -> MaResult<(SampleBuffer<<F as PcmFormat>::PcmUnit>, u64)> {
        decoder_ffi::ma_decoder_read_pcm_frames::<F, Decoder<F, S>>(self, frame_count)
    }
}

pub trait DecoderOps: AsDecoderPtr + AsSourcePtr {
    fn read_pcm_frames<F: PcmFormat>(
        &mut self,
        frame_count: u64,
    ) -> MaResult<(SampleBuffer<<F as PcmFormat>::PcmUnit>, u64)> {
        decoder_ffi::ma_decoder_read_pcm_frames::<F, Self>(self, frame_count)
    }

    fn seek_to_pcm_frame(&mut self, frame_index: u64) -> MaResult<()> {
        decoder_ffi::ma_decoder_seek_to_pcm_frame(self, frame_index)
    }

    fn data_format(&self) -> MaResult<DataFormat> {
        decoder_ffi::ma_decoder_get_data_format(self)
    }

    fn cursor_pcm(&self) -> MaResult<u64> {
        decoder_ffi::ma_decoder_get_cursor_in_pcm_frames(self)
    }

    fn length_pcm(&self) -> MaResult<u64> {
        decoder_ffi::ma_decoder_get_length_in_pcm_frames(self)
    }

    fn available_frames(&self) -> MaResult<u64> {
        decoder_ffi::ma_decoder_get_available_frames(self)
    }

    fn as_source(&self) -> DataSourceRef<'_> {
        debug_assert!(!private_decoder::decoder_ptr(self).is_null());
        let ptr = private_decoder::decoder_ptr(self).cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

pub(crate) mod decoder_ffi {
    use maudio_sys::ffi as sys;

    use crate::audio::{channels::Channel, formats::SampleBuffer};
    use crate::data_source::{
        sources::decoder::{private_decoder, AsDecoderPtr, DecoderBuilder},
        DataFormat,
    };
    use crate::util::pcm_frames::{PcmFormat, PcmFormatInternal};
    use crate::{Binding, MaResult, MaudioError};

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
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_decoder_uninit<D: AsDecoderPtr + ?Sized>(decoder: &mut D) -> MaResult<()> {
        let res = unsafe { sys::ma_decoder_uninit(private_decoder::decoder_ptr(decoder)) };
        MaudioError::check(res)
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
        MaudioError::check(res)
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
        MaudioError::check(res)
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
        MaudioError::check(res)
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

    pub fn ma_decoder_read_pcm_frames<F: PcmFormat, D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        frame_count: u64,
    ) -> MaResult<(SampleBuffer<<F as PcmFormat>::PcmUnit>, u64)> {
        let mut buffer =
            <F as PcmFormatInternal>::new_zeroed_internal(frame_count, decoder.channels())?;
        let frames_read =
            ma_decoder_read_pcm_frames_internal(decoder, frame_count, buffer.as_mut_ptr())?;
        F::truncate_to_frames_read_internal(&mut buffer, frames_read)?;
        let buffer = F::storage_to_pcm_internal(buffer)?;
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
        MaudioError::check(res)?;
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
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_decoder_get_data_format<D: AsDecoderPtr + ?Sized>(
        decoder: &D,
    ) -> MaResult<DataFormat> {
        let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        let mut channels: sys::ma_uint32 = 0;
        let mut sample_rate: sys::ma_uint32 = 0;

        let mut channel_map_raw = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];

        let res = unsafe {
            sys::ma_decoder_get_data_format(
                private_decoder::decoder_ptr(decoder),
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

    pub fn ma_decoder_get_cursor_in_pcm_frames<D: AsDecoderPtr + ?Sized>(
        decoder: &D,
    ) -> MaResult<u64> {
        let mut cursor = 0;
        let res = unsafe {
            sys::ma_decoder_get_cursor_in_pcm_frames(
                private_decoder::decoder_ptr(decoder),
                &mut cursor,
            )
        };
        MaudioError::check(res)?;
        Ok(cursor)
    }

    pub fn ma_decoder_get_length_in_pcm_frames<D: AsDecoderPtr + ?Sized>(
        decoder: &D,
    ) -> MaResult<u64> {
        let mut length = 0;
        let res = unsafe {
            sys::ma_decoder_get_length_in_pcm_frames(
                private_decoder::decoder_ptr(decoder),
                &mut length,
            )
        };
        MaudioError::check(res)?;
        Ok(length)
    }

    pub fn ma_decoder_get_available_frames<D: AsDecoderPtr + ?Sized>(decoder: &D) -> MaResult<u64> {
        let mut frames = 0;
        let res = unsafe {
            sys::ma_decoder_get_available_frames(private_decoder::decoder_ptr(decoder), &mut frames)
        };
        MaudioError::check(res)?;
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

impl<F: PcmFormat, S> Drop for Decoder<F, S> {
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
    pub fn new(out_channels: u32, out_sample_rate: SampleRate) -> Self {
        decoder_b_ffi::ma_decoder_config_init(out_channels, out_sample_rate)
    }

    pub fn u8_memory<'a>(&mut self, data: &'a [u8]) -> MaResult<Decoder<u8, Borrowed<'a>>> {
        self.inner.format = Format::U8.into();
        Decoder::<u8, Borrowed<'a>>::init_u8_ref_from_memory(data, self)
    }

    pub fn i16_memory<'a>(&mut self, data: &'a [u8]) -> MaResult<Decoder<i16, Borrowed<'a>>> {
        self.inner.format = Format::S16.into();
        Decoder::<i16, Borrowed<'a>>::init_i16_ref_from_memory(data, self)
    }

    pub fn s24_packed_memory<'a>(
        &mut self,
        data: &'a [u8],
    ) -> MaResult<Decoder<S24Packed, Borrowed<'a>>> {
        self.inner.format = Format::S24.into();
        Decoder::<S24Packed, Borrowed<'a>>::init_s24_packed_ref_from_memory(data, self)
    }

    pub fn s24_memory<'a>(&mut self, data: &'a [u8]) -> MaResult<Decoder<S24, Borrowed<'a>>> {
        self.inner.format = Format::S24.into();
        Decoder::<S24, Borrowed<'a>>::init_s24_ref_from_memory(data, self)
    }

    pub fn i32_memory<'a>(&mut self, data: &'a [u8]) -> MaResult<Decoder<i32, Borrowed<'a>>> {
        self.inner.format = Format::S32.into();
        Decoder::<i32, Borrowed<'a>>::init_i32_ref_from_memory(data, self)
    }

    pub fn f32_memory<'a>(&mut self, data: &'a [u8]) -> MaResult<Decoder<f32, Borrowed<'a>>> {
        self.inner.format = Format::F32.into();
        Decoder::<f32, Borrowed<'a>>::init_f32_ref_from_memory(data, self)
    }

    pub fn copy_u8_memory<D: Into<Arc<[u8]>>>(&mut self, data: D) -> MaResult<Decoder<u8, Owned>> {
        self.inner.format = Format::U8.into();
        Decoder::<u8, Owned>::init_u8_copy_from_memory(data, self)
    }

    pub fn copy_i16_memory<D: Into<Arc<[u8]>>>(
        &mut self,
        data: D,
    ) -> MaResult<Decoder<i16, Owned>> {
        self.inner.format = Format::S16.into();
        Decoder::<i16, Owned>::init_i16_copy_from_memory(data, self)
    }

    pub fn copy_s24_packed_memory<D: Into<Arc<[u8]>>>(
        &mut self,
        data: D,
    ) -> MaResult<Decoder<S24Packed, Owned>> {
        self.inner.format = Format::S24.into();
        Decoder::<S24Packed, Owned>::init_s24_copy_from_memory(data, self)
    }

    pub fn copy_s24_memory<D: Into<Arc<[u8]>>>(
        &mut self,
        _data: D,
    ) -> MaResult<Decoder<S24, Owned>> {
        self.inner.format = Format::S24.into();
        // Decoder::<S24, Owned>::init_s24_copy_from_memory(data, self)
        todo!()
    }

    pub fn copy_i32_memory<D: Into<Arc<[u8]>>>(
        &mut self,
        data: D,
    ) -> MaResult<Decoder<i32, Owned>> {
        self.inner.format = Format::S32.into();
        Decoder::<i32, Owned>::init_i32_copy_from_memory(data, self)
    }

    pub fn copy_f32_memory<D: Into<Arc<[u8]>>>(
        &mut self,
        data: D,
    ) -> MaResult<Decoder<f32, Owned>> {
        self.inner.format = Format::F32.into();
        Decoder::<f32, Owned>::init_f32_copy_from_memory(data, self)
    }

    pub fn u8_file(&mut self, path: &Path) -> MaResult<Decoder<u8, External>> {
        self.inner.format = Format::U8.into();
        Decoder::<u8, External>::init_u8_file(path, self)
    }

    pub fn i16_file(&mut self, path: &Path) -> MaResult<Decoder<i16, External>> {
        self.inner.format = Format::S16.into();
        Decoder::<i16, External>::init_i16_file(path, self)
    }

    pub fn s24_file(&mut self, path: &Path) -> MaResult<Decoder<S24Packed, External>> {
        self.inner.format = Format::S24.into();
        Decoder::<S24Packed, External>::init_s24_file(path, self)
    }

    pub fn i32_file(&mut self, path: &Path) -> MaResult<Decoder<i32, External>> {
        self.inner.format = Format::S32.into();
        Decoder::<i32, External>::init_i32_file(path, self)
    }

    pub fn f32_file(&mut self, path: &Path) -> MaResult<Decoder<f32, External>> {
        self.inner.format = Format::F32.into();
        Decoder::<f32, External>::init_f32_file(path, self)
    }
}

pub(crate) mod decoder_b_ffi {
    use crate::{
        audio::{formats::Format, sample_rate::SampleRate},
        data_source::sources::decoder::DecoderBuilder,
    };
    use maudio_sys::ffi as sys;

    pub fn ma_decoder_config_init(
        out_channels: u32,
        out_sample_rate: SampleRate,
    ) -> DecoderBuilder {
        // Placeholder. Will be changed when Decoder is built.
        let out_format = Format::U8;
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

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::time::{SystemTime, UNIX_EPOCH};

//     fn unique_tmp_path(ext: &str) -> std::path::PathBuf {
//         let mut p = std::env::temp_dir();
//         let nanos = SystemTime::now()
//             .duration_since(UNIX_EPOCH)
//             .unwrap()
//             .as_nanos();
//         p.push(format!("miniaudio_decoder_test_{nanos}.{ext}"));
//         p
//     }

//     /// Build a minimal PCM 16-bit little-endian WAV file.
//     fn wav_i16_le(channels: u16, sample_rate: u32, samples_interleaved: &[i16]) -> Vec<u8> {
//         assert!(channels > 0);
//         assert_eq!(samples_interleaved.len() % channels as usize, 0);

//         let bits_per_sample: u16 = 16;
//         let block_align: u16 = channels * (bits_per_sample / 8);
//         let byte_rate: u32 = sample_rate * block_align as u32;
//         let data_bytes_len: u32 = (samples_interleaved.len() * 2) as u32;

//         let riff_chunk_size: u32 = 4 + (8 + 16) + (8 + data_bytes_len);

//         let mut out = Vec::with_capacity((8 + riff_chunk_size) as usize);

//         out.extend_from_slice(b"RIFF");
//         out.extend_from_slice(&riff_chunk_size.to_le_bytes());
//         out.extend_from_slice(b"WAVE");

//         out.extend_from_slice(b"fmt ");
//         out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
//         out.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = 1 (PCM)
//         out.extend_from_slice(&channels.to_le_bytes());
//         out.extend_from_slice(&sample_rate.to_le_bytes());
//         out.extend_from_slice(&byte_rate.to_le_bytes());
//         out.extend_from_slice(&block_align.to_le_bytes());
//         out.extend_from_slice(&bits_per_sample.to_le_bytes());

//         out.extend_from_slice(b"data");
//         out.extend_from_slice(&data_bytes_len.to_le_bytes());

//         for s in samples_interleaved {
//             out.extend_from_slice(&s.to_le_bytes());
//         }

//         out
//     }

//     fn tiny_test_wav_mono(frames: usize) -> Vec<u8> {
//         let mut samples = Vec::with_capacity(frames);
//         for i in 0..frames {
//             samples.push(((i as i32 * 300) % i16::MAX as i32) as i16);
//         }
//         wav_i16_le(1, 48_000, &samples)
//     }

//     #[test]
//     fn test_decoder_from_memory_f32_read_seek_cursor_length_available() {
//         let frames_total: usize = 64;
//         let wav = tiny_test_wav_mono(frames_total);

//         let builder = DecoderBuilder::new(Format::F32, 1, SampleRate::Sr48000);

//         let mut dec = builder.copy_from_memory(wav).unwrap();

//         let len = dec.length_pcm().unwrap();
//         assert_eq!(len as usize, frames_total);

//         let cursor0 = dec.cursor_pcm().unwrap();
//         assert_eq!(cursor0, 0);

//         let avail0 = dec.available_frames().unwrap();
//         assert_eq!(avail0 as usize, frames_total);

//         let df = dec.data_format().unwrap();
//         assert_eq!(df.channels, 1);
//         assert_eq!(df.sample_rate, 48_000);
//         assert_eq!(df.format, Format::F32);

//         let (buf, read) = dec.read_pcm_frames_f32(10).unwrap();
//         assert_eq!(read, 10);
//         assert_eq!(buf.len_samples(), 10);

//         let cursor1 = dec.cursor_pcm().unwrap();
//         assert_eq!(cursor1, 10);

//         let avail1 = dec.available_frames().unwrap();
//         assert_eq!(avail1 as usize, frames_total - 10);

//         dec.seek_to_pcm_frame(0).unwrap();
//         assert_eq!(dec.cursor_pcm().unwrap(), 0);

//         let (_buf2, read2) = dec.read_pcm_frames_f32(7).unwrap();
//         assert_eq!(read2, 7);
//         assert_eq!(dec.cursor_pcm().unwrap(), 7);
//     }

//     #[test]
//     fn test_decoder_ref_from_memory_decodes() {
//         let frames_total: usize = 32;
//         let wav = tiny_test_wav_mono(frames_total);

//         let builder = DecoderBuilder::new(Format::S16, 1, SampleRate::Sr48000);

//         let mut dec_ref = builder.ref_from_memory(&wav).unwrap();

//         let (buf, read) = dec_ref.read_pcm_frames_s16(12).unwrap();
//         assert_eq!(read, 12);
//         assert_eq!(buf.len_samples(), 12);
//     }

//     #[test]
//     fn test_decoder_from_file_reads_and_reports_length() {
//         let frames_total: usize = 40;
//         let wav = tiny_test_wav_mono(frames_total);

//         let guard = TempFileGuard::new(unique_tmp_path("wav"));
//         std::fs::write(guard.path(), &wav).unwrap();

//         let builder = DecoderBuilder::new(Format::F32, 1, SampleRate::Sr48000);

//         let mut dec = builder.from_file(guard.path()).unwrap();

//         assert_eq!(dec.length_pcm().unwrap() as usize, frames_total);
//         assert_eq!(dec.cursor_pcm().unwrap(), 0);

//         let (_buf, read) = dec.read_pcm_frames_f32(1000).unwrap();
//         assert_eq!(read as usize, frames_total);
//     }

//     #[test]
//     fn test_decoder_read_variants_return_expected_lengths() {
//         let frames_total: usize = 16;
//         let wav = tiny_test_wav_mono(frames_total);
//         let wav: Arc<[u8]> = wav.into();

//         let cases = [
//             (Format::U8, "u8"),
//             (Format::S16, "s16"),
//             (Format::S24, "s24"),
//             (Format::S32, "s32"),
//             (Format::F32, "f32"),
//         ];

//         for (fmt, _name) in cases {
//             let builder = DecoderBuilder::new(fmt, 1, SampleRate::Sr48000);

//             let mut dec = builder.copy_from_memory(wav.clone()).unwrap();

//             let (len_units, read) = match fmt {
//                 Format::U8 => {
//                     let (b, r) = dec.read_pcm_frames_u8(5).unwrap();
//                     (b.len_samples(), r)
//                 }
//                 Format::S16 => {
//                     let (b, r) = dec.read_pcm_frames_s16(5).unwrap();
//                     (b.len_samples(), r)
//                 }
//                 Format::S24 => {
//                     let (b, r) = dec.read_pcm_frames_s24(5).unwrap();
//                     (b.len_samples(), r)
//                 }
//                 Format::S32 => {
//                     let (b, r) = dec.read_pcm_frames_s32(5).unwrap();
//                     (b.len_samples(), r)
//                 }
//                 Format::F32 => {
//                     let (b, r) = dec.read_pcm_frames_f32(5).unwrap();
//                     (b.len_samples(), r)
//                 }
//             };

//             assert_eq!(read, 5);

//             let expected_units = match fmt {
//                 Format::S24 => 15,
//                 _ => 5,
//             };

//             assert_eq!(len_units, expected_units);
//         }
//     }
// }

// struct TempFileGuard {
//     path: std::path::PathBuf,
// }

// impl TempFileGuard {
//     fn new(path: std::path::PathBuf) -> Self {
//         Self { path }
//     }

//     fn path(&self) -> &std::path::Path {
//         &self.path
//     }
// }

// impl Drop for TempFileGuard {
//     fn drop(&mut self) {
//         let _ = std::fs::remove_file(&self.path);
//     }
// }
