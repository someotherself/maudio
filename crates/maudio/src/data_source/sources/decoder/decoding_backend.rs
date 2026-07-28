//! Integration point for a third-party decoding backend.
use std::io::{Read, Seek};
use std::{
    fs::File,
    io::{Cursor, SeekFrom},
};

use crate::audio::channels::Channel;
use crate::audio::sample_rate::SampleRate;
use crate::data_source::DataFormat;
use crate::{data_source::pcm_source::PcmSource, pcm_frames::PcmFormat, MaResult};

use maudio_sys::ffi as sys;

/// Implement this trait for a backend type that initializes a decoder from an
/// encoded [`DecoderStream`]. The initialized decoder must implement
/// [`PcmSource`] so that miniaudio can read decoded PCM frames from it.
///
/// # Backend selection
///
/// Add backends to a [`CustomDecoderBuilder`](crate::data_source) by
/// calling [`CustomDecoderBuilder::backend`](crate::data_source).
/// When more than one backend is registered, miniaudio tries them in the order they were added.
///
/// Returning an error from [`Self::init_decoder`] rejects the input for that
/// backend. Miniaudio then tries the next backend. If none were able to be initialized,
/// miniaudio will also try the built in decoding backend.
///
/// This fallback only applies during initialization. An error returned later
/// while reading or seeking is reported as a decoding error; it does not cause
/// miniaudio to select another backend.
///
/// # PCM format
///
/// [`Self::input_data_format`] must describe the PCM frames produced by the
/// initialized decoder. This is the input format used by miniaudio for sample
/// conversion, channel conversion, and resampling.
///
/// The reported format describes decoded PCM output, not the format of the
/// encoded input or the output format requested from `CustomDecoderBuilder`.
pub trait DecodingBackend: Send {
    /// Sample type produced by the backend decoder.
    ///
    /// This must agree with the sample format reported by [`Self::input_data_format`].
    type Format: PcmFormat;

    /// Decoder initialized for a particular input stream.
    ///
    /// The decoder may borrow from the supplied [`DecoderStream`] and therefore
    /// cannot outlive that stream.
    type Decoder<'stream>: PcmSource<Self::Format> + 'stream
    where
        Self: 'stream;

    /// Initializes the backend decoder from an encoded input stream.
    ///
    /// `ctx` provides context about the miniaudio decoder being initialized.
    ///
    /// Returning an error causes miniaudio to try the next registered backend.
    fn init_decoder<'stream>(
        stream: DecoderStream<'stream>,
        ctx: DrBackendContext,
    ) -> MaResult<Self::Decoder<'stream>>;

    /// Returns the native PCM format produced by `Decoder`.
    ///
    /// The sample format, channel count, sample rate, and channel map must match
    /// the frames written by [`PcmSource::fill_pcm_frames`].
    ///
    /// Miniaudio uses this information to convert the backend's PCM output to
    /// the output format requested from `CustomDecoderBuilder`.
    ///
    /// If the channel map is left empty, miniaudio will generate a default
    /// channel map for the channel count.
    fn input_data_format<'stream>(ds: &Self::Decoder<'stream>) -> DataFormat;
}

/// Context information used by [`DecodingBackend::init_decoder`]
///
/// The output_format field reprerents the data format added to the `CustomDecoderBuilder`
pub struct DrBackendContext {
    pub output_channels: Option<u32>,
    pub output_sample_rate: Option<SampleRate>,
    pub channel_map: Vec<Channel>,
}

/// Encoded input stream passed to a custom decoding backend.
///
/// A [`DecodingBackend`] reads the encoded audio data from this stream while
/// initializing and operating its decoder. The stream may be retained by the
/// decoder returned from [`DecodingBackend::init_decoder`], but it cannot outlive
/// the original decoder input.
///
/// `DecoderStream` implements [`Read`] and [`Seek`] by forwarding operations to
/// the underlying input stream.
///
/// Not every input source supports seeking. Use
/// [`DecoderStreamImpl::is_seekable`] to determine whether seek operations are
/// expected to succeed.
pub struct DecoderStream<'stream>(pub(crate) Box<dyn DecoderStreamImpl + 'stream>);

/// Interface for an encoded decoder input stream.
///
/// This trait extends [`Read`] and [`Seek`] with convenience methods commonly
/// required by third-party decoding libraries.
///
/// All lengths reported by this trait are measured in encoded bytes, not decoded
/// PCM frames.
pub trait DecoderStreamImpl: Read + Seek + Send + Sync {
    /// Returns whether the underlying input supports seeking.
    fn is_seekable(&self) -> bool;

    /// Returns the total length of the encoded input in bytes, if known.
    ///
    /// Returns `None` when the length cannot be determined.
    fn byte_len(&self) -> Option<u64>;
}

impl<'stream> DecoderStreamImpl for DecoderStream<'stream> {
    fn is_seekable(&self) -> bool {
        self.0.is_seekable()
    }

    fn byte_len(&self) -> Option<u64> {
        self.0.byte_len()
    }
}

impl<'stream> std::io::Read for DecoderStream<'stream> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl<'stream> std::io::Seek for DecoderStream<'stream> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

pub(crate) struct ReaderStream {
    pub(crate) on_read: sys::ma_read_proc,
    pub(crate) on_seek: sys::ma_seek_proc,
    pub(crate) on_tell: sys::ma_tell_proc,
    pub(crate) stream_user_data: *mut core::ffi::c_void,
    // TODO: on_tell is optional. Use this when it is missing.
    #[allow(unused)]
    pub(crate) cursor: u64,
}

unsafe impl Send for ReaderStream {}
unsafe impl Sync for ReaderStream {}

impl DecoderStreamImpl for ReaderStream {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

impl std::io::Read for ReaderStream {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let on_read = self.on_read.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "decoder stream has no read callback",
            )
        })?;
        let mut bytes_read = 0;

        let result = unsafe {
            on_read(
                self.stream_user_data,
                output.as_mut_ptr().cast(),
                output.len(),
                &mut bytes_read,
            )
        };

        match result {
            sys::ma_result_MA_SUCCESS => Ok(bytes_read),
            sys::ma_result_MA_AT_END => Ok(bytes_read),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "miniaudio stream read failed",
            )),
        }
    }
}

impl std::io::Seek for ReaderStream {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let on_seek = self.on_seek.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "decoder stream has no seek callback",
            )
        })?;

        // Convert io SeekFrom into ma_seek_origin and the offset for it
        let (offset, origin) = match pos {
            SeekFrom::Start(offset) => {
                let offset = i64::try_from(offset).map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek offset overflow")
                })?;
                (offset, sys::ma_seek_origin_ma_seek_origin_start)
            }
            SeekFrom::Current(offset) => (offset, sys::ma_seek_origin_ma_seek_origin_current),
            SeekFrom::End(offset) => (offset, sys::ma_seek_origin_ma_seek_origin_end),
        };

        // Seek to that offset (run fseek)
        let result = unsafe { on_seek(self.stream_user_data, offset, origin) };

        if result != sys::ma_result_MA_SUCCESS {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "miniaudio stream seek failed",
            ));
        }

        // TODO: onTell should be an optional parameter
        // Run ftell to confirm confirm the cursor position.
        // fseek only returns sucess or fail.

        let on_tell = self.on_tell.ok_or_else(|| {
            println!("on_tell called and failed");
            std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "decoder stream has no callback for cursor position",
            )
        })?;

        let mut cursor = 0i64;
        let result = unsafe { on_tell(self.stream_user_data, &mut cursor) };

        if result != sys::ma_result_MA_SUCCESS {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "miniaudio stream tell failed",
            ));
        }

        println!("seek ran correctly: {cursor}");
        u64::try_from(cursor).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "miniaudio returned a negative stream position",
            )
        })
    }
}

pub(crate) struct DecoderFileStream(pub(crate) File);

impl DecoderStreamImpl for DecoderFileStream {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.0.metadata().ok().map(|m| m.len())
    }
}

impl std::io::Read for DecoderFileStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl std::io::Seek for DecoderFileStream {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

pub(crate) struct DecoderByteStream<'a>(pub(crate) Cursor<&'a [u8]>);

impl<'a> DecoderStreamImpl for DecoderByteStream<'a> {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.0.get_ref().len() as u64)
    }
}

impl<'a> std::io::Read for DecoderByteStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl<'a> std::io::Seek for DecoderByteStream<'a> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}
