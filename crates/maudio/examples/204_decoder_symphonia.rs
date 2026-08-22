use std::{io::ErrorKind, path::PathBuf};

use maudio::{
    audio::formats::Format,
    data_source::{
        pcm_source::PcmSource,
        sources::decoder::{
            custom_decoder::CustomDecoderBuilder,
            decoding_backend::{
                DecoderStream, DecoderStreamImpl, DecodingBackend, DrBackendContext,
            },
            DecoderOps,
        },
        DataFormat, SourceContext,
    },
    device::device_builder::DeviceBuilder,
    ErrorKinds, MaResult, MaudioError,
};

use symphonia::{
    core::{
        codecs::audio::{AudioDecoder, AudioDecoderOptions},
        errors::Error as SymphoniaError,
        formats::{probe::Hint, FormatOptions, FormatReader, SeekMode, SeekTo, TrackType},
        io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions},
        meta::MetadataOptions,
        units::Timestamp,
    },
    default::{get_codecs, get_probe},
};

// # Implementing a custom decoder with Symphonia
//
// This example integrates `Symphonia` as a custom decoding backend for
// `CustomDecoderBuilder`.
//
// Symphonia handles the container and codec-specific work: probing the encoded
// stream, selecting an audio track, reading packets, and decoding those packets
// into PCM samples. The custom backend adapts that decoded audio to Maudio's
// `PcmSource` interface. Miniaudio can then consume it like any other decoder
// and, when necessary, convert it to the output format requested by the
// application.
//
// The integration consists of the following steps:
//
// 1. **Adapt Maudio's input stream to Symphonia.**
//
// `DecodingBackend::init_decoder` receives a `DecoderStream` containing
// the encoded input.
// `DecoderStream` is Read + Seek + Send + Sync + 'stream, which satifies part
// Symphonia's expected type implementing `MediaSource`.
//
// We'll wrap the `DecoderStream` into another type called `SymponiaSource`
// so we can implement the `MediaSource` trait
//
// 2. **Probe the stream and create the decoder type**
//
// During initialization, the decoder should confirm that it
// recognizes input, and be able to pass its data format to maudio.
// In this example, we use symphonia to probe the encoded data.
//
// At this point, Maudio only expects the decoder to be initialized,
// and does not expects any decoded frames yet.
// The decoder can either decode the entire source at this point, which increases
// latency, or decode frames are they are requested, in streaming mode.
//
// The exact initialization and probing 3rd party library specific.
//
// This example produces interleaved `f32` samples. That choice determines
// both the backend's `DecodingBackend::Format` type and the
// `PcmSource<f32>` implementation.
//
// Once the initialization is done, the decoder can pass the data format of the
// decoded data to maudio using `DecodingBackend::input_data_format`.
// This wil let miniaudio know if it needs to do any resampling or channel conversion.
//
// 3. **Decode and buffer packets.**
//
// Symphonia decodes one packet at a time, while miniaudio requests an
// arbitrary number of PCM frames at a time. A decoded packet may therefore
// be smaller or larger than the buffer supplied to [`PcmSource`].
//
// `SymphoniaDecoder` stores each the decoded packets in `packet_samples`.
//
// 4. **Expose decoded frames through `PcmSource`.**
//
// The decoder's output must be compatible with miniaudio's DataSource interface
// and bridging that is done through the `PcmSource` trait.
//
// [`PcmSource::fill_pcm_frames`] copies samples from the packet buffer into
// miniaudio's output buffer. It decodes additional packets as necessary until
// the requested buffer is full or the encoded stream reaches its end.
//
// Because `PcmSource` works in PCM frames rather than individual samples, the
// number of copied samples is divided by the channel count before being
// returned. The source cursor is advanced by the same number of frames.
//
// 5. **Build and play the custom decoder.**
//
// The application registers `SymphoniaBackend` with
// [`CustomDecoderBuilder::backend`] and requests its desired output channel
// count and sample rate. If those values differ from Symphonia's native PCM
// output, miniaudio performs the necessary channel conversion and resampling.
//
// Finally, the playback callback reads PCM frames from the resulting custom
// decoder into the device's output buffer.
//
// [Symphonia]: https://crates.io/crates/symphonia

fn symphonia_error(error: impl ToString) -> MaudioError {
    MaudioError::new_ma_error(ErrorKinds::Other(error.to_string()))
}

pub struct SymphoniaBackend;

pub struct SymphoniaDecoder<'stream> {
    format: Box<dyn FormatReader + 'stream>,
    decoder: Box<dyn AudioDecoder>,

    len_frames: u64,
    track_id: u32,
    channels: usize,
    sample_rate: u32,

    packet_samples: Vec<f32>,
    packet_offset: usize,
    discard_frames: u64,
}

struct SymponiaSource<'stream>(DecoderStream<'stream>);

// std::io::Read and std::io::Seek need to be re-implemented for our wrapper type
impl<'stream> std::io::Read for SymponiaSource<'stream> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl<'stream> std::io::Seek for SymponiaSource<'stream> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

// From the DecoderStreamImpl trait ,`DecoderStream` also has convenience
// methods to help bridge it with 3rd party libraries. In this case,
// it gives us `is_seekable` and `byte_len`.
// The output of these 2 methods matches the input type in the `CustomDecoderBuilder`
// Example: from_reader gives a source that is seekable, but does not have a length.
impl<'stream> MediaSource for SymponiaSource<'stream> {
    fn is_seekable(&self) -> bool {
        self.0.is_seekable()
    }

    fn byte_len(&self) -> Option<u64> {
        self.0.byte_len()
    }
}

impl<'stream> SymphoniaDecoder<'stream> {
    fn new(stream: DecoderStream<'stream>) -> MaResult<SymphoniaDecoder<'stream>> {
        let media_stream = MediaSourceStream::new(
            Box::new(SymponiaSource(stream)),
            MediaSourceStreamOptions::default(),
        );

        let hint = Hint::new();

        let format = get_probe()
            .probe(
                &hint,
                media_stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(symphonia_error)?;

        let (track_id, codec_params, channels, sample_rate, len_frames) = {
            let track = format
                .default_track(TrackType::Audio)
                .ok_or_else(|| symphonia_error("the stream contains no supported audio track"))?;

            let codec_params = track
                .codec_params
                .as_ref()
                .and_then(|params| params.audio())
                .ok_or_else(|| symphonia_error("the selected track has no audio codec parameters"))?
                .clone();

            let channels = codec_params
                .channels
                .as_ref()
                .ok_or_else(|| symphonia_error("the channel count is unknown"))?
                .count();

            let sample_rate = codec_params
                .sample_rate
                .ok_or_else(|| symphonia_error("the sample rate is unknown"))?;

            (
                track.id,
                codec_params,
                channels,
                sample_rate,
                track.num_frames,
            )
        };

        let decoder = get_codecs()
            .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
            .map_err(symphonia_error)?;

        Ok(SymphoniaDecoder {
            format,
            decoder,
            len_frames: len_frames.unwrap(), // we are guaranteed to have a length here
            track_id,
            channels,
            sample_rate,
            packet_samples: Vec::new(),
            packet_offset: 0,
            discard_frames: 0,
        })
    }

    /// Decodes another packet belonging to the selected audio track.
    ///
    /// Returns `false` at the end of the stream.
    fn decode_next_packet(&mut self) -> MaResult<bool> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => {
                    // We reached the end
                    self.packet_samples.clear();
                    self.packet_offset = 0;
                    return Ok(false);
                }
                Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                    self.packet_samples.clear();
                    self.packet_offset = 0;
                    return Ok(false);
                }
                Err(error) => return Err(symphonia_error(error)),
            };

            if packet.track_id != self.track_id {
                continue;
            }

            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(SymphoniaError::DecodeError(_)) | Err(SymphoniaError::IoError(_)) => continue,
                Err(error) => return Err(symphonia_error(error)),
            };

            let spec = decoded.spec();

            if spec.channels().count() != self.channels {
                return Err(symphonia_error("the channel count changed while decoding"));
            }

            if spec.rate() != self.sample_rate {
                return Err(symphonia_error("the sample rate changed while decoding"));
            }

            self.packet_samples
                .resize(decoded.samples_interleaved(), 0.0);

            decoded.copy_to_slice_interleaved(&mut self.packet_samples);

            self.packet_offset = 0;

            if self.discard_frames != 0 {
                let packet_frames = self.packet_samples.len() / self.channels;

                let discarded = self.discard_frames.min(packet_frames as u64);

                self.packet_offset = discarded as usize * self.channels;

                self.discard_frames -= discarded;

                if self.packet_offset == self.packet_samples.len() {
                    continue;
                }
            }

            return Ok(true);
        }
    }
}

impl<'stream> PcmSource<f32> for SymphoniaDecoder<'stream> {
    fn fill_pcm_frames(&mut self, out: &mut [f32], ctx: &mut SourceContext) -> MaResult<usize> {
        if out.len() % self.channels != 0 {
            return Err(symphonia_error(
                "the output buffer does not contain a whole number of frames",
            ));
        }

        let mut samples_written = 0;

        while samples_written < out.len() {
            if self.packet_offset == self.packet_samples.len() && !self.decode_next_packet()? {
                break;
            }

            let available = self.packet_samples.len() - self.packet_offset;
            let requested = out.len() - samples_written;
            let to_copy = std::cmp::min(available, requested);

            out[samples_written..samples_written + to_copy].copy_from_slice(
                &self.packet_samples[self.packet_offset..self.packet_offset + to_copy],
            );

            self.packet_offset += to_copy;
            samples_written += to_copy;
        }

        let frames_written = samples_written / self.channels;
        ctx.cursor += frames_written as u64;

        Ok(frames_written)
    }

    fn seek_to_pcm_frame(&mut self, frame_index: u64, ctx: &mut SourceContext) -> MaResult<()> {
        let frame_index = frame_index.min(self.len_frames);
        let track_id = self.track_id;
        let Ok(seeked) = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Timestamp {
                ts: Timestamp::new(frame_index as i64),
                track_id,
            },
        ) else {
            return Ok(());
        };

        self.decoder.reset();

        self.packet_samples.clear();
        self.packet_offset = 0;

        // Determine how far the actual seek landed before frame_index.
        self.discard_frames = ((frame_index as i64) - seeked.actual_ts.get()) as u64;
        ctx.cursor = frame_index;
        Ok(())
    }

    fn cursor_in_pcm_frames(&self, ctx: &SourceContext) -> MaResult<u64> {
        Ok(ctx.cursor)
    }

    fn length_in_pcm_frames(&self, _ctx: &SourceContext) -> MaResult<u64> {
        Ok(self.len_frames)
    }
}

impl DecodingBackend for SymphoniaBackend {
    type Format = f32;
    type Decoder<'stream>
        = SymphoniaDecoder<'stream>
    where
        Self: 'stream;

    fn init_decoder<'stream>(
        stream: DecoderStream<'stream>,
        _ctx: DrBackendContext,
    ) -> MaResult<Self::Decoder<'stream>> {
        SymphoniaDecoder::new(stream)
    }

    fn input_data_format<'stream>(ds: &Self::Decoder<'stream>) -> maudio::data_source::DataFormat {
        DataFormat {
            format: Format::F32,
            channels: ds.channels as u32,
            sample_rate: ds.sample_rate.try_into().unwrap(),
            channel_map: Vec::new(),
        }
    }
}

fn main() -> MaResult<()> {
    // use maudio::audio::sample_rate::SampleRate;

    // Replace this with your example file
    let path = PathBuf::from("crates/maudio/examples/template_aac_1.aac");

    // let channels = 2;
    // let sample_rate = SampleRate::Sr44100;
    let mut decoder = CustomDecoderBuilder::new_f32()
        // .output_channels(channels)
        // .output_sample_rate(sample_rate)
        .backend::<SymphoniaBackend>()
        .from_file(&path)?;

    let mut device = DeviceBuilder::playback()
        .f32()
        .with_callback(move |_, out| {
            decoder.read_pcm_frames_into(out).unwrap();
        })?;

    device.device_start()?;
    std::thread::sleep(std::time::Duration::from_secs(4));
    Ok(())
}
