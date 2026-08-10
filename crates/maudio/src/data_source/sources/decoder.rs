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
        channels::{Channel, RawChannel},
        formats::{Format, SampleBuffer},
        sample_rate::SampleRate,
    },
    data_source::{data_source_ffi, private_data_source, AsSourcePtr, DataFormat, DataSourceRef},
    device::device_builder::Unknown,
    pcm_frames::{PcmFormat, S24Packed},
    AsRawRef, Binding, MaResult,
};

pub mod custom_decoder;
pub(crate) mod decoder_vtable;
pub mod decoding_backend;

/// Streaming audio decoder.
///
/// A `Decoder` reads encoded audio data from some source `S` and produces
/// interleaved PCM frames in format `F` on demand.
///
/// Instances are created with [`DecoderBuilder`] from one of:
///
/// - a filesystem path
/// - a custom reader
/// - an in-memory byte slice or owned byte buffer
///
/// # Thread-safety
///
/// This decoder does not support concurrent access.
///
/// To read from or seek the decoder across multiple threads, wrap it in
/// [`Arc<Mutex<_>>`](std::sync::Arc).
///
/// # What a decoder does
///
/// A decoder behaves like a `DataSource` because the decoding backend produces a `DataSource`.
/// It sits between encoded input data and raw PCM output:
///
/// - encoded bytes come from the source chosen when the decoder is created
/// - PCM frames are produced and they are fed into miniaudio in the form of a `DataSource`
///
/// This makes decoders well suited for:
///
/// - large audio files
/// - streamed or incremental playback
/// - directly feeding sounds, engines or node graphs
///
/// # Examples
///
/// Decoding from a custom reader:
///
/// ```no_run
/// # use std::fs::File;
/// # use maudio::data_source::sources::decoder::DecoderBuilder;
/// # use maudio::{MaResult, audio::sample_rate::SampleRate};
/// # fn main() -> MaResult<()> {
/// let file = File::open("audio.flac").unwrap();
///
/// let decoder = DecoderBuilder::new_f32()
///     .from_reader(file)?;
/// # let _ = decoder;
/// # Ok(())
/// # }
/// ```
///
/// Decoding from a file path:
///
/// ```no_run
/// # use std::path::PathBuf;
/// # use maudio::data_source::sources::decoder::DecoderBuilder;
/// # use maudio::{MaResult, audio::sample_rate::SampleRate};
/// # fn main() -> MaResult<()> {
/// let path = PathBuf::from("audio.wav");
///
/// let decoder = DecoderBuilder::new_f32()
///     .from_file(&path)?;
/// # let _ = decoder;
/// # Ok(())
/// # }
/// ```
///
/// Decoding from memory without copying:
///
/// ```no_run
/// # use maudio::data_source::sources::decoder::DecoderBuilder;
/// # use maudio::{MaResult, audio::sample_rate::SampleRate};
/// # fn main() -> MaResult<()> {
/// let bytes: &[u8] = &[];
///
/// let decoder = DecoderBuilder::new_f32()
///     .from_memory(bytes)?;
/// # let _ = decoder;
/// # Ok(())
/// # }
/// ```
#[allow(unused)]
pub struct Decoder<F: PcmFormat, S> {
    inner: *mut sys::ma_decoder,
    channels: u32,
    format: Format,
    user_data: Option<DecoderUserDataDestructor>,
    _sample_format: PhantomData<F>,
    source_data: S,
}

unsafe impl<F: PcmFormat, S> Send for Decoder<F, S> {}

type DecoderUserDataDestructor = (*mut core::ffi::c_void, fn(*mut core::ffi::c_void));

/// Borrowed in-memory audio data used as a decoder source.
#[allow(unused)]
pub struct Borrowed<'a>(&'a [u8]);

/// Owned in-memory audio data used as a decoder source.
#[allow(unused)]
pub struct Owned(Arc<[u8]>);
/// Data source or destination is in a filesystem (e.g., file path) managed by miniaudio.
pub struct Fs;
/// Data source or destination is a callback (reader or writer)
pub struct Cb;

impl<F: PcmFormat, S> Binding for Decoder<F, S> {
    type Raw = *mut sys::ma_decoder;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat, S> Decoder<F, S> {
    #[inline]
    fn new(inner: *mut sys::ma_decoder, format: Format, source_data: S) -> Self {
        let ptr = unsafe { &*inner };
        let channels = ptr.converter.channelConverter.channelsOut;
        Self {
            inner,
            channels,
            format,
            user_data: None,
            _sample_format: PhantomData,
            source_data,
        }
    }

    fn init_from_memory<'a>(
        data: &'a [u8],
        config: &mut DecoderBuilder<F>,
    ) -> MaResult<Decoder<F, Borrowed<'a>>> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        decoder_ffi::ma_decoder_init_memory(
            data.as_ptr() as *const _,
            data.len(),
            config.as_raw_ptr(),
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(Decoder::new(inner, config.format, Borrowed(data)))
    }

    fn init_copy<D: Into<Arc<[u8]>>>(
        data: D,
        config: &mut DecoderBuilder<F>,
    ) -> MaResult<Decoder<F, Owned>> {
        let data_arc = data.into();
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        decoder_ffi::ma_decoder_init_memory(
            data_arc.as_ptr() as *const _,
            data_arc.len(),
            config.as_raw_ptr(),
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(Decoder::new(inner, config.format, Owned(data_arc)))
    }

    fn init_file(path: &Path, config: &mut DecoderBuilder<F>) -> MaResult<Decoder<F, Fs>> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        Decoder::<F, S>::init_from_file_internal(path, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(Decoder::new(inner, config.format, Fs))
    }

    fn init_from_reader<R: SeekRead>(
        reader: R,
        config: &mut DecoderBuilder<F>,
    ) -> MaResult<Decoder<F, Cb>> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        let user_data = Box::new(DecoderUserData { reader });
        let user_data_ptr = Box::into_raw(user_data) as *mut _;

        if let Err(e) = decoder_ffi::ma_decoder_init(
            Some(decoder_read_proc::<R>),
            Some(decoder_seek_proc::<R>),
            user_data_ptr,
            config.as_raw_ptr(),
            mem.as_mut_ptr(),
        ) {
            drop(unsafe { Box::from_raw(user_data_ptr as *mut DecoderUserData<R>) });
            return Err(e);
        }

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        let mut decoder = Decoder::new(inner, config.format, Cb);
        decoder.user_data = Some((user_data_ptr, encoder_user_data_drop::<R>));

        Ok(decoder)
    }

    fn init_from_file_internal(
        path: &Path,
        config: &DecoderBuilder<F>,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            use crate::cstring_from_path;

            let path = cstring_from_path(path)?;
            decoder_ffi::ma_decoder_init_file(path, config.as_raw_ptr(), decoder)
        }

        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated;

            let path = wide_null_terminated(path);

            decoder_ffi::ma_decoder_init_file_w(&path, config.as_raw_ptr(), decoder)
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }
}

/// Trait alias for types that implement both [`std::io::Read`] and [`std::io::Seek`].
///
/// This is used by [`DecoderBuilder::from_reader`] to accept custom input
/// sources for decoder audio data.
pub trait SeekRead: std::io::Read + std::io::Seek {}
impl<T: std::io::Read + std::io::Seek> SeekRead for T {}

struct DecoderUserData<R> {
    reader: R,
}

unsafe extern "C" fn decoder_read_proc<R: SeekRead>(
    decoder: *mut sys::ma_decoder,
    buffer_out: *mut core::ffi::c_void,
    bytes_to_read: usize,
    bytes_read: *mut usize,
) -> sys::ma_result {
    if decoder.is_null() || buffer_out.is_null() || bytes_read.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    // Make sure we don't leave this uninitialized
    *bytes_read = 0;

    let user_data = &mut *((&*decoder).pUserData as *mut DecoderUserData<R>);

    let slice = core::slice::from_raw_parts_mut(buffer_out as _, bytes_to_read);

    match user_data.reader.read(slice) {
        // If the number of bytes actually read is less than the number of bytes
        // requested (bytes_to_read), miniaudio will treat
        // it as if the end of the file has been reached
        Ok(0) => sys::ma_result_MA_AT_END,
        Ok(n) => {
            *bytes_read = n;
            sys::ma_result_MA_SUCCESS
        }
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn decoder_seek_proc<R: SeekRead>(
    decoder: *mut sys::ma_decoder,
    byte_offset: i64,
    origin: sys::ma_seek_origin,
) -> sys::ma_result {
    if decoder.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    let user_data = &mut *((&*decoder).pUserData as *mut DecoderUserData<R>);

    let pos = match origin {
        sys::ma_seek_origin_ma_seek_origin_start => {
            if byte_offset < 0 {
                return sys::ma_result_MA_INVALID_ARGS;
            }
            std::io::SeekFrom::Start(byte_offset as _)
        }
        sys::ma_seek_origin_ma_seek_origin_current => std::io::SeekFrom::Current(byte_offset as _),
        sys::ma_seek_origin_ma_seek_origin_end => std::io::SeekFrom::End(byte_offset as _),
        _ => return sys::ma_result_MA_INVALID_ARGS,
    };

    match user_data.reader.seek(pos) {
        Ok(_) => sys::ma_result_MA_SUCCESS,
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

#[allow(unused)]
unsafe extern "C" fn decoder_seek_proc_no_op(
    decoder: *mut sys::ma_decoder,
    _byte_offset: i64,
    _origin: sys::ma_seek_origin,
) -> sys::ma_result {
    if decoder.is_null() {
        return sys::ma_result_MA_ERROR;
    }

    sys::ma_result_MA_SUCCESS
}

// Keeps the as_decoder_ptr method on AsDecoderPtr private.
// Could be removed as there is no DecoderRef
mod private_decoder {
    use crate::data_source::sources::decoder::custom_decoder::CustomDecoder;

    use super::*;
    use maudio_sys::ffi as sys;

    pub trait DecoderPtrProvider<T: ?Sized> {
        fn as_decoder_ptr(t: &T) -> *mut sys::ma_decoder;
    }

    pub struct DecoderProvider;
    pub struct CustomDecoderProvider;

    impl<F: PcmFormat, S> DecoderPtrProvider<CustomDecoder<F, S>> for CustomDecoderProvider {
        fn as_decoder_ptr(t: &CustomDecoder<F, S>) -> *mut sys::ma_decoder {
            t.to_raw()
        }
    }

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
    type Format = F;
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

impl<F: PcmFormat, S> DecoderOps for Decoder<F, S> {
    type Source = S;
}

pub trait DecoderOps: AsDecoderPtr + AsSourcePtr {
    type Source;

    /// Reads PCM frames into `dst`, returning the number of frames read.
    fn read_pcm_frames_into(
        &mut self,
        dst: &mut [<Self::Format as PcmFormat>::PcmUnit],
    ) -> MaResult<usize> {
        decoder_ffi::ma_decoder_read_pcm_frames_into::<Self::Format, Self>(self, dst)
    }

    /// Allocates and reads `frame_count` PCM frames, returning a typed sample buffer.
    fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<SampleBuffer<Self::Format>> {
        decoder_ffi::ma_decoder_read_pcm_frames(self, frame_count)
    }

    /// Seeks to an absolute PCM frame index.
    fn seek_to_pcm_frame(&mut self, frame_index: u64) -> MaResult<()> {
        decoder_ffi::ma_decoder_seek_to_pcm_frame(self, frame_index)
    }

    /// Returns the output PCM format of the underlying source source.
    /// This may differ from the output format, as the decoder incorporates a sample rate and channelc converter.
    fn backend_data_format(&self) -> MaResult<DataFormat> {
        decoder_ffi::ma_decode_get_backend_data_format(self)
    }

    /// Returns the output PCM format of the decoder.
    fn data_format(&self) -> MaResult<DataFormat> {
        decoder_ffi::ma_decoder_get_data_format(self)
    }

    fn looping(&mut self) -> bool {
        data_source_ffi::ma_data_source_is_looping(self)
    }

    fn set_looping(&mut self, looping: bool) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_looping(self, looping)
    }

    /// Returns the current cursor position in PCM frames.
    fn cursor_pcm(&self) -> MaResult<u64> {
        decoder_ffi::ma_decoder_get_cursor_in_pcm_frames(self)
    }

    fn cursor_in_seconds(&self) -> MaResult<f32> {
        data_source_ffi::ma_data_source_get_cursor_in_seconds(self)
    }

    /// Returns the total length in PCM frames.
    fn length_pcm(&self) -> MaResult<u64> {
        decoder_ffi::ma_decoder_get_length_in_pcm_frames(self)
    }

    fn length_in_seconds(&self) -> MaResult<f32> {
        data_source_ffi::ma_data_source_get_length_in_seconds(self)
    }

    /// Returns the number of frames available from the current cursor to the end.
    fn available_frames(&self) -> MaResult<u64> {
        decoder_ffi::ma_decoder_get_available_frames(self)
    }

    /// Returns a [`DataSourceRef`] view of this decoder.
    fn as_source_ref<'a>(&'a self) -> DataSourceRef<'a, Self::Format> {
        debug_assert!(!private_decoder::decoder_ptr(self).is_null());
        // let ptr = private_decoder::decoder_ptr(self).cast::<sys::ma_data_source>();
        // DataSourceRef::from_ptr(ptr)
        let ptr = private_decoder::decoder_ptr(self);
        let backend = unsafe { &*ptr }.pBackend.cast::<sys::ma_data_source>();
        debug_assert!(!backend.is_null());
        DataSourceRef::from_ptr(backend)
    }
}

pub(crate) mod decoder_ffi {
    use maudio_sys::ffi as sys;

    use crate::audio::channels::Channel;
    use crate::audio::formats::SampleBuffer;
    use crate::audio::sample_rate::SampleRate;
    use crate::data_source::{
        sources::decoder::{private_decoder, AsDecoderPtr},
        DataFormat,
    };
    use crate::pcm_frames::{PcmFormat, PcmFormatInternal};
    use crate::{MaResult, MaudioError};

    #[inline]
    pub fn ma_decoder_init(
        on_read: sys::ma_decoder_read_proc,
        on_seek: sys::ma_decoder_seek_proc,
        user_data: *mut core::ffi::c_void,
        config: *const sys::ma_decoder_config,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_decoder_init(on_read, on_seek, user_data, config, decoder) };
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
        config: *const sys::ma_decoder_config,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_decoder_init_memory(data, data_size, config, decoder) };
        MaudioError::check(res)
    }

    #[inline]
    #[cfg(unix)]
    pub fn ma_decoder_init_file(
        path: std::ffi::CString,
        config: *const sys::ma_decoder_config,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_decoder_init_file(path.as_ptr(), config, decoder) };
        MaudioError::check(res)
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_decoder_init_file_w(
        path: &[u16],
        config: *const sys::ma_decoder_config,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_decoder_init_file_w(path.as_ptr(), config, decoder) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_decoder_read_pcm_frames_into<F: PcmFormat, D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        dst: &mut [F::PcmUnit],
    ) -> MaResult<usize> {
        let channels = decoder.channels();
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        // The frame count we can fit in the final dst in PcmUnit
        let frame_count = (dst.len() / channels as usize / F::VEC_PCM_UNITS_PER_FRAME) as u64;

        match F::DIRECT_READ {
            true => {
                // Read directly into destination
                let frames_read = ma_decoder_read_pcm_frames_internal(
                    decoder,
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
                let frames_read = ma_decoder_read_pcm_frames_internal(
                    decoder,
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

    pub fn ma_decoder_read_pcm_frames<F: PcmFormat, D: AsDecoderPtr + ?Sized>(
        decoder: &mut D,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<F>> {
        let mut buffer = SampleBuffer::<F>::new_zeroed(frame_count as usize, decoder.channels())?;

        let frames_read = ma_decoder_read_pcm_frames_internal(
            decoder,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<F>::from_storage(buffer, frames_read as usize, decoder.channels())
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
    pub fn ma_decode_get_backend_data_format<D: AsDecoderPtr + ?Sized>(
        decoder: &D,
    ) -> MaResult<DataFormat> {
        let ptr = private_decoder::decoder_ptr(decoder);
        debug_assert!(!ptr.is_null());
        let decoder = unsafe { &*ptr };
        let format_raw = decoder.outputFormat; // format conversion is not possible in maudio yet
        let channels = decoder.converter.channelConverter.channelsIn;
        let sample_rate: SampleRate = decoder.converter.sampleRateIn.try_into()?;

        let mut channel_map_raw = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];

        let res = unsafe {
            sys::ma_channel_converter_get_input_channel_map(
                &decoder.converter.channelConverter as *const _,
                channel_map_raw.as_mut_ptr(),
                channels as usize,
            )
        };
        MaudioError::check(res)?;

        let mut channel_map: Vec<Channel> = Vec::new();
        for c in channel_map_raw {
            channel_map.push(Channel::try_from(c)?);
        }
        channel_map.truncate(channels as usize);

        Ok(DataFormat {
            format: format_raw.try_into()?,
            channels,
            sample_rate,
            channel_map,
        })
    }

    #[inline]
    pub fn ma_decoder_get_data_format<D: AsDecoderPtr + ?Sized>(
        decoder: &D,
    ) -> MaResult<DataFormat> {
        let ptr = private_decoder::decoder_ptr(decoder);
        debug_assert!(!ptr.is_null());
        let decoder = unsafe { &*ptr };
        let format_raw = decoder.outputFormat;
        let channels = decoder.converter.channelConverter.channelsOut;
        let sample_rate: SampleRate = decoder.converter.sampleRateOut.try_into()?;

        let mut channel_map_raw = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];

        let res = unsafe {
            sys::ma_channel_converter_get_output_channel_map(
                &decoder.converter.channelConverter as *const _,
                channel_map_raw.as_mut_ptr(),
                channels as usize,
            )
        };
        MaudioError::check(res)?;

        let mut channel_map: Vec<Channel> = Vec::new();
        for c in channel_map_raw {
            channel_map.push(Channel::try_from(c)?);
        }
        channel_map.truncate(channels as usize);

        Ok(DataFormat {
            format: format_raw.try_into()?,
            channels,
            sample_rate,
            channel_map,
        })

        // TODO: Change this back when 0.11.26 fixes the bug in the data converter
        // let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        // let mut channels: sys::ma_uint32 = 0;
        // let mut sample_rate: sys::ma_uint32 = 0;

        // let mut channel_map_raw = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];

        // let res = unsafe {
        //     sys::ma_decoder_get_data_format(
        //         private_decoder::decoder_ptr(decoder),
        //         &mut format_raw,
        //         &mut channels,
        //         &mut sample_rate,
        //         channel_map_raw.as_mut_ptr(),
        //         channel_map_raw.len(),
        //     )
        // };
        // MaudioError::check(res)?;

        // // Could maybe cast when passing the ptr to miniaudio, but copying should be fine here
        // let mut channel_map: Vec<Channel> = Vec::new();
        // for c in channel_map_raw {
        //     channel_map.push(Channel::try_from(c)?);
        // }
        // channel_map.truncate(channels as usize);

        // Ok(DataFormat {
        //     format: format_raw.try_into()?,
        //     channels,
        //     sample_rate: sample_rate.try_into()?,
        //     channel_map,
        // })
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
    #[allow(unused)]
    fn ma_decode_file(
        path: *const core::ffi::c_char,
        config: *mut sys::ma_decoder_config,
        frame_count_out: *mut u64,
        frames_out: *mut *mut core::ffi::c_void,
    ) {
        let _res = unsafe { sys::ma_decode_file(path, config, frame_count_out, frames_out) };
    }

    // TODO: Not implemented. Look into usefulness
    #[allow(unused)]
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

fn encoder_user_data_drop<R: SeekRead>(ptr: *mut core::ffi::c_void) {
    drop(unsafe { Box::from_raw(ptr as *mut DecoderUserData<R>) });
}

impl<F: PcmFormat, S> Drop for Decoder<F, S> {
    fn drop(&mut self) {
        let _ = decoder_ffi::ma_decoder_uninit(self);
        if let Some((ptr, destructor)) = self.user_data {
            destructor(ptr);
        }
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub struct DecoderBuilder<F = Unknown> {
    config: sys::ma_decoder_config,
    format: Format,
    channels: Option<u32>,
    channel_map: Vec<RawChannel>,
    sample_rate: Option<SampleRate>,
    _format: PhantomData<F>,
}

impl<F: PcmFormat> AsRawRef for DecoderBuilder<F> {
    type Raw = sys::ma_decoder_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.config
    }
}

impl DecoderBuilder<Unknown> {
    #[inline]
    fn build<F: PcmFormat>(format: Format) -> DecoderBuilder<F> {
        let mut config = unsafe { sys::ma_decoder_config_init_default() };
        config.resampling = unsafe {
            sys::ma_resampler_config_init(
                format.into(),
                0,
                0,
                0,
                sys::ma_resample_algorithm_ma_resample_algorithm_linear,
            )
        };
        config.format = format.into();
        DecoderBuilder {
            config,
            format,
            channels: None,
            channel_map: Vec::new(),
            sample_rate: None,
            _format: PhantomData,
        }
    }

    pub fn new_u8() -> DecoderBuilder<u8> {
        Self::build(Format::U8)
    }

    pub fn new_i16() -> DecoderBuilder<i16> {
        Self::build(Format::S16)
    }

    pub fn new_i32() -> DecoderBuilder<i32> {
        Self::build(Format::S32)
    }

    pub fn new_s24_packed() -> DecoderBuilder<S24Packed> {
        Self::build(Format::S24Packed)
    }

    pub fn new_f32() -> DecoderBuilder<f32> {
        Self::build(Format::F32)
    }
}

impl<F: PcmFormat> DecoderBuilder<F> {
    /// Select the output channel count for the decoder
    ///
    /// If not set, it will output the native channel count of the source
    /// If a channel map is not provided, a standard channel map will be generated.
    pub fn channels(&mut self, channels: u32) -> &mut Self {
        self.channels = Some(channels);
        self.config.channels = channels;
        self
    }

    /// Select the output sample rate for the decoder
    ///
    /// If not set, it will output the native sample rate of the source
    pub fn sample_rate(&mut self, sample_rate: SampleRate) -> &mut Self {
        self.sample_rate = Some(sample_rate);
        self.config.sampleRate = sample_rate.into();
        self
    }

    /// Set the channel map of the output.
    /// This map may differ from the input channel map, as the decoder implements a channel converter.
    ///
    /// Also sets the channel count to the length of the channel map
    /// If not set, miniaudio will generate a standard channel map for the channel count.
    pub fn set_channel_map<I>(mut self, channel_map: I) -> Self
    where
        I: IntoIterator<Item = Channel>,
    {
        self.channel_map = channel_map.into_iter().map(RawChannel::from).collect();
        self.config.channels = self.channel_map.len() as u32;
        self.channels = Some(self.config.channels);
        self.config.pChannelMap = self.channel_map.as_mut_ptr() as *mut _;
        self
    }

    /// Creates a decoder from borrowed in-memory audio data.
    ///
    /// This uses `ma_decoder_init_memory`.
    ///
    /// The input bytes are borrowed for the lifetime of the returned decoder,
    /// so the data must remain valid for as long as the decoder exists.
    ///
    /// This is the most direct in-memory constructor when you already have the
    /// full encoded audio data available and can keep it alive externally.
    pub fn from_memory<'a>(&mut self, data: &'a [u8]) -> MaResult<Decoder<F, Borrowed<'a>>> {
        Decoder::<F, Borrowed<'a>>::init_from_memory(data, self)
    }

    /// Creates a decoder from owned in-memory audio data.
    ///
    /// This is the same as from_memory, but stores an owned copy of the
    /// encoded data inside the returned decoder.
    ///
    /// Use this when you want the decoder to own backing memory
    /// instead of borrowing it from the caller.
    pub fn copy_memory<D: Into<Arc<[u8]>>>(&mut self, data: D) -> MaResult<Decoder<F, Owned>> {
        Decoder::<F, Owned>::init_copy(data, self)
    }

    /// Creates a decoder from a file path.
    ///
    /// The file is opened and managed through miniaudio's file-based decoding
    /// path rather than by storing the file contents in Rust memory first.
    ///
    /// This is usually the most convenient option when decoding from a normal
    /// file on disk.
    pub fn from_file(&mut self, path: &Path) -> MaResult<Decoder<F, Fs>> {
        Decoder::<F, Fs>::init_file(path, self)
    }

    /// Creates a decoder from a custom Rust reader.
    ///
    /// The reader must implement [`SeekRead`], meaning it supports both
    /// [`std::io::Read`] and [`std::io::Seek`]. This makes it suitable for
    /// file-like sources such as:
    ///
    /// - [`std::fs::File`]
    /// - [`std::io::Cursor`]
    /// - buffered wrappers around seekable readers
    ///
    /// This constructor is intended for custom, seekable data sources.
    /// It is best suited to sources that behave like regular files or in-memory
    /// byte buffers.
    ///
    /// # Notes
    /// The reader is owned by the decoder and accessed through internal
    /// callbacks required by miniaudio.
    ///
    /// This constructor is not ideal for temporary "data not available yet"
    /// situations. The supplied reader should behave like a normal seekable
    /// byte source.
    ///
    /// If the source behaves like a stream and may temporarily provide fewer
    /// bytes than requested, this will be treated as EOF,
    /// and decoding will stop instead of waiting for more data.
    pub fn from_reader<R: SeekRead>(&mut self, reader: R) -> MaResult<Decoder<F, Cb>> {
        Decoder::<F, Cb>::init_from_reader(reader, self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        audio::channels::Channel,
        test_assets::{
            temp_file::{unique_tmp_path, TempFileGuard},
            wav_i16_le,
        },
    };

    use super::*;

    fn tiny_test_wav_mono(frames: usize) -> Vec<u8> {
        let mut samples = Vec::with_capacity(frames);
        for i in 0..frames {
            samples.push(((i as i32 * 300) % i16::MAX as i32) as i16);
        }
        wav_i16_le(1, SampleRate::Sr48000, &samples)
    }

    #[test]
    fn test_decoder_from_memory_f32_passthrough_data_format() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let dec = DecoderBuilder::new_f32().copy_memory(wav).unwrap();

        let flag = unsafe { &*dec.inner }.converter.isPassthrough;
        assert!(flag == 1);
        let res = dec.data_format();
        assert!(res.is_ok());
        let df = res.unwrap();
        assert_eq!(df.format, Format::F32);
        assert_eq!(df.sample_rate, SampleRate::Sr48000);
        assert_eq!(df.channels, 1);
        assert_eq!(df.channel_map, [Channel::Mono]);
    }

    #[test]
    fn test_decoder_from_memory_f32_change_data_format() {
        let frames_total: usize = 120;
        let wav = tiny_test_wav_mono(frames_total);

        let dec = DecoderBuilder::new_f32()
            .channels(2)
            .sample_rate(SampleRate::Sr44100)
            .copy_memory(wav)
            .unwrap();

        let flag = unsafe { &*dec.inner }.converter.isPassthrough;
        assert!(flag == 0);
        let has_ch_conv = unsafe { &*dec.inner }.converter.hasChannelConverter;
        assert!(has_ch_conv == 1);
        let res = dec.data_format();
        assert!(res.is_ok());
        let df = res.unwrap();
        assert_eq!(df.format, Format::F32);
        assert_eq!(df.sample_rate, SampleRate::Sr44100);
        assert_eq!(df.channels, 2);
        assert_eq!(df.channel_map, [Channel::FrontLeft, Channel::FrontRight]);
    }

    #[test]
    fn test_decoder_from_memory_f32_get_input_data_format() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let dec = DecoderBuilder::new_f32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .copy_memory(wav)
            .unwrap();

        let res = dec.backend_data_format();
        assert!(res.is_ok());
        let df = res.unwrap();
        assert_eq!(df.format, Format::F32);
        assert_eq!(df.sample_rate, SampleRate::Sr48000);
        assert_eq!(df.channels, 1);
        assert_eq!(df.channel_map, [Channel::Mono]);
    }

    #[test]
    fn test_decoder_from_memory_f32_change_channel_map() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let map = [
            Channel::TopBackLeft,
            Channel::TopBackLeft,
            Channel::TopFrontLeft,
            Channel::TopFrontRight,
        ];

        let dec = DecoderBuilder::new_f32()
            .set_channel_map(map)
            // .sample_rate(SampleRate::Sr48000)
            .copy_memory(wav)
            .unwrap();

        let res = dec.data_format();
        assert!(res.is_ok());
        let df = res.unwrap();
        assert_eq!(df.format, Format::F32);
        assert_eq!(df.sample_rate, SampleRate::Sr48000);
        assert_eq!(df.channels, 4);
        assert_eq!(df.channel_map, map);
    }

    #[test]
    fn test_decoder_from_memory_f32_read_seek_cursor_length_available() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let mut dec = DecoderBuilder::new_f32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .copy_memory(wav)
            .unwrap();

        let len = dec.length_pcm().unwrap();
        assert_eq!(len as usize, frames_total);

        let cursor0 = dec.cursor_pcm().unwrap();
        assert_eq!(cursor0, 0);

        let avail0 = dec.available_frames().unwrap();
        assert_eq!(avail0 as usize, frames_total);

        let df = dec.data_format().unwrap();
        assert_eq!(df.channels, 1);
        assert_eq!(df.sample_rate, SampleRate::Sr48000);
        assert_eq!(df.format, Format::F32);

        let buf = dec.read_pcm_frames(10).unwrap();
        let read = buf.frames();
        assert_eq!(read, 10);
        assert_eq!(buf.len(), 10);

        let cursor1 = dec.cursor_pcm().unwrap();
        assert_eq!(cursor1, 10);

        let avail1 = dec.available_frames().unwrap();
        assert_eq!(avail1 as usize, frames_total - 10);

        dec.seek_to_pcm_frame(0).unwrap();
        assert_eq!(dec.cursor_pcm().unwrap(), 0);

        let buf2 = dec.read_pcm_frames(7).unwrap();
        let read2 = buf2.frames();
        assert_eq!(read2, 7);
        assert_eq!(dec.cursor_pcm().unwrap(), 7);
    }

    #[test]
    fn test_decoder_ref_from_memory_decodes() {
        let frames_total: usize = 32;
        let wav = tiny_test_wav_mono(frames_total);

        let mut dec_ref = DecoderBuilder::new_f32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_memory(&wav)
            .unwrap();

        let buf = dec_ref.read_pcm_frames(12).unwrap();
        let read = buf.frames();
        assert_eq!(read, 12);
        assert_eq!(buf.len(), 12);
    }

    #[test]
    fn test_decoder_from_file_reads_and_reports_length() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let mut dec = DecoderBuilder::new_f32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_file(guard.path())
            .unwrap();

        assert_eq!(dec.length_pcm().unwrap() as usize, frames_total);
        assert_eq!(dec.cursor_pcm().unwrap(), 0);

        let buf = dec.read_pcm_frames(1000).unwrap();
        assert_eq!(buf.frames(), frames_total);
    }

    #[test]
    fn test_decoder_read_u8_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);

        let mut dec = DecoderBuilder::new_u8()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_memory(&wav)
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_decoder_read_i16_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);

        let mut dec = DecoderBuilder::new_i16()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_memory(&wav)
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_decoder_read_i32_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);

        let mut dec = DecoderBuilder::new_i32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_memory(&wav)
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_decoder_read_s24_packed_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);

        let mut dec = DecoderBuilder::new_s24_packed()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_memory(&wav)
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_decoder_read_f32_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);

        let mut dec = DecoderBuilder::new_f32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_memory(&wav)
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_decoder_read_u8_copy_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);
        let wav: Arc<[u8]> = wav.into();

        let mut dec = DecoderBuilder::new_f32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .copy_memory(wav.clone())
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_decoder_read_i16_copy_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);
        let wav: Arc<[u8]> = wav.into();

        let mut dec = DecoderBuilder::new_i16()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .copy_memory(wav.clone())
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_decoder_read_i32_copy_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);
        let wav: Arc<[u8]> = wav.into();

        let mut dec = DecoderBuilder::new_i32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .copy_memory(wav.clone())
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_decoder_read_s24_packed_copy_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);
        let wav: Arc<[u8]> = wav.into();

        let mut dec = DecoderBuilder::new_s24_packed()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .copy_memory(wav.clone())
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_decoder_read_f32_copy_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);
        let wav: Arc<[u8]> = wav.into();

        let mut dec = DecoderBuilder::new_f32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .copy_memory(wav.clone())
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_decoder_read_u8_path() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let mut dec = DecoderBuilder::new_u8()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_file(guard.path())
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_decoder_read_i16_path() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let mut dec = DecoderBuilder::new_i16()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_file(guard.path())
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_decoder_read_i32_path() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let mut dec = DecoderBuilder::new_i32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_file(guard.path())
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_decoder_read_s24_packed_path() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let mut dec = DecoderBuilder::new_s24_packed()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_file(guard.path())
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_decoder_read_f32_path() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let mut dec = DecoderBuilder::new_f32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_file(guard.path())
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_decoder_read_u8_file() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();
        let file = std::fs::File::open(guard.path()).unwrap();

        let mut dec = DecoderBuilder::new_u8()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_reader(file)
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_decoder_read_i16_file() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();
        let file = std::fs::File::open(guard.path()).unwrap();

        let mut dec = DecoderBuilder::new_i16()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_reader(file)
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_decoder_read_i32_file() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();
        let file = std::fs::File::open(guard.path()).unwrap();

        let mut dec = DecoderBuilder::new_i32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_reader(file)
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_decoder_read_s24_packed_file() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();
        let file = std::fs::File::open(guard.path()).unwrap();

        let mut dec = DecoderBuilder::new_s24_packed()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_reader(file)
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_decoder_read_f32_file() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();
        let file = std::fs::File::open(guard.path()).unwrap();

        let mut dec = DecoderBuilder::new_f32()
            .channels(1)
            .sample_rate(SampleRate::Sr48000)
            .from_reader(file)
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }
}
