//! Audio file encoding.
use std::{marker::PhantomData, mem::MaybeUninit, path::Path, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    data_source::sources::decoder::{Cb, Fs},
    device::device_builder::Unknown,
    engine::AllocationCallbacks,
    pcm_frames::{PcmFormat, S24Packed, S24},
    AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
};

/// Writes PCM audio frames into an encoded output destination.
///
/// An `Encoder` accepts interleaved PCM frames in the format specified by `F`,
/// encodes them as `E`, and writes the resulting bytes to the destination type `D`.
///
/// # What an encoder does
///
/// An encoder sits between raw PCM audio and some encoded output destination:
///
/// - you provide PCM frames with [`Encoder::write_pcm_frames`]
/// - the encoder converts them into the selected encoded format
/// - the encoded bytes are written to the file or writer chosen during construction
///
/// # Input and output
///
/// The encoder's **destination** is chosen once at build time:
///
/// - [`EncoderBuilder::build_file`] creates an encoder that writes to a file path
/// - [`EncoderBuilder::build_writer`] creates an encoder that writes through custom callbacks
///
/// After construction, audio data can be supplied manually through
/// [`Encoder::write_pcm_frames`].
///
/// # PCM requirements
///
/// The written PCM data must already match the encoder's configured:
///
/// - sample format
/// - channel count
/// - sample rate
///
/// No automatic conversion is performed by the encoder.
/// If these are not supported by the output file type an error will be returned.
///
/// # Examples
///
/// Writing to a file:
///
/// ```no_run
/// # use std::path::PathBuf;
/// # use maudio::encoder::EncoderBuilder;
/// # use maudio::{MaResult, audio::sample_rate::SampleRate};
/// # fn main() -> MaResult<()> {
/// let path = PathBuf::from("out.flac");
///
/// let mut encoder = EncoderBuilder::new_f32(2, SampleRate::Sr44100)
///     .flac()
///     .build_file(&path)?;
///
/// let pcm = vec![0.0f32; 512 * 2];
/// encoder.write_pcm_frames(&pcm)?;
/// # Ok(())
/// # }
/// ```
///
/// Writing to a custom writer:
///
/// ```no_run
/// # use std::fs::File;
/// # use maudio::encoder::EncoderBuilder;
/// # use maudio::{MaResult, audio::sample_rate::SampleRate};
/// # fn main() -> MaResult<()> {
/// let file = File::create("out.wav").unwrap();
///
/// let mut encoder = EncoderBuilder::new_i16(2, SampleRate::Sr44100)
///     .wav()
///     .build_writer(file)?;
///
/// let pcm = vec![0i16; 256 * 2];
/// encoder.write_pcm_frames(&pcm)?;
/// # Ok(())
/// # }
/// ```
pub struct Encoder<F: PcmFormat, E: CodecFormat, D> {
    inner: *mut sys::ma_encoder,
    channels: u32,
    sample_rate: SampleRate,
    format: Format,
    _format: PhantomData<F>,
    _encoding: PhantomData<E>,
    _destination: PhantomData<D>,
}

impl<F: PcmFormat, E: CodecFormat, D> Binding for Encoder<F, E, D> {
    type Raw = *mut sys::ma_encoder;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat, E: CodecFormat, D> Encoder<F, E, D> {
    // TODO: Test if this works with F::PcmUnit (i32)
    pub fn write_pcm_frames(&mut self, source: &[F::StorageUnit]) -> MaResult<u64> {
        encoder_ffi::ma_encoder_write_pcm_frames(self, source)
    }
}

// Private methods
impl<F: PcmFormat, E: CodecFormat, D> Encoder<F, E, D> {
    fn new(
        inner: *mut sys::ma_encoder,
        channels: u32,
        sample_rate: SampleRate,
        format: Format,
    ) -> Self {
        Self {
            inner,
            channels,
            sample_rate,
            format,
            _format: PhantomData,
            _encoding: PhantomData,
            _destination: PhantomData,
        }
    }

    fn init_from_file(config: &EncoderBuilder<F, E>, path: &Path) -> MaResult<Encoder<F, E, Fs>> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_encoder>> = Box::new(MaybeUninit::uninit());

        Encoder::<F, E, D>::init_from_file_internal(path, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_encoder = Box::into_raw(mem) as *mut sys::ma_encoder;
        Ok(Encoder::new(
            inner,
            config.channels,
            config.sample_rate,
            config.format,
        ))
    }

    fn init_from_writer<W: WriteSeek>(
        config: &EncoderBuilder<F, E>,
        writer: W,
    ) -> MaResult<Encoder<F, E, Cb>> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_encoder>> = Box::new(MaybeUninit::uninit());

        let user_data = Box::new(EncoderUserData { writer });
        encoder_ffi::ma_encoder_init(
            Some(encoder_write_proc::<W>),
            Some(encoder_seek_proc::<W>),
            Box::into_raw(user_data) as *mut _,
            config,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_encoder = Box::into_raw(mem) as *mut sys::ma_encoder;

        Ok(Encoder::new(
            inner,
            config.channels,
            config.sample_rate,
            config.format,
        ))
    }

    fn init_from_file_internal(
        path: &Path,
        config: &EncoderBuilder<F, E>,
        encoder: *mut sys::ma_encoder,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            use crate::engine::cstring_from_path;

            let path = cstring_from_path(path)?;
            encoder_ffi::ma_encoder_init_file(path, config, encoder)?;
            Ok(())
        }

        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated;

            let path = wide_null_terminated(path);

            encoder_ffi::ma_encoder_init_file_w(&path, config, encoder)?;
            Ok(())
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }
}

/// Trait alias for types that implement both [`std::io::Write`] and [`std::io::Seek`].
///
/// This is used by [`EncoderBuilder::build_writer`] to accept custom output
/// destinations for encoded audio data.
pub trait WriteSeek: std::io::Write + std::io::Seek {}
impl<T: std::io::Write + std::io::Seek> WriteSeek for T {}

struct EncoderUserData<W> {
    writer: W,
}

unsafe extern "C" fn encoder_write_proc<W: WriteSeek>(
    encoder: *mut sys::ma_encoder,
    buffer_in: *const core::ffi::c_void,
    bytes_to_write: usize,
    bytes_written: *mut usize,
) -> sys::ma_result {
    if encoder.is_null() || buffer_in.is_null() || bytes_written.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    // Make sure we don't leave this uninitialized
    *bytes_written = 0;

    let user_data = &mut *((&*encoder).pUserData as *mut EncoderUserData<W>);

    let slice = core::slice::from_raw_parts(buffer_in as _, bytes_to_write);

    match user_data.writer.write(slice) {
        Ok(n) => {
            *bytes_written = n;
            sys::ma_result_MA_SUCCESS
        }
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn encoder_seek_proc<W: WriteSeek>(
    encoder: *mut sys::ma_encoder,
    byte_offset: i64,
    origin: sys::ma_seek_origin,
) -> sys::ma_result {
    if encoder.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    let user_data = &mut *((&*encoder).pUserData as *mut EncoderUserData<W>);

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

    match user_data.writer.seek(pos) {
        Ok(_) => sys::ma_result_MA_SUCCESS,
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn encoder_seek_proc_no_op(
    encoder: *mut sys::ma_encoder,
    _byte_offset: i64,
    _origin: sys::ma_seek_origin,
) -> sys::ma_result {
    if encoder.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    sys::ma_result_MA_SUCCESS
}

mod encoder_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        encoder::{CodecFormat, Encoder, EncoderBuilder},
        pcm_frames::PcmFormat,
        AsRawRef, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_encoder_init<F: PcmFormat, E: CodecFormat>(
        on_write: sys::ma_encoder_write_proc,
        on_seek: sys::ma_encoder_seek_proc,
        user_data: *mut core::ffi::c_void,
        config: &EncoderBuilder<F, E>,
        encoder: *mut sys::ma_encoder,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_encoder_init(on_write, on_seek, user_data, config.as_raw_ptr(), encoder)
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_encoder_init_file<F: PcmFormat, E: CodecFormat>(
        path: std::ffi::CString,
        config: &EncoderBuilder<F, E>,
        encoder: *mut sys::ma_encoder,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_encoder_init_file(path.as_ptr(), config.as_raw_ptr(), encoder) };
        MaudioError::check(res)
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_encoder_init_file_w<F: PcmFormat, E: CodecFormat>(
        path: &[u16],
        config: &EncoderBuilder<F, E>,
        encoder: *mut sys::ma_encoder,
    ) -> MaResult<()> {
        let res =
            unsafe { sys::ma_encoder_init_file_w(path.as_ptr(), config.as_raw_ptr(), encoder) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_encoder_uninit<F: PcmFormat, E: CodecFormat, D>(encoder: &mut Encoder<F, E, D>) {
        unsafe {
            sys::ma_encoder_uninit(encoder.to_raw());
        };
    }

    pub fn ma_encoder_write_pcm_frames<F: PcmFormat, E: CodecFormat, D>(
        encoder: &mut Encoder<F, E, D>,
        source: &[F::StorageUnit],
    ) -> MaResult<u64> {
        if source.is_empty() {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        };
        ma_encoder_write_pcm_frames_internal(
            encoder,
            source.as_ptr() as *const core::ffi::c_void,
            source.len() as u64,
        )
    }

    #[inline]
    fn ma_encoder_write_pcm_frames_internal<F: PcmFormat, E: CodecFormat, D>(
        encoder: &mut Encoder<F, E, D>,
        frames_in: *const core::ffi::c_void,
        frame_count: u64,
    ) -> MaResult<u64> {
        let mut frames_written: u64 = 0;
        let res = unsafe {
            sys::ma_encoder_write_pcm_frames(
                encoder.to_raw(),
                frames_in,
                frame_count,
                &mut frames_written,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_written)
    }
}

impl<F: PcmFormat, E: CodecFormat, D> Drop for Encoder<F, E, D> {
    fn drop(&mut self) {
        encoder_ffi::ma_encoder_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

/// Encoding/container formats supported by [`Encoder`].
pub enum EncodingFormat {
    Wav,
    Flac,
    Mp3,
    Vorbis,
}

impl From<EncodingFormat> for sys::ma_encoding_format {
    fn from(value: EncodingFormat) -> Self {
        match value {
            EncodingFormat::Wav => sys::ma_encoding_format_ma_encoding_format_wav,
            EncodingFormat::Flac => sys::ma_encoding_format_ma_encoding_format_flac,
            EncodingFormat::Mp3 => sys::ma_encoding_format_ma_encoding_format_mp3,
            EncodingFormat::Vorbis => sys::ma_encoding_format_ma_encoding_format_vorbis,
        }
    }
}

impl TryFrom<sys::ma_encoding_format> for EncodingFormat {
    type Error = MaudioError;

    fn try_from(value: sys::ma_encoding_format) -> Result<Self, Self::Error> {
        match value {
            sys::ma_encoding_format_ma_encoding_format_wav => Ok(EncodingFormat::Wav),
            sys::ma_encoding_format_ma_encoding_format_flac => Ok(EncodingFormat::Flac),
            sys::ma_encoding_format_ma_encoding_format_mp3 => Ok(EncodingFormat::Mp3),
            sys::ma_encoding_format_ma_encoding_format_vorbis => Ok(EncodingFormat::Vorbis),
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                EncodingFormat,
            >(other as i64))),
        }
    }
}

/// Builder for creating an [`Encoder`].
///
/// `EncoderBuilder` uses a staged type-state API to guide encoder construction:
///
/// 1. choose the PCM sample format with a `new_*` constructor
/// 2. choose the encoding/container format with methods like [`Self::wav`] or [`Self::flac`]
/// 3. build the encoder for a destination with [`Self::build_file`] or [`Self::build_writer`]
///
pub struct EncoderBuilder<F = Unknown, E = Unknown> {
    inner: sys::ma_encoder_config,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    format: Format,
    channels: u32,
    sample_rate: SampleRate,
    _format: PhantomData<F>,
    _encoding: PhantomData<E>,
}

impl<F: PcmFormat, E: CodecFormat> AsRawRef for EncoderBuilder<F, E> {
    type Raw = sys::ma_encoder_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl EncoderBuilder<Unknown, Unknown> {
    pub fn new_inner(
        channels: u32,
        sample_rate: SampleRate,
        format: Format,
    ) -> sys::ma_encoder_config {
        // Format::U8 and encoding format unkwown are placeholders
        unsafe {
            sys::ma_encoder_config_init(
                sys::ma_encoding_format_ma_encoding_format_unknown,
                format.into(),
                channels,
                sample_rate.into(),
            )
        }
    }

    pub fn new_u8(channels: u32, sample_rate: SampleRate) -> EncoderBuilder<u8, Unknown> {
        let inner = EncoderBuilder::new_inner(channels, sample_rate, Format::U8);
        EncoderBuilder {
            inner,
            alloc_cb: None,
            format: Format::U8,
            channels,
            sample_rate,
            _format: PhantomData,
            _encoding: PhantomData,
        }
    }

    pub fn new_i16(channels: u32, sample_rate: SampleRate) -> EncoderBuilder<i16, Unknown> {
        let inner = EncoderBuilder::new_inner(channels, sample_rate, Format::S16);
        EncoderBuilder {
            inner,
            alloc_cb: None,
            format: Format::S16,
            channels,
            sample_rate,
            _format: PhantomData,
            _encoding: PhantomData,
        }
    }

    pub fn new_i32(channels: u32, sample_rate: SampleRate) -> EncoderBuilder<i32, Unknown> {
        let inner = EncoderBuilder::new_inner(channels, sample_rate, Format::S32);
        EncoderBuilder {
            inner,
            alloc_cb: None,
            format: Format::S32,
            channels,
            sample_rate,
            _format: PhantomData,
            _encoding: PhantomData,
        }
    }

    pub fn new_s24_packed(
        channels: u32,
        sample_rate: SampleRate,
    ) -> EncoderBuilder<S24Packed, Unknown> {
        let inner = EncoderBuilder::new_inner(channels, sample_rate, Format::S24Packed);
        EncoderBuilder {
            inner,
            alloc_cb: None,
            format: Format::S24Packed,
            channels,
            sample_rate,
            _format: PhantomData,
            _encoding: PhantomData,
        }
    }

    pub fn new_s24(channels: u32, sample_rate: SampleRate) -> EncoderBuilder<S24, Unknown> {
        let inner = EncoderBuilder::new_inner(channels, sample_rate, Format::S24Packed);
        EncoderBuilder {
            inner,
            alloc_cb: None,
            format: Format::S24Packed,
            channels,
            sample_rate,
            _format: PhantomData,
            _encoding: PhantomData,
        }
    }

    pub fn new_f32(channels: u32, sample_rate: SampleRate) -> EncoderBuilder<f32, Unknown> {
        let inner = EncoderBuilder::new_inner(channels, sample_rate, Format::F32);
        EncoderBuilder {
            inner,
            alloc_cb: None,
            format: Format::F32,
            channels,
            sample_rate,
            _format: PhantomData,
            _encoding: PhantomData,
        }
    }
}

pub struct Wav {}
pub struct Mp3 {}
pub struct Flac {}
pub struct Vorbis {}

/// Trait for the codec formats supporter by the `Encoder`
pub trait CodecFormat {}
impl CodecFormat for Wav {}
impl CodecFormat for Mp3 {}
impl CodecFormat for Flac {}
impl CodecFormat for Vorbis {}

impl<F: PcmFormat> EncoderBuilder<F, Unknown> {
    pub fn wav(mut self) -> EncoderBuilder<F, Wav> {
        self.inner.encodingFormat = EncodingFormat::Wav.into();
        EncoderBuilder {
            inner: self.inner,
            alloc_cb: self.alloc_cb,
            format: self.format,
            channels: self.channels,
            sample_rate: self.sample_rate,
            _format: PhantomData,
            _encoding: PhantomData,
        }
    }

    pub fn mp3(mut self) -> EncoderBuilder<F, Mp3> {
        self.inner.encodingFormat = EncodingFormat::Mp3.into();
        EncoderBuilder {
            inner: self.inner,
            alloc_cb: self.alloc_cb,
            format: self.format,
            channels: self.channels,
            sample_rate: self.sample_rate,
            _format: PhantomData,
            _encoding: PhantomData,
        }
    }

    pub fn flac(mut self) -> EncoderBuilder<F, Flac> {
        self.inner.encodingFormat = EncodingFormat::Flac.into();
        EncoderBuilder {
            inner: self.inner,
            alloc_cb: self.alloc_cb,
            format: self.format,
            channels: self.channels,
            sample_rate: self.sample_rate,
            _format: PhantomData,
            _encoding: PhantomData,
        }
    }

    pub fn vorbis(mut self) -> EncoderBuilder<F, Vorbis> {
        self.inner.encodingFormat = EncodingFormat::Vorbis.into();
        EncoderBuilder {
            inner: self.inner,
            alloc_cb: self.alloc_cb,
            format: self.format,
            channels: self.channels,
            sample_rate: self.sample_rate,
            _format: PhantomData,
            _encoding: PhantomData,
        }
    }
}

impl<F: PcmFormat, E: CodecFormat> EncoderBuilder<F, E> {
    pub fn build_file(&self, path: &Path) -> MaResult<Encoder<F, E, Fs>> {
        Encoder::<F, E, Fs>::init_from_file(self, path)
    }

    pub fn build_writer<W: WriteSeek>(&self, writer: W) -> MaResult<Encoder<F, E, Cb>> {
        Encoder::<F, E, Cb>::init_from_writer(self, writer)
    }
}
