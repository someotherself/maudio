//! Support for custom decoder backends.
//!
//! Miniaudio includes built-in support for decoding WAV, FLAC, MP3, and
//! Ogg Vorbis. This module allows applications to support additional formats
//! by integrating a third-party decoder with miniaudio's decoder interface.
//!
//! Maudio does not choose or provide additional decoding libraries. Instead,
//! users implement [`DecodingBackend`] using the decoder library that best
//! suits their application.
//!
//! Complete implementations can be found in the examples folder.
//!
//! # How custom decoding works
//!
//! A custom decoder consists of two parts:
//!
//! - A [DecodingBackend] implementation initializes the third-party decoder
//!   and reports the PCM format it produces.
//! - The decoder returned by the backend implements
//!   PcmSource, allowing
//!   miniaudio to read and seek through the decoded PCM frames.
//!
//! Call [CustomDecoderBuilder::backend] to add one or more decoding backends.
//! When multiple decoding backends are added, miniaudio will try them in the order they were added.
//! If the initialization fails for one backend, for any reason, it will try to the next one.
//! If none were able to be initialized, miniaudio will also try the built in decoding backend.
//!
//! When initialization begins, the backend receives a [DecoderStream](crate::data_source) for
//! reading and seeking through the encoded input. The backend uses this stream
//! to create its decoder and returns an object implementing [PcmSource](crate::data_source).
//!
//! [DecodingBackend::input_data_format] describes the native PCM output of
//! that object, including its sample format, channel count, sample rate, and
//! channel map. This is the input format seen by miniaudio.
//!
//! The format passed to [CustomDecoderBuilder] instead describes the PCM
//! output requested by the application. When the backend's native format
//! differs from the requested output format, miniaudio performs the necessary
//! sample-format conversion, channel conversion, and resampling.
use std::{marker::PhantomData, mem::MaybeUninit, path::Path, sync::Arc};

use crate::{
    audio::{
        channels::{Channel, RawChannel},
        formats::Format,
        sample_rate::SampleRate,
    },
    data_source::{
        data_source_ffi, private_data_source,
        sources::decoder::{
            decoder_ffi, decoder_read_proc, decoder_seek_proc, decoder_vtable::decoder_vtable,
            decoding_backend::DecodingBackend, encoder_user_data_drop, private_decoder,
            AsDecoderPtr, Borrowed, Cb, DecoderOps, DecoderUserData, DecoderUserDataDestructor, Fs,
            Owned, SeekRead,
        },
        AsSourcePtr, SourceContext,
    },
    device::device_builder::Unknown,
    pcm_frames::{PcmFormat, S24Packed},
    AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
};

use maudio_sys::ffi as sys;

pub struct CustomDecoder<F: PcmFormat, S> {
    inner: *mut sys::ma_decoder,
    channels: u32,
    format: Format,
    user_data: Option<DecoderUserDataDestructor>,
    backend_reg: *mut BackendRegistration<F>,
    _source_data: S,                                                 // keep alive
    _decoder_vtables: Box<[*const sys::ma_decoding_backend_vtable]>, // keep alive
    _sample_format: PhantomData<F>,
}

unsafe impl<F: PcmFormat, S> Send for CustomDecoder<F, S> {}

/// `backend_user_data` inside the onInit in ma_decoding_backend_vtable
///
/// This object contains any data / config that can be used inside the init function
/// to create the data source and (potentially) as config for the `DecodingBackend` trait
///
/// The information in this object will be available to all the vtables (backends)
pub(crate) struct BackendRegistration<F>
where
    F: PcmFormat,
{
    pub(crate) channels: u32,
    pub(crate) sample_rate: SampleRate,
    pub(crate) format: Format,
    pub(crate) channel_map: Vec<Channel>,
    _format: PhantomData<F>,
}

/// The Data Source create inside the onInit
#[repr(C)]
pub(crate) struct BackendDataSource<'stream, F, D>
where
    F: PcmFormat,
    D: DecodingBackend<Format = F> + 'stream,
{
    pub(crate) base: sys::ma_data_source_base,
    pub(crate) context: SourceContext,
    pub(crate) decoder: D::Decoder<'stream>,
    pub(crate) vtable: *const sys::ma_data_source_vtable,
    pub(crate) _format: PhantomData<F>,
}

impl<'stream, F, D> AsRawRef for BackendDataSource<'stream, F, D>
where
    F: PcmFormat,
    D: DecodingBackend<Format = F>,
{
    type Raw = sys::ma_data_source_base;

    fn as_raw(&self) -> &Self::Raw {
        &self.base
    }
}

impl<'stream, F, D> Drop for BackendDataSource<'stream, F, D>
where
    F: PcmFormat,
    D: DecodingBackend<Format = F>,
{
    fn drop(&mut self) {
        data_source_ffi::ma_data_source_uninit(self.as_raw_ptr() as *mut _);
        drop(unsafe { Box::from_raw(self.vtable as *mut sys::ma_data_source_vtable) });
    }
}

impl<F: PcmFormat, S> Binding for CustomDecoder<F, S> {
    type Raw = *mut sys::ma_decoder;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat, S> AsDecoderPtr for CustomDecoder<F, S> {
    #[doc(hidden)]
    type __PtrProvider = private_decoder::CustomDecoderProvider;

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

// Allows CustomDecoder to pass as a DataSource
#[doc(hidden)]
impl<F: PcmFormat, S> AsSourcePtr for CustomDecoder<F, S> {
    type Format = F;
    type __PtrProvider = private_data_source::CustomDecoderProvider;
}

impl<F: PcmFormat, S> DecoderOps for CustomDecoder<F, S> {
    type Source = S;
}

impl<F: PcmFormat, S> CustomDecoder<F, S> {
    #[inline]
    fn new(
        inner: *mut sys::ma_decoder,
        config: &CustomDecoderBuilder<F>,
        format: Format,
        vtables: Box<[*const sys::ma_decoding_backend_vtable]>,
        reg: *mut BackendRegistration<F>,
        source_data: S,
    ) -> Self {
        Self {
            inner,
            channels: config.channels,
            format,
            user_data: None,
            backend_reg: reg,
            _source_data: source_data,
            _sample_format: PhantomData,
            _decoder_vtables: vtables,
        }
    }

    fn init_file(
        path: &Path,
        config: &mut CustomDecoderBuilder<F>,
    ) -> MaResult<CustomDecoder<F, Fs>> {
        let vtables = config.set_backend_vtables()?;
        let map = config.set_channel_map()?;
        let reg = config.set_backend_registration(map.clone());

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        CustomDecoder::<F, S>::init_from_file_internal(path, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(CustomDecoder::new(
            inner,
            config,
            config.format,
            vtables,
            reg,
            Fs,
        ))
    }

    fn init_from_file_internal(
        path: &Path,
        config: &CustomDecoderBuilder<F>,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            use crate::engine::cstring_from_path;

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
    fn init_from_reader<R: SeekRead>(
        reader: R,
        config: &mut CustomDecoderBuilder<F>,
    ) -> MaResult<CustomDecoder<F, Cb>> {
        let vtables = config.set_backend_vtables()?;
        let map = config.set_channel_map()?;
        let reg = config.set_backend_registration(map.clone());
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
        let mut decoder = CustomDecoder::new(inner, config, config.format, vtables, reg, Cb);
        decoder.user_data = Some((user_data_ptr, encoder_user_data_drop::<R>));

        Ok(decoder)
    }

    fn init_from_memory<'a>(
        data: &'a [u8],
        config: &mut CustomDecoderBuilder<F>,
    ) -> MaResult<CustomDecoder<F, Borrowed<'a>>> {
        let vtables = config.set_backend_vtables()?;
        let map = config.set_channel_map()?;
        let reg = config.set_backend_registration(map.clone());

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        decoder_ffi::ma_decoder_init_memory(
            data.as_ptr() as *const _,
            data.len(),
            config.as_raw_ptr(),
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(CustomDecoder::new(
            inner,
            config,
            config.format,
            vtables,
            reg,
            Borrowed(data),
        ))
    }

    fn init_copy<D: Into<Arc<[u8]>>>(
        data: D,
        config: &mut CustomDecoderBuilder<F>,
    ) -> MaResult<CustomDecoder<F, Owned>> {
        let vtables = config.set_backend_vtables()?;
        let map = config.set_channel_map()?;
        let reg = config.set_backend_registration(map.clone());

        let data_arc = data.into();
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        decoder_ffi::ma_decoder_init_memory(
            data_arc.as_ptr() as *const _,
            data_arc.len(),
            config.as_raw_ptr(),
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(CustomDecoder::new(
            inner,
            config,
            config.format,
            vtables,
            reg,
            Owned(data_arc),
        ))
    }
}

/// Builder for a `CustomDecoder`.
///
/// When initialized, the channel count and sample rate of the output are expected.
/// They do not need to match the native format produced by the decoding backend.
///
/// When necessary, miniaudio converts the backend's output to the configured
/// channel count and map, and resamples it to the configured sample rate.
pub struct CustomDecoderBuilder<F = Unknown> {
    inner: sys::ma_decoder_config,
    vtables: Vec<*const sys::ma_decoding_backend_vtable>,
    sample_rate: SampleRate,
    channels: u32,
    format: Format,
    channel_map: Vec<RawChannel>,
    _format: PhantomData<F>,
}

impl<F: PcmFormat> AsRawRef for CustomDecoderBuilder<F> {
    type Raw = sys::ma_decoder_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl CustomDecoderBuilder<Unknown> {
    #[inline]
    fn new_inner(
        out_channels: u32,
        out_sample_rate: SampleRate,
        format: Format,
    ) -> sys::ma_decoder_config {
        let mut config = unsafe { sys::ma_decoder_config_init_default() };
        config.channels = out_channels;
        config.sampleRate = out_sample_rate.into();
        config.format = format.into();
        config
    }

    pub fn new_u8(out_channels: u32, out_sample_rate: SampleRate) -> CustomDecoderBuilder<u8> {
        let inner = Self::new_inner(out_channels, out_sample_rate, Format::U8);
        CustomDecoderBuilder {
            inner,
            vtables: Vec::new(),
            sample_rate: out_sample_rate,
            channels: out_channels,
            format: Format::U8,
            channel_map: Vec::new(),
            // user_data: None,
            _format: PhantomData,
        }
    }

    pub fn new_i16(out_channels: u32, out_sample_rate: SampleRate) -> CustomDecoderBuilder<i16> {
        let inner = Self::new_inner(out_channels, out_sample_rate, Format::S16);
        CustomDecoderBuilder {
            inner,
            vtables: Vec::new(),
            sample_rate: out_sample_rate,
            channels: out_channels,
            format: Format::S16,
            channel_map: Vec::new(),
            // user_data: None,
            _format: PhantomData,
        }
    }

    pub fn new_i32(out_channels: u32, out_sample_rate: SampleRate) -> CustomDecoderBuilder<i32> {
        let inner = Self::new_inner(out_channels, out_sample_rate, Format::S32);
        CustomDecoderBuilder {
            inner,
            vtables: Vec::new(),
            sample_rate: out_sample_rate,
            channels: out_channels,
            format: Format::S32,
            channel_map: Vec::new(),
            // user_data: None,
            _format: PhantomData,
        }
    }

    pub fn new_s24_packed(
        out_channels: u32,
        out_sample_rate: SampleRate,
    ) -> CustomDecoderBuilder<S24Packed> {
        let inner = Self::new_inner(out_channels, out_sample_rate, Format::S24Packed);
        CustomDecoderBuilder {
            inner,
            vtables: Vec::new(),
            sample_rate: out_sample_rate,
            channels: out_channels,
            format: Format::S24Packed,
            channel_map: Vec::new(),
            // user_data: None,
            _format: PhantomData,
        }
    }

    pub fn new_f32(out_channels: u32, out_sample_rate: SampleRate) -> CustomDecoderBuilder<f32> {
        let inner = Self::new_inner(out_channels, out_sample_rate, Format::F32);
        CustomDecoderBuilder {
            inner,
            vtables: Vec::new(),
            sample_rate: out_sample_rate,
            channels: out_channels,
            format: Format::F32,
            channel_map: Vec::new(),
            // user_data: None,
            _format: PhantomData,
        }
    }
}

impl<F: PcmFormat> CustomDecoderBuilder<F> {
    fn set_backend_registration(
        &mut self,
        channel_map: Vec<Channel>,
    ) -> *mut BackendRegistration<F> {
        let registration: BackendRegistration<F> = BackendRegistration {
            channels: self.channels,
            sample_rate: self.sample_rate,
            format: self.format,
            channel_map,
            _format: PhantomData,
        };
        let ptr = Box::into_raw(Box::new(registration));
        self.inner.pCustomBackendUserData = ptr.cast();
        ptr
    }

    /// Set the channel map of the output.
    /// This map may differ from the input channel map, as the decoder implements a channel converter.
    ///
    /// Also sets the channel count to the length of the channel map
    /// If not set, miniaudio will generate a standard channel map for the channel count.
    pub fn set_out_channel_map<I>(mut self, channel_map: I) -> Self
    where
        I: IntoIterator<Item = Channel>,
    {
        self.channel_map = channel_map.into_iter().map(RawChannel::from).collect();
        self.channels = self.channel_map.len() as u32;
        self
    }

    fn set_backend_vtables(&mut self) -> MaResult<Box<[*const sys::ma_decoding_backend_vtable]>> {
        if self.vtables.is_empty() {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "At least one backend mut be provided.",
            )));
        }
        self.inner.customBackendCount = self.vtables.len() as u32;

        // Each backend is converted to a vtable and stored in a vec
        // Drain that vec and store the vtables into a Box with a stable address
        let mut vtables: Box<[*const sys::ma_decoding_backend_vtable]> =
            self.vtables.drain(..).collect();
        self.inner.ppCustomBackendVTables =
            vtables.as_mut_ptr() as *mut *mut sys::ma_decoding_backend_vtable;

        Ok(vtables)
    }

    fn set_channel_map(&mut self) -> MaResult<Vec<Channel>> {
        if self.channels == 0 {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Channel count cannot be 0",
            )));
        }
        if !self.channel_map.is_empty() && self.channels as usize != self.channel_map.len() {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Channel map length must match the channel count",
            )));
        }

        if self.channel_map.is_empty() {
            self.channel_map
                .resize_with(self.channels as usize, || RawChannel::from_raw(0));
            unsafe {
                sys::ma_channel_map_init_standard(
                    sys::ma_standard_channel_map_ma_standard_channel_map_default,
                    self.channel_map.as_mut_ptr().cast(),
                    self.channels as usize,
                    self.channels,
                );
            };
        };
        let mut map: Vec<Channel> = Vec::with_capacity(self.channel_map.len());
        for &entry in self.channel_map.iter() {
            map.push(Channel::try_from(entry)?);
        }
        self.inner.pChannelMap = self.channel_map.as_mut_ptr() as *mut _;
        Ok(map)
    }

    // pub fn output_channels(&mut self, channels: u32) -> &mut Self {
    //     self.channels = Some(channels);
    //     self
    // }

    // pub fn output_sample_rate(&mut self, channels: u32) -> &mut Self {
    //     self.channels = Some(channels);
    //     self
    // }

    /// Add a decoding backend to the `CustomDecoder`.
    ///
    /// Can be called multiple times to add multipe backends.
    pub fn backend<B: DecodingBackend<Format = F>>(&mut self) -> &mut Self {
        let vtable = decoder_vtable::<F, B>();
        self.vtables.push(vtable);
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
    pub fn from_memory<'a>(&mut self, data: &'a [u8]) -> MaResult<CustomDecoder<F, Borrowed<'a>>> {
        CustomDecoder::<F, Borrowed<'a>>::init_from_memory(data, self)
    }

    /// Creates a decoder from owned in-memory audio data.
    ///
    /// This is the same as from_memory, but stores an owned copy of the
    /// encoded data inside the returned decoder.
    ///
    /// Use this when you want the decoder to own backing memory
    /// instead of borrowing it from the caller.
    pub fn copy_memory<D: Into<Arc<[u8]>>>(
        &mut self,
        data: D,
    ) -> MaResult<CustomDecoder<F, Owned>> {
        CustomDecoder::<F, Owned>::init_copy(data, self)
    }

    /// Creates a decoder from a file path.
    ///
    /// The file is opened and managed through miniaudio's file-based decoding
    /// path rather than by storing the file contents in Rust memory first.
    ///
    /// This is usually the most convenient option when decoding from a normal
    /// file on disk.
    pub fn from_file(&mut self, path: &Path) -> MaResult<CustomDecoder<F, Fs>> {
        CustomDecoder::<F, Fs>::init_file(path, self)
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
    pub fn from_reader<R: SeekRead + Send + Sync>(
        &mut self,
        reader: R,
    ) -> MaResult<CustomDecoder<F, Cb>> {
        CustomDecoder::<F, Cb>::init_from_reader(reader, self)
    }
}

impl<F: PcmFormat, S> Drop for CustomDecoder<F, S> {
    fn drop(&mut self) {
        let _ = decoder_ffi::ma_decoder_uninit(self);
        for vtable in self._decoder_vtables.iter() {
            drop(unsafe { Box::from_raw(*vtable as *mut sys::ma_decoding_backend_vtable) });
        }
        if let Some((ptr, destructor)) = self.user_data {
            destructor(ptr);
        }
        drop(unsafe { Box::from_raw(self.backend_reg) });
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

#[cfg(test)]
mod test {
    use crate::{
        audio::sample_rate::SampleRate,
        data_source::{
            pcm_source::PcmSource,
            sources::decoder::{
                decoding_backend::{DecodingBackend, DrBackendContext},
                Cb, Decoder, DecoderBuilder, DecoderOps,
            },
            DataFormat,
        },
        MaResult,
    };
    use crate::{
        data_source::sources::decoder::decoding_backend::DecoderStream,
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

    struct TestCbDecoder(Decoder<f32, Cb>);

    impl DecodingBackend for TestCbDecoder {
        type Format = f32;

        type Decoder<'stream>
            = TestCbDecoder
        where
            Self: 'stream;

        fn init_decoder<'stream>(
            stream: DecoderStream<'stream>,
            _ctx: DrBackendContext,
        ) -> MaResult<Self::Decoder<'stream>> {
            let decoder = DecoderBuilder::new_f32(1, SampleRate::Sr48000).from_reader(stream)?;
            Ok(TestCbDecoder(decoder))
        }

        fn input_data_format<'stream>(
            _ds: &Self::Decoder<'stream>,
        ) -> crate::data_source::DataFormat {
            DataFormat {
                format: Format::F32,
                channels: 1,
                sample_rate: SampleRate::Sr48000,
                channel_map: Vec::new(),
            }
        }
    }

    impl PcmSource<f32> for TestCbDecoder {
        fn fill_pcm_frames(
            &mut self,
            out: &mut [f32],
            ctx: &mut crate::data_source::SourceContext,
        ) -> MaResult<usize> {
            let frames = self.0.read_pcm_frames_into(out).unwrap_or(0);
            ctx.cursor += frames as u64;

            Ok(frames)
        }

        fn seek_to_pcm_frame(
            &mut self,
            frame_index: u64,
            ctx: &mut crate::data_source::SourceContext,
        ) -> MaResult<()> {
            if DecoderOps::seek_to_pcm_frame(&mut self.0, frame_index).is_ok() {
                ctx.cursor = frame_index
            }
            Ok(())
        }

        fn cursor_in_pcm_frames(&self, ctx: &crate::data_source::SourceContext) -> MaResult<u64> {
            Ok(ctx.cursor)
        }

        fn length_in_pcm_frames(&self, _ctx: &crate::data_source::SourceContext) -> MaResult<u64> {
            self.0.length_pcm()
        }
    }

    #[test]
    fn test_custom_decoder_from_memory_f32_get_input_data_format() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let mut builder = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000);

        let dec = builder.backend::<TestCbDecoder>().copy_memory(wav).unwrap();

        let res = dec.backend_data_format();
        assert!(res.is_ok());
        let df = res.unwrap();
        assert_eq!(df.format, Format::F32);
        assert_eq!(df.sample_rate, SampleRate::Sr48000);
        assert_eq!(df.channels, 1);
        assert_eq!(df.channel_map, [Channel::Mono]);
    }

    #[test]
    fn test_custom_decoder_from_memory_f32_read_seek_cursor_length_available() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let mut builder = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000);

        let mut dec = builder.backend::<TestCbDecoder>().copy_memory(wav).unwrap();

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
    fn test_custom_decoder_ref_from_memory_decodes() {
        let frames_total: usize = 32;
        let wav = tiny_test_wav_mono(frames_total);

        let mut builder = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000);

        let mut dec_ref = builder
            .backend::<TestCbDecoder>()
            .from_memory(&wav)
            .unwrap();

        let buf = dec_ref.read_pcm_frames(12).unwrap();
        let read = buf.frames();
        assert_eq!(read, 12);
        assert_eq!(buf.len(), 12);
    }

    #[test]
    fn test_custom_decoder_from_file_reads_and_reports_length() {
        let frames_total: usize = 60;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let mut builder = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000);

        let mut dec = builder
            .backend::<TestCbDecoder>()
            .from_file(guard.path())
            .unwrap();

        assert_eq!(dec.length_pcm().unwrap() as usize, frames_total);
        assert_eq!(dec.cursor_pcm().unwrap(), 0);

        let buf = dec.read_pcm_frames(1000).unwrap();
        assert_eq!(buf.frames(), frames_total);
    }

    #[test]
    fn test_custom_decoder_read_f32_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);

        let mut dec = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000)
            .backend::<TestCbDecoder>()
            .from_memory(&wav)
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_custom_decoder_read_f32_copy_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);
        let wav: Arc<[u8]> = wav.into();

        let mut dec = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000)
            .backend::<TestCbDecoder>()
            .copy_memory(wav.clone())
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_custom_decoder_read_f32_path() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let mut dec = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000)
            .backend::<TestCbDecoder>()
            .from_file(guard.path())
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_custom_decoder_read_f32_file() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();
        let file = std::fs::File::open(guard.path()).unwrap();

        let mut dec = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000)
            .backend::<TestCbDecoder>()
            .from_reader(file)
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }
}
