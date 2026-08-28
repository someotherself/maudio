//! Builder for creating a [`ResourceManager`]
use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    data_source::sources::decoder::{
        custom_decoder::BackendRegistration, decoder_vtable::decoder_vtable,
        decoding_backend::DecodingBackend,
    },
    device::device_builder::Unknown,
    engine::resource::{rm_flags::RmFlags, ResourceManager},
    pcm_frames::{PcmFormat, S24Packed},
    AllocationCallbacks, AsRawRef, ErrorKinds, MaResult, MaudioError,
};

/// At the end, you will set the sample format that audio decoded by
/// this `ResourceManager` will be converted to.
/// The audio will decoded from its native sample format.
pub struct ResourceManagerBuilder<F = Unknown> {
    config: sys::ma_resource_manager_config,
    pub(crate) format: Format,
    pub(crate) channels: Option<u32>,
    sample_rate: Option<SampleRate>,
    flags: RmFlags,
    pub(crate) vtables: Vec<*const sys::ma_decoding_backend_vtable>, // keep alive
    _sample_format: PhantomData<F>,
}

impl<F: PcmFormat> AsRawRef for ResourceManagerBuilder<F> {
    type Raw = sys::ma_resource_manager_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.config
    }
}

impl ResourceManagerBuilder<Unknown> {
    fn new_internal(format: Format) -> sys::ma_resource_manager_config {
        let mut config = unsafe { sys::ma_resource_manager_config_init() };
        config.decodedFormat = format.into();
        if let Some(alloc) = AllocationCallbacks::clone_callbacks() {
            config.allocationCallbacks = alloc
        };
        config
    }

    pub fn new_u8() -> ResourceManagerBuilder<u8> {
        ResourceManagerBuilder {
            config: Self::new_internal(Format::U8),
            format: Format::U8,
            channels: None,
            sample_rate: None,
            flags: RmFlags::NONE,
            vtables: Vec::new(),
            _sample_format: PhantomData,
        }
    }

    pub fn new_i16() -> ResourceManagerBuilder<i16> {
        ResourceManagerBuilder {
            config: Self::new_internal(Format::S16),
            format: Format::S16,
            channels: None,
            sample_rate: None,
            flags: RmFlags::NONE,
            vtables: Vec::new(),
            _sample_format: PhantomData,
        }
    }

    pub fn new_s24_packed() -> ResourceManagerBuilder<S24Packed> {
        ResourceManagerBuilder {
            config: Self::new_internal(Format::S24Packed),
            format: Format::S24Packed,
            channels: None,
            sample_rate: None,
            flags: RmFlags::NONE,
            vtables: Vec::new(),
            _sample_format: PhantomData,
        }
    }

    pub fn new_i32() -> ResourceManagerBuilder<i32> {
        ResourceManagerBuilder {
            config: Self::new_internal(Format::S32),
            format: Format::S32,
            channels: None,
            sample_rate: None,
            flags: RmFlags::NONE,
            vtables: Vec::new(),
            _sample_format: PhantomData,
        }
    }

    pub fn new_f32() -> ResourceManagerBuilder<f32> {
        ResourceManagerBuilder {
            config: Self::new_internal(Format::F32),
            format: Format::F32,
            channels: None,
            sample_rate: None,
            flags: RmFlags::NONE,
            vtables: Vec::new(),
            _sample_format: PhantomData,
        }
    }
}

impl<F: PcmFormat> ResourceManagerBuilder<F> {
    /// Sets the number of channels that all audio decoded by this ResourceManager
    /// will be converted to.
    ///
    /// By default, audio keeps its original channel count. If set, all resources
    /// loaded through this ResourceManager will be decoded to this channel count
    /// at load time.
    pub fn channels(&mut self, channels: u32) -> &mut Self {
        self.config.decodedChannels = channels;
        self.channels = Some(channels);
        self
    }

    pub(crate) fn set_backend_registration(&mut self) -> *mut BackendRegistration<F> {
        let registration: BackendRegistration<F> = BackendRegistration {
            channels: self.channels,
            sample_rate: self.sample_rate,
            channel_map: Vec::new(),
            _format: PhantomData,
        };
        let ptr = Box::into_raw(Box::new(registration));
        self.config.pCustomDecodingBackendUserData = ptr.cast();
        ptr
    }

    pub(crate) fn set_backend_vtables(
        &mut self,
    ) -> MaResult<Box<[*const sys::ma_decoding_backend_vtable]>> {
        if self.vtables.is_empty() {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "At least one backend mut be provided.",
            )));
        }
        self.config.customDecodingBackendCount = self.vtables.len() as u32;

        // Each backend is converted to a vtable and stored in a vec
        // Drain that vec and store the vtables into a Box with a stable address
        let mut vtables: Box<[*const sys::ma_decoding_backend_vtable]> =
            self.vtables.drain(..).collect();
        self.config.ppCustomDecodingBackendVTables =
            vtables.as_mut_ptr() as *mut *mut sys::ma_decoding_backend_vtable;

        Ok(vtables)
    }

    pub fn decoding_backend<B: DecodingBackend>(&mut self) -> &mut Self {
        let vtable = decoder_vtable::<F, B>();
        self.vtables.push(vtable);
        self
    }

    /// Sets the [`SampleRate`] that audio decoded by this `ResourceManager`
    /// will be converted to.
    ///
    /// By default, audio is decoded at its original sample rate. If set,
    /// all resources loaded through this `ResourceManager` will be resampled
    /// to this rate during decoding.
    pub fn sample_rate(&mut self, sample_rate: SampleRate) -> &mut Self {
        self.config.decodedSampleRate = sample_rate.into();
        self.sample_rate = Some(sample_rate);
        self
    }

    /// Sets the [`RmFlags`]. Removes any existing ones.
    pub fn flags(&mut self, flags: RmFlags) -> &mut Self {
        self.config.flags = flags.bits();
        self
    }

    pub fn non_blocking(&mut self) -> &mut Self {
        let mut flags = RmFlags::from_bits(self.config.flags);
        flags.insert(RmFlags::NON_BLOCKING);
        self.config.flags = flags.bits();
        self.flags = flags;
        self
    }

    /// Also sets the job_thread_count to 0
    pub fn no_threading(&mut self) -> &mut Self {
        let mut flags = RmFlags::from_bits(self.config.flags);
        flags.insert(RmFlags::NO_THREADING);
        self.config.flags = flags.bits();
        self.flags = flags;
        self
    }

    pub fn job_thread_count(&mut self, count: u32) -> &mut Self {
        self.config.jobThreadCount = count;
        self
    }

    pub fn build(&mut self) -> MaResult<ResourceManager<F>> {
        ResourceManager::<F>::new_with_config(self)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        audio::{channels::Channel, sample_rate::SampleRate},
        data_source::{
            pcm_source::PcmSource,
            sources::decoder::{
                decoding_backend::{DecodingBackend, DrBackendContext},
                Cb, Decoder, DecoderBuilder, DecoderOps,
            },
            DataFormat,
        },
        engine::{
            engine_builder::EngineBuilder,
            resource::{rm_source_flags::RmSourceFlags, RmOps},
        },
        MaResult,
    };
    use crate::{
        data_source::sources::decoder::decoding_backend::DecoderStream, test_assets::wav_i16_le,
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
        type NativeFormat = f32;

        type Decoder<'stream>
            = TestCbDecoder
        where
            Self: 'stream;

        fn init_decoder<'stream>(
            stream: DecoderStream<'stream>,
            _ctx: DrBackendContext,
        ) -> MaResult<Self::Decoder<'stream>> {
            let decoder = DecoderBuilder::new_f32()
                .channels(1)
                .sample_rate(SampleRate::Sr48000)
                .from_reader(stream)?;
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
    fn test_rm_builder_backend_default() {
        let frames_total: usize = 120;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32().build().unwrap();
        let engine = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();

        let src_guard = rm.register_encoded("wav", &wav).unwrap();
        let src = src_guard
            .build_source(RmSourceFlags::NONE, None)
            .unwrap()
            .into_ready()
            .unwrap();

        let sound = engine.new_sound_from_source(&src).unwrap();
        let df = sound.data_format().unwrap();
        assert_eq!(df.channel_map, [Channel::Mono]);
        assert_eq!(df.channels, 1);
        assert_eq!(df.sample_rate, SampleRate::Sr48000);
        assert_eq!(df.format, Format::F32);
    }

    #[test]
    fn test_rm_builder_backend_default_custom_df() {
        let frames_total: usize = 120;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32()
            .channels(2)
            .build()
            .unwrap();
        let engine = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();

        let src_guard = rm.register_encoded("wav", &wav).unwrap();
        let src = src_guard
            .build_source(RmSourceFlags::NONE, None)
            .unwrap()
            .into_ready()
            .unwrap();

        let sound = engine.new_sound_from_source(&src).unwrap();
        let df = sound.data_format().unwrap();
        assert_eq!(df.channel_map, [Channel::Mono, Channel::None]); // bug in miniaudio
        assert_eq!(df.channels, 2);
        assert_eq!(df.sample_rate, SampleRate::Sr48000);
        assert_eq!(df.format, Format::F32);
    }

    #[test]
    fn test_rm_builder_backend_custom() {
        let frames_total: usize = 120;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32()
            .decoding_backend::<TestCbDecoder>()
            .build()
            .unwrap();
        let engine = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();

        let src_guard = rm.register_encoded("wav", &wav).unwrap();
        let src = src_guard
            .build_source(RmSourceFlags::NONE, None)
            .unwrap()
            .into_ready()
            .unwrap();

        let sound = engine.new_sound_from_source(&src).unwrap();
        let df = sound.data_format().unwrap();
        assert_eq!(df.channel_map, [Channel::Mono]);
        assert_eq!(df.channels, 1);
        assert_eq!(df.sample_rate, SampleRate::Sr48000);
        assert_eq!(df.format, Format::F32);
    }

    #[test]
    fn test_rm_builder_backend_custom_custom_df() {
        let frames_total: usize = 120;
        let wav = tiny_test_wav_mono(frames_total);

        let rm = ResourceManagerBuilder::new_f32()
            .channels(2)
            .build()
            .unwrap();
        let engine = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();

        let src_guard = rm.register_encoded("wav", &wav).unwrap();
        let src = src_guard
            .build_source(RmSourceFlags::NONE, None)
            .unwrap()
            .into_ready()
            .unwrap();

        let sound = engine.new_sound_from_source(&src).unwrap();
        let df = sound.data_format().unwrap();
        assert_eq!(df.channel_map, [Channel::Mono, Channel::None]); // bug in miniaudio
        assert_eq!(df.channels, 2);
        assert_eq!(df.sample_rate, SampleRate::Sr48000);
        assert_eq!(df.format, Format::F32);
    }
}
