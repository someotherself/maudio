use std::marker::PhantomData;

use maudio::{
    ErrorKinds, MaResult, MaudioError,
    audio::{formats::Format, sample_rate::SampleRate},
    backend::{
        custom_backend::CustomBackend,
        custom_context::{BackendDeviceConfig, DeviceDescriptor},
    },
    device::{
        custom_device::BackendDeviceHandle,
        device_id::DeviceId,
        device_info::{DeviceInfo, DeviceInfoBuilder},
        device_type::DeviceType,
    },
    logging::{LogLevel, LogOps, LogRef},
    pcm_frames::MaSampleFormat,
};
use sdl2::audio::{
    AudioCallback, AudioDevice, AudioFormat, AudioFormatNum, AudioSpec, AudioSpecDesired,
};

pub struct SdlBackend;

pub struct SdlContext {
    audio: sdl2::AudioSubsystem,
    _sdl: sdl2::Sdl,
}

#[derive(Default)]
pub struct SdlDevice<'device>
where
    BackendDeviceHandle<'device, SdlBackend>: Send,
{
    playback: Option<AudioDevice<PlaybackCallback<'device, f32, SdlBackend>>>,
    playback_identity: Option<String>, // None if using default device
    capture: Option<AudioDevice<CaptureCallback<'device, f32, SdlBackend>>>,
    capture_identity: Option<String>,
}

pub struct PlaybackCallback<'device, F: MaSampleFormat, B: CustomBackend> {
    device: BackendDeviceHandle<'device, B>,
    format: PhantomData<fn() -> F>,
}

impl<'device, F, B> AudioCallback for PlaybackCallback<'device, F, B>
where
    F: MaSampleFormat,
    B: CustomBackend,
    F::StorageUnit: AudioFormatNum + 'static,
    BackendDeviceHandle<'device, B>: Send,
{
    type Channel = F::StorageUnit;

    fn callback(&mut self, buffer: &mut [Self::Channel]) {
        sdl_playback_callback::<F, B>(&self.device, buffer);
    }
}

pub struct CaptureCallback<'device, F: MaSampleFormat, B: CustomBackend> {
    device: BackendDeviceHandle<'device, B>,
    format: PhantomData<fn() -> F>,
}

impl<'device, F, B> AudioCallback for CaptureCallback<'device, F, B>
where
    F: MaSampleFormat,
    B: CustomBackend,
    F::StorageUnit: AudioFormatNum + 'static,
    BackendDeviceHandle<'device, B>: Send,
{
    type Channel = F::StorageUnit;

    fn callback(&mut self, buffer: &mut [Self::Channel]) {
        sdl_capture_callback::<F, B>(&self.device, buffer);
    }
}

/// Creates a AudioSpecDesired with our desired audio configuration
fn desired_spec(
    descriptor: &DeviceDescriptor,
    config: &BackendDeviceConfig,
) -> MaResult<AudioSpecDesired> {
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

/// Helper function to call the device data callback for capture
fn sdl_capture_callback<F: MaSampleFormat, B: CustomBackend>(
    device: &BackendDeviceHandle<B>,
    buffer: &[F::StorageUnit],
) {
    // playback and capture can have different formats, but we only pass one at a time here
    // We pass the second format generic as F to the function in this case
    let _ = device.handle_backend_data_callback::<F, F>(None, Some(buffer));
}

/// Helper function to call the device data callback for playback
fn sdl_playback_callback<F: MaSampleFormat, B: CustomBackend>(
    device: &BackendDeviceHandle<B>,
    buffer: &mut [F::StorageUnit],
) {
    let _ = device.handle_backend_data_callback::<F, F>(Some(buffer), None);
}

/// Helper to populate a DeviceDescriptor with the backends configuration
fn apply_obtained_spec(descriptor: &mut DeviceDescriptor, sdl_spec: &AudioSpec) -> MaResult<()> {
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
    type Device<'device> = SdlDevice<'device>;

    fn init_context(log: Option<&LogRef>) -> MaResult<Self::Context> {
        if let Some(ref log) = log {
            log.post(LogLevel::Debug, "Attempting to initialize SDL2 backend")?;
        }
        let sdl = sdl2::init().map_err(MaudioError::other)?;
        let audio = sdl.audio().map_err(|e| {
            if let Some(ref log) = log {
                let _ = log.post(
                    LogLevel::Error,
                    format!("Failed to initialize SDL2 subsystem: {:?}", &e),
                );
            }
            MaudioError::other(e)
        })?;

        Ok(SdlContext { audio, _sdl: sdl })
    }

    fn enumerate_devices<F>(
        context: &mut Self::Context,
        mut report: F,
        _log: Option<&LogRef>,
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

    fn context_query_device_info(
        context: &mut Self::Context,
        device_type: DeviceType,
        device_id: DeviceId,
        _log: Option<&LogRef>,
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

    fn init_device<'device>(
        device: BackendDeviceHandle<'device, Self>,
        config: BackendDeviceConfig,
        playback: Option<&mut DeviceDescriptor>,
        capture: Option<&mut DeviceDescriptor>,
        log: Option<&LogRef>,
    ) -> MaResult<Self::Device<'device>>
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

            let desired: AudioSpecDesired = desired_spec(descriptor, &config)?;

            post(
                LogLevel::Debug,
                &format!(
                    "Opening SDL2 capture device '{:?}': \
                    format=f32, sample_rate={:?}, channels={:?}, period_size_frames={:?}",
                    name, desired.freq, desired.channels, desired.samples,
                ),
            );

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
            state.capture_identity = name;
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
            state.playback_identity = name;
        }

        Ok(state)
    }

    fn device_start<'device>(
        device: &'device BackendDeviceHandle<Self>,
        _log: Option<&LogRef>,
    ) -> MaResult<()>
    where
        Self: Sized,
    {
        let Some(device) = device.backend_device() else {
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

    fn device_stop<'device>(
        device: &BackendDeviceHandle<'device, Self>,
        _log: Option<&LogRef>,
    ) -> MaResult<()>
    where
        Self: Sized,
    {
        let Some(device) = device.backend_device() else {
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

    fn device_get_info<'device>(
        device: BackendDeviceHandle<'device, Self>,
        _context: &'device Self::Context,
        device_type: DeviceType,
        _log: Option<&LogRef>,
    ) -> MaResult<DeviceInfo>
    where
        Self: Sized,
    {
        let Some(device) = device.backend_device() else {
            return Err(MaudioError::new_ma_error(ErrorKinds::Other(
                "Backend device not available".to_string(),
            )));
        };

        if device_type == DeviceType::Playback {
            if let Some(name) = &device.playback_identity {
                return Ok(DeviceInfoBuilder::from_name(name)?.build());
            } else {
                return Err(MaudioError::new_ma_error(ErrorKinds::Other(
                    "Querying the default device is not supported".to_string(),
                )));
            }
        };

        if device_type == DeviceType::Capture {
            if let Some(name) = &device.capture_identity {
                let id = DeviceId::custom_from_name(name)?;
                return Ok(DeviceInfoBuilder::new(id, name.clone()).build());
            } else {
                return Err(MaudioError::new_ma_error(ErrorKinds::Other(
                    "Querying the default device is not supported".to_string(),
                )));
            }
        }

        return Err(MaudioError::new_ma_error(ErrorKinds::Other(
            "Unsupported device type".to_string(),
        )));
    }
}
