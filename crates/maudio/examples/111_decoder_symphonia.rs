use std::{io::ErrorKind, path::PathBuf};

use maudio::{
    audio::formats::Format,
    data_source::{
        pcm_source::PcmSource,
        sources::decoder::decoding_backend::{
            DecoderStream, DecoderStreamImpl, DecodingBackend, DrBackendContext,
        },
        DataFormat, SourceContext,
    },
    engine::{engine_builder::EngineBuilder, resource::rm_builder::ResourceManagerBuilder},
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

// This example uses a decoder implementation identical to `203_decoder_symphonia`
// See that example file for more information about how to create a custom decoder.
//
// Instead, this example uses that implementation in the high level API,
// by adding the custom decoder to a resource manager.
//
// In the Engine, the Resource Manager is responsible for decoding sounds
// added using the `Sound` or by loading them into the resource manager directly.
//
// Adding a custom decoding backend to a Resource Manager makes it the default
// decoder for the Engine (or multiple Engines) extending the list supported formats.

fn main() -> MaResult<()> {
    // Replace this with your example file
    let path = PathBuf::from("crates/maudio/examples/template_aac_1.aac");

    let rm = ResourceManagerBuilder::new_f32()
        .decoding_backend::<SymphoniaBackend>()
        .build()?;

    let engine = EngineBuilder::new().resource_manager(&rm).build()?;

    let sound = engine.new_sound_from_file(&path)?;

    sound.play_sound()?;

    std::thread::sleep(std::time::Duration::from_secs(4));
    Ok(())
}

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
    type NativeFormat = f32;
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
