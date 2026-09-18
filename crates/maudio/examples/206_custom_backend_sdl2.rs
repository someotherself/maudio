use std::{marker::PhantomData, path::PathBuf};

use maudio::{
    audio::{formats::Format, sample_rate::SampleRate},
    backend::{
        custom_backend::CustomBackend,
        custom_context::{BackendDeviceConfig, DeviceDescriptor},
        Backend,
    },
    context::ContextBuilder,
    data_source::sources::decoder::{DecoderBuilder, DecoderOps},
    device::{
        custom_device::BackendDeviceHandle,
        device_builder::DeviceBuilder,
        device_id::DeviceId,
        device_info::{DeviceInfo, DeviceInfoBuilder},
        device_type::DeviceType,
    },
    logging::{Log, LogLevel, LogOps, LogRef},
    pcm_frames::PcmFormat,
    ErrorKinds, MaResult, MaudioError,
};
use sdl2::audio::{AudioCallback, AudioFormat, AudioFormatNum, AudioSpec};

// Note on SDL2:
// When testing with the PulseAudio backend, I have noticed some memory leaks on sdl2 2.26.5.
//
// This seems to be fixed in SDL 2.30.0
// I have not tested backends other than PulseAudio

struct SdlBackend;

struct SdlContext {
    audio: sdl2::AudioSubsystem,
    _sdl: sdl2::Sdl,
}

#[derive(Default)]
struct SdlDevice
where
    BackendDeviceHandle<SdlBackend>: Send,
{
    playback: Option<sdl2::audio::AudioDevice<PlaybackCallback<f32, SdlBackend>>>,
    capture: Option<sdl2::audio::AudioDevice<CaptureCallback<f32, SdlBackend>>>,
}

struct PlaybackCallback<F: PcmFormat, B: CustomBackend> {
    device: BackendDeviceHandle<B>,
    format: PhantomData<fn() -> F>,
}

impl<F, B> AudioCallback for PlaybackCallback<F, B>
where
    F: PcmFormat,
    B: CustomBackend,
    F::StorageUnit: AudioFormatNum + 'static,
    BackendDeviceHandle<B>: Send,
{
    type Channel = F::StorageUnit;

    fn callback(&mut self, buffer: &mut [Self::Channel]) {
        sdl_playback_callback::<F, B>(&self.device, buffer);
    }
}

impl<F, B> AudioCallback for CaptureCallback<F, B>
where
    F: PcmFormat,
    B: CustomBackend,
    F::StorageUnit: AudioFormatNum + 'static,
    BackendDeviceHandle<B>: Send,
{
    type Channel = F::StorageUnit;

    fn callback(&mut self, buffer: &mut [Self::Channel]) {
        // Coerces to the immutable slice your capture function accepts.
        sdl_capture_callback::<F, B>(&self.device, buffer);
    }
}

struct CaptureCallback<F: PcmFormat, B: CustomBackend> {
    device: BackendDeviceHandle<B>,
    format: PhantomData<fn() -> F>,
}

fn desired_spec(
    descriptor: &DeviceDescriptor,
    config: &BackendDeviceConfig,
) -> MaResult<sdl2::audio::AudioSpecDesired> {
    let sample_rate = descriptor.sample_rate.unwrap_or(SampleRate::Sr48000).into();

    let freq = i32::try_from(sample_rate)
        .map_err(|_| MaudioError::other("Sample rate exceeds SDL's range"))?;

    let channels = descriptor.channels.map(|c| c as u8);

    // Proposed maudio helper—not an existing method:
    let frames =
        descriptor.calculate_buffer_size_in_frames(sample_rate, config.performance_profile);

    let samples = frames.clamp(1, 32_768).next_power_of_two() as u16;

    Ok(sdl2::audio::AudioSpecDesired {
        freq: Some(freq),
        channels,
        samples: Some(samples),
    })
}

fn sdl_capture_callback<F: PcmFormat, B: CustomBackend>(
    device: &BackendDeviceHandle<B>,
    buffer: &[F::StorageUnit],
) {
    // playback and capture can have different formats, but we only pass one at a time here
    // We pass the second format generic as F to the function in this case
    let _ = device.handle_backend_data_callback::<F, F>(None, Some(buffer));
}

fn sdl_playback_callback<F: PcmFormat, B: CustomBackend>(
    device: &BackendDeviceHandle<B>,
    buffer: &mut [F::StorageUnit],
) {
    let _ = device.handle_backend_data_callback::<F, F>(Some(buffer), None);
}

fn apply_obtained_spec(descriptor: &mut DeviceDescriptor, sdl_spec: &AudioSpec) -> MaResult<()> {
    println!("sdl2 samples: {}", sdl_spec.samples);
    println!("sdl2 size: {}", sdl_spec.size);

    descriptor.format = Format::F32;
    descriptor.channels = Some(sdl_spec.channels as u32);
    descriptor.sample_rate = (sdl_spec.freq as u32).try_into().ok();
    descriptor.period_size_frames = sdl_spec.samples as u32;
    descriptor.period_size_millis = 0;
    descriptor.period_count = 1;
    Ok(())
}

impl CustomBackend for SdlBackend {
    type Context = SdlContext;
    type Device = SdlDevice;

    fn init_context(log: Option<LogRef>) -> MaResult<Self::Context> {
        if let Some(log) = log {
            log.post(LogLevel::Debug, "Attempting to initialize SDL2 backend")?;
        }
        let sdl = sdl2::init().map_err(MaudioError::other)?;
        let audio = sdl.audio().map_err(|e| {
            println!("{e}");
            MaudioError::other(e)
        })?;

        Ok(SdlContext { audio, _sdl: sdl })
    }

    fn enumerate_devices<F>(
        context: &mut Self::Context,
        mut report: F,
        _log: Option<LogRef>,
    ) -> MaResult<()>
    where
        F: FnMut(DeviceType, &DeviceInfo) -> bool,
    {
        let count = context.audio.num_audio_playback_devices().unwrap_or(0);

        for index in 0..count {
            let name = context
                .audio
                .audio_playback_device_name(index)
                .map_err(MaudioError::other)?;
            let info = DeviceInfoBuilder::from_name(name)?.build();
            if !report(DeviceType::Playback, &info) {
                return Ok(());
            }
        }

        let count = context.audio.num_audio_capture_devices().unwrap_or(0);

        for index in 0..count {
            let name = context
                .audio
                .audio_capture_device_name(index)
                .map_err(MaudioError::other)?;
            let info = DeviceInfoBuilder::from_name(name)?.build();
            if !report(DeviceType::Capture, &info) {
                return Ok(());
            }
        }

        Ok(())
    }

    fn context_get_device_info(
        context: &mut Self::Context,
        device_type: DeviceType,
        device_id: maudio::device::device_id::DeviceId,
        _log: Option<LogRef>,
    ) -> MaResult<DeviceInfo> {
        let capture = match device_type {
            DeviceType::Playback => false,
            DeviceType::Capture => true,
            _ => {
                return Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented));
            }
        };

        let count = if capture {
            context.audio.num_audio_capture_devices()
        } else {
            context.audio.num_audio_playback_devices()
        }
        .ok_or_else(|| MaudioError::other("SDL cannot enumerate audio devices"))?;

        for index in 0..count {
            let name = if capture {
                context.audio.audio_capture_device_name(index)
            } else {
                context.audio.audio_playback_device_name(index)
            }
            .map_err(MaudioError::other)?;

            // Use the same ID construction as enumerate_devices.
            if DeviceId::custom_from_name(name.clone())? != device_id {
                continue;
            }

            let spec = if capture {
                context.audio.audio_capture_device_spec(index)
            } else {
                context.audio.audio_playback_device_spec(index)
            }
            .map_err(MaudioError::other)?;

            let format = match spec.format {
                AudioFormat::U8 => Format::U8,
                f if f == AudioFormat::s16_sys() => Format::S16,
                f if f == AudioFormat::s32_sys() => Format::S32,
                f if f == AudioFormat::f32_sys() => Format::F32,
                other => {
                    return Err(MaudioError::other(format!(
                        "SDL audio format {other:?} cannot be represented by maudio"
                    )));
                }
            };

            if spec.freq <= 0 || spec.channels == 0 {
                return Err(MaudioError::other(
                    "SDL returned an invalid audio specification",
                ));
            }

            let sample_rate = SampleRate::try_from(spec.freq as u32)?;

            let mut info = DeviceInfoBuilder::new(device_id, name);
            info.add_data_formats(format, u32::from(spec.channels), sample_rate, false);

            return Ok(info.build());
        }

        Err(MaudioError::other("SDL audio device was not found"))
    }

    fn device_init(
        device: BackendDeviceHandle<Self>,
        config: BackendDeviceConfig,
        playback: Option<&mut DeviceDescriptor>,
        capture: Option<&mut DeviceDescriptor>,
        log: Option<LogRef>,
    ) -> MaResult<Self::Device>
    where
        Self: Sized,
    {
        let post = |level: LogLevel, message: &str| {
            if let Some(log) = log.as_ref() {
                let _ = log.post(level, message);
            }
        };

        if config.device_type == DeviceType::Loopback {
            let message = "SDL2 backend does not support loopback";
            post(LogLevel::Error, message);
            return Err(MaudioError::other("SDL2 backend does not support loopback"));
        }

        // Proposed accessor returning &SdlContext.
        let audio = &device.backend_context().audio;

        let mut state = SdlDevice::default();

        if matches!(config.device_type, DeviceType::Capture | DeviceType::Duplex) {
            let descriptor =
                capture.ok_or_else(|| MaudioError::other("Missing capture descriptor"))?;

            let name = match descriptor.device_id.as_ref() {
                None => None,
                Some(id) => Some(
                    id.get_custom_name()
                        .ok_or_else(|| MaudioError::other("Expected an SDL device name"))?,
                ),
            };

            let desired: sdl2::audio::AudioSpecDesired = desired_spec(descriptor, &config)?;

            post(
                LogLevel::Debug,
                &format!(
                    "Opening SDL2 capture device '{:?}': \
                    format=f32, sample_rate={:?}, channels={:?}, period_size_frames={:?}",
                    name, desired.freq, desired.channels, desired.samples,
                ),
            );

            // Proposed adapter: forwards captured PCM to miniaudio.

            let callback = CaptureCallback::<f32, Self> {
                device: device.clone(),
                format: PhantomData,
            };

            let opened = audio
                .open_capture(name.as_deref(), &desired, move |_| callback)
                .map_err(|error| {
                    post(
                        LogLevel::Error,
                        &format!("Failed to open SDL2 capture device '{:?}': {}", name, error),
                    );
                    MaudioError::other(error)
                })?;

            debug_assert_eq!(opened.spec().format, AudioFormat::f32_sys());

            apply_obtained_spec(descriptor, opened.spec()).map_err(|error| {
                post(
                    LogLevel::Error,
                    &format!("Failed to apply SDL2 capture specification: {}", error),
                );
                error
            })?;

            let spec = opened.spec();
            post(
                LogLevel::Debug,
                &format!(
                    "SDL2 capture device initialized: \
                    format={:?}, sample_rate={}, channels={}, period_size_frames={}",
                    spec.format, spec.freq, spec.channels, spec.samples,
                ),
            );

            state.capture = Some(opened);
        }

        if matches!(
            config.device_type,
            DeviceType::Playback | DeviceType::Duplex
        ) {
            let descriptor =
                playback.ok_or_else(|| MaudioError::other("Missing playback descriptor"))?;

            let name = match descriptor.device_id.as_ref() {
                None => None,
                Some(id) => Some(
                    id.get_custom_name()
                        .ok_or_else(|| MaudioError::other("Expected an SDL device name"))?,
                ),
            };

            let desired = desired_spec(descriptor, &config)?;

            post(
                LogLevel::Debug,
                &format!(
                    "Opening SDL2 playback device '{:?}': \
         format=f32, sample_rate={:?}, channels={:?}, period_size_frames={:?}",
                    name, desired.freq, desired.channels, desired.samples,
                ),
            );

            let callback = PlaybackCallback::<f32, Self> {
                device: device.clone(),
                format: PhantomData,
            };
            let opened = audio
                .open_playback(name.as_deref(), &desired, move |_| callback)
                .map_err(|error| {
                    post(
                        LogLevel::Error,
                        &format!(
                            "Failed to open SDL2 playback device '{:?}': {}",
                            name, error
                        ),
                    );
                    MaudioError::other(error)
                })?;

            debug_assert_eq!(opened.spec().format, AudioFormat::f32_sys());

            apply_obtained_spec(descriptor, opened.spec()).map_err(|error| {
                post(
                    LogLevel::Error,
                    &format!("Failed to apply SDL2 playback specification: {}", error),
                );
                error
            })?;

            let spec = opened.spec();
            post(
                LogLevel::Debug,
                &format!(
                    "SDL2 playback device initialized: \
         format={:?}, sample_rate={}, channels={}, period_size_frames={}",
                    spec.format, spec.freq, spec.channels, spec.samples,
                ),
            );

            state.playback = Some(opened);
        }

        Ok(state)
    }

    fn device_start(device: &BackendDeviceHandle<Self>) -> MaResult<()>
    where
        Self: Sized,
    {
        let Some(device) = device.user_device() else {
            return Err(MaudioError::new_ma_error(ErrorKinds::Other(
                "Backend device not available".to_string(),
            )));
        };

        if let Some(playback) = &device.playback {
            playback.resume();
        }

        if let Some(capture) = &device.capture {
            capture.resume();
        }

        Ok(())
    }

    fn device_stop(device: &BackendDeviceHandle<Self>) -> MaResult<()>
    where
        Self: Sized,
    {
        let Some(device) = device.user_device() else {
            return Err(MaudioError::new_ma_error(ErrorKinds::Other(
                "Backend device not available".to_string(),
            )));
        };

        if let Some(playback) = &device.playback {
            playback.pause();
        }

        if let Some(capture) = &device.capture {
            capture.pause();
        }

        Ok(())
    }
}

fn main() -> MaResult<()> {
    let log = Log::new()?;
    log.print_level(LogLevel::Debug)?;
    log.print_level(LogLevel::Error)?;
    log.print_level(LogLevel::Info)?;
    log.print_level(LogLevel::Warning)?;

    let context = ContextBuilder::new()
        .log(&log)
        .preferred_backends([Backend::Custom])
        .build_custom::<SdlBackend>()?;

    // let mut id = None;

    // context.enumerate_devices(|ty, info| {
    //     println!("SDL2 - Type: {}, name: {}", ty, info.name());

    //     EnumerateControl::Continue
    // })?;

    // // Verify that a stop request ends the entire enumeration.
    // let mut visits = 0;
    // context.enumerate_devices(|_, info| {
    //     visits += 1;

    //     if visits == 2 {
    //         id = Some(info.id());
    //     }

    //     if visits == 3 {
    //         return EnumerateControl::Stop;
    //     }
    //     EnumerateControl::Continue
    // })?;
    // assert!(visits == 3);
    // println!("Stopped with visits = {visits}");

    // let id = id.unwrap();
    // let info = context.device_info(DeviceType::Playback, &id)?;

    // println!(
    //     "Device Info: {} / f count: {:?}",
    //     info.device_name(),
    //     info.device_formats()
    // );

    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let mut decoder = DecoderBuilder::new_i16()
        .channels(2)
        .sample_rate(SampleRate::Sr44100)
        .from_file(&path)?;

    let data_format = decoder.data_format()?;

    let mut device =
        DeviceBuilder::playback()
            .i16()
            .with_custom_context(&context, move |_, out| {
                let frames_read = decoder.read_pcm_frames_into(out).unwrap_or(0);

                let samples_read = frames_read * data_format.channels as usize;

                if samples_read < out.len() {
                    out[samples_read..].fill(0);
                }
            })?;

    device.device_start()?;

    std::thread::sleep(std::time::Duration::from_secs(1));
    device.device_stop()?;

    Ok(())
}
