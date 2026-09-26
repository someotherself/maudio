use asio_sys::{AsioSampleType, BufferCallbackId, Driver};
use maudio::{
    audio::{
        channels::target_channel_position, converters::channel_converter::default_channel_map_into,
        formats::Format,
    },
    backend::{
        custom_backend::CustomBackend,
        custom_context::{BackendDeviceConfig, DeviceDescriptor},
        Backend,
    },
    context::{ContextBuilder, ContextOps, EnumerateControl},
    device::{
        custom_device::BackendDeviceHandle, device_id::DeviceId, device_info::DeviceInfoBuilder,
        device_type::DeviceType,
    },
    engine::engine_builder::EngineBuilder,
    logging::{Log, LogLevel, LogOps, LogRef},
    pcm_frames::MaSampleFormat,
    ErrorKinds, MaResult, MaudioError,
};

struct AsioBackend;

// ASIO can only represent one opened maudio device
// Whether is playback, capture or duplex depends on streams opened
struct AsioDriver {
    driver: Driver,
    callback_id: BufferCallbackId,
}

impl Drop for AsioDriver {
    fn drop(&mut self) {
        self.driver.remove_callback(self.callback_id);
    }
}

fn report_stream_spec(
    sample_rate: u32,
    buffer_size: usize,
    channels: u32,
    sample_type: AsioSampleType,
    descriptor: &mut DeviceDescriptor,
) -> MaResult<()> {
    descriptor.channels = Some(channels);
    descriptor.sample_rate = Some(sample_rate.try_into()?);
    descriptor.period_size_frames = buffer_size as u32;
    default_channel_map_into(&mut descriptor.channel_map, Some(target_channel_position()));
    descriptor.format = direct_asio_format(sample_type)?;

    Ok(())
}

fn requested_asio_buffer_size(
    config: &BackendDeviceConfig,
    sample_rate: u32,
) -> MaResult<Option<i32>> {
    let frames = if config.period_size_frames != 0 {
        u64::from(config.period_size_frames)
    } else if config.period_size_millis != 0 {
        // Round up so the period is at least the requested duration.
        (u64::from(config.period_size_millis) * u64::from(sample_rate)).div_ceil(1000)
    } else {
        return Ok(None); // ASIO's preferred buffer size
    };

    let frames =
        i32::try_from(frames).map_err(|_| MaudioError::other("ASIO period size is too large"))?;

    if frames == 0 {
        return Err(MaudioError::other("ASIO period size is zero"));
    }

    Ok(Some(frames))
}

fn direct_asio_format(sample_type: asio_sys::AsioSampleType) -> MaResult<Format> {
    use asio_sys::AsioSampleType::*;

    match sample_type {
        ASIOSTInt16LSB => Ok(Format::S16),
        ASIOSTInt32LSB => Ok(Format::S32),
        ASIOSTFloat32LSB => Ok(Format::F32),
        other => Err(MaudioError::other(format!(
            "ASIO sample type {other:?} is not supported by this example"
        ))),
    }
}

fn register_playback_callback<F: MaSampleFormat>(
    handle: BackendDeviceHandle<'static, AsioBackend>,
    driver: &asio_sys::Driver,
    frames: usize,
    channels: usize,
) -> asio_sys::BufferCallbackId
where
    F::StorageUnit: 'static + Send,
{
    let streams = driver.streams();
    let mut interleaved = vec![F::STORE_SILENCE; frames * channels];

    driver.add_callback(move |callback_info| {
        let Ok(buffer_index) = usize::try_from(callback_info.buffer_index) else {
            return;
        };
        if buffer_index >= 2 {
            return;
        }

        {
            let Ok(streams) = streams.lock() else {
                return;
            };
            let Some(output) = streams.output.as_ref() else {
                return;
            };
            if output.buffer_size as usize != frames || output.buffer_infos.len() != channels {
                return;
            }

            for (channel_index, channel_info) in output.buffer_infos.iter().enumerate() {
                // Copy the field out: AsioBufferInfo is packed.
                let buffers = channel_info.buffers;
                let ptr = buffers[buffer_index].cast::<F::StorageUnit>();
                if ptr.is_null() {
                    return;
                }

                // SAFETY: ASIO owns this selected channel buffer; stream
                // preparation established `frames` f32 samples for it.
                let channel = unsafe { std::slice::from_raw_parts(ptr.cast_const(), frames) };

                for (frame_index, &sample) in channel.iter().enumerate() {
                    interleaved[frame_index * channels + channel_index] = sample;
                }
            }
        } // Release the ASIO stream lock before calling maudio.

        playback_callback::<F>(handle.clone(), &mut interleaved);
    })
}

fn register_capture_callback<F: MaSampleFormat>(
    handle: BackendDeviceHandle<'static, AsioBackend>,
    driver: &asio_sys::Driver,
    frames: usize,
    channels: usize,
) -> asio_sys::BufferCallbackId
where
    F::StorageUnit: 'static + Send,
{
    let streams = driver.streams();
    let mut interleaved = vec![F::STORE_SILENCE; frames * channels];

    driver.add_callback(move |callback_info| {
        let Ok(buffer_index) = usize::try_from(callback_info.buffer_index) else {
            return;
        };
        if buffer_index >= 2 {
            return;
        }

        {
            let Ok(streams) = streams.lock() else {
                return;
            };
            let Some(input) = streams.input.as_ref() else {
                return;
            };
            if input.buffer_size as usize != frames || input.buffer_infos.len() != channels {
                return;
            }

            for (channel_index, channel_info) in input.buffer_infos.iter().enumerate() {
                // Copy the field out: AsioBufferInfo is packed.
                let buffers = channel_info.buffers;
                let ptr = buffers[buffer_index].cast::<F::StorageUnit>();
                if ptr.is_null() {
                    return;
                }

                // SAFETY: ASIO owns this selected channel buffer; stream
                // preparation established `frames` f32 samples for it.
                let channel = unsafe { std::slice::from_raw_parts(ptr.cast_const(), frames) };

                for (frame_index, &sample) in channel.iter().enumerate() {
                    interleaved[frame_index * channels + channel_index] = sample;
                }
            }
        } // Release the ASIO stream lock before calling maudio.

        capture_callback::<F>(handle.clone(), &interleaved);
    })
}

fn playback_callback<F: MaSampleFormat>(
    handle: BackendDeviceHandle<'static, AsioBackend>,
    buffer: &mut [F::StorageUnit],
) {
    let _ = handle.handle_backend_data_callback::<F, F>(Some(buffer), None);
}

fn capture_callback<F: MaSampleFormat>(
    handle: BackendDeviceHandle<'static, AsioBackend>,
    buffer: &[F::StorageUnit],
) {
    let _ = handle.handle_backend_data_callback::<F, F>(None, Some(buffer));
}

impl CustomBackend for AsioBackend {
    type Context = asio_sys::Asio;
    type Device<'device> = AsioDriver;

    fn init_context(_log: Option<&LogRef>) -> maudio::MaResult<Self::Context> {
        Ok(asio_sys::Asio::new())
    }

    fn context_query_device_info(
        context: &mut Self::Context,
        device_type: DeviceType,
        device_id: maudio::device::device_id::DeviceId,
        log: Option<&maudio::logging::LogRef>,
    ) -> maudio::MaResult<maudio::device::device_info::DeviceInfo> {
        if matches!(device_type, DeviceType::Loopback) {
            if let Some(log) = log.as_ref() {
                let _ = log.post(LogLevel::Error, "Loopback is not supported");
            }
            return Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented));
        }

        for name in context.driver_names() {
            if DeviceId::custom_from_name(name.clone())? == device_id {
                return Ok(DeviceInfoBuilder::new(device_id, name).build());
            }
        }

        Err(MaudioError::other("ASIO driver was not found"))
    }

    fn enumerate_devices<F>(
        context: &mut Self::Context,
        mut f: F,
        _log: Option<&maudio::logging::LogRef>,
    ) -> MaResult<()>
    where
        F: FnMut(DeviceType, &maudio::device::device_info::DeviceInfo) -> bool,
    {
        eprintln!("before enumeration: {}", context.driver_names().len());
        for name in context.driver_names() {
            let Ok(driver) = context.load_driver(&name) else {
                continue;
            };
            let name = driver.name();

            let Ok(channels) = driver.channels() else {
                continue;
            };
            let info = DeviceInfoBuilder::from_name(name)?.build();

            if channels.ins != 0 && !f(DeviceType::Capture, &info) {
                return Ok(());
            }

            if channels.outs != 0 && !f(DeviceType::Playback, &info) {
                return Ok(());
            }
            let _ = driver.destroy();
        }
        eprintln!("after enumeration: {}", context.driver_names().len());

        Ok(())
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
        eprintln!("init device - 1");
        let post = |level: LogLevel, message: &str| {
            if let Some(log) = log.as_ref() {
                let _ = log.post(level, message);
            }
        };

        if config.device_type == DeviceType::Loopback {
            post(LogLevel::Error, "Loopback is not supported.");
            return Err(MaudioError::other("Loopback is not supported"));
        }
        eprintln!("init device - 2");

        let context = device.backend_context();

        if matches!(config.device_type, DeviceType::Duplex) {
            // TODO later
        }

        eprintln!("found {} names", context.driver_names().len());
        for name_check in context.driver_names() {
            eprintln!("name check: {}", name_check);
        }

        if matches!(config.device_type, DeviceType::Playback) {
            eprintln!("init device - 3");
            let descriptor =
                playback.ok_or_else(|| MaudioError::other("Missing capture descriptor"))?;

            // TODO: How to handle picking a 'default' device?
            let Some(device_id) = descriptor.device_id.as_ref() else {
                eprintln!("No device id");
                post(LogLevel::Error, "A device id must be provided.");
                return Err(MaudioError::other("A device id must be provided."));
            };
            eprintln!("init device - 4");

            let Some(name) = device_id.get_custom_name() else {
                post(LogLevel::Error, "A device id must be provided.");
                return Err(MaudioError::other("A device id must be provided."));
            };
            eprintln!("init device - 5");

            let message = format!("Opening device: {}.", &name);
            eprintln!("{}", &message);
            post(LogLevel::Debug, &message);

            let driver = match context.load_driver(&name) {
                Ok(driver) => driver,
                Err(e) => {
                    let message = format!("Could not load device: {}.", e);
                    eprintln!("{}", &message);
                    post(LogLevel::Error, &message);
                    return Err(MaudioError::other(message));
                }
            };

            eprintln!("init device - 6");
            let asio_streams = driver.streams();
            let Ok(mut streams) = asio_streams.lock() else {
                post(LogLevel::Error, "Stream lock poisoned");
                return Err(MaudioError::other("Stream lock poisoned"));
            };
            eprintln!("init device - 7");

            let channels = config.playback_channels.unwrap_or(2);

            let sample_rate = driver
                .sample_rate()
                .ok()
                .and_then(|s| Some(s as u32))
                .or(config.sample_rate.map(|s| s.into()))
                .unwrap_or(44_100);
            let buffer_size = requested_asio_buffer_size(&config, sample_rate)?;

            // TODO: If this uses en existing stream, make sure to report sample rate and channels
            let frames = match streams.output {
                Some(ref output) => Ok(output.buffer_size as usize),
                None => {
                    let output = streams.input.take();
                    driver
                        .prepare_output_stream(output, channels as usize, buffer_size)
                        .map(|new_streams| {
                            let bs = match new_streams.output {
                                Some(ref out) => out.buffer_size as usize,
                                None => unreachable!(),
                            };
                            *streams = new_streams;
                            bs
                        })
                }
            };
            let Ok(frames) = frames else {
                post(LogLevel::Error, "Could not create output stream");
                return Err(MaudioError::other("Could not create output stream"));
            };

            let sample_type = driver.output_data_type().map_err(MaudioError::other)?;

            // Safety: We call `unregister_callback` when the AsioDriver is dropped
            let device = unsafe { device.clone_static_unchecked() };

            let callback_id = match sample_type {
                AsioSampleType::ASIOSTFloat32LSB => {
                    register_playback_callback::<f32>(device, &driver, frames, channels as usize)
                }

                AsioSampleType::ASIOSTInt16LSB => {
                    register_playback_callback::<i16>(device, &driver, frames, channels as usize)
                }

                AsioSampleType::ASIOSTInt32LSB => {
                    register_playback_callback::<i32>(device, &driver, frames, channels as usize)
                }

                other => {
                    return Err(MaudioError::other(format!(
                        "Unsupported ASIO sample type: {other:?}"
                    )))
                }
            };

            report_stream_spec(sample_rate, frames, channels, sample_type, descriptor)?;

            return Ok(AsioDriver {
                driver,
                callback_id,
            });
        }

        if matches!(config.device_type, DeviceType::Capture) {
            let descriptor =
                capture.ok_or_else(|| MaudioError::other("Missing capture descriptor"))?;

            // TODO: How to handle picking a 'default' device?
            let Some(device_id) = descriptor.device_id.as_ref() else {
                post(LogLevel::Error, "A device id must be provided.");
                return Err(MaudioError::other("A device id must be provided."));
            };

            let Some(name) = device_id.get_custom_name() else {
                post(LogLevel::Error, "A device id must be provided.");
                return Err(MaudioError::other("A device id must be provided."));
            };

            let Ok(driver) = context.load_driver(&name) else {
                post(LogLevel::Error, "Could not load device.");
                return Err(MaudioError::other("Could not load device."));
            };

            let asio_streams = driver.streams();
            let Ok(mut streams) = asio_streams.lock() else {
                post(LogLevel::Error, "Stream lock poisoned");
                return Err(MaudioError::other("Stream lock poisoned"));
            };

            let channels = config.capture_channels.unwrap_or(2);

            let sample_rate = driver
                .sample_rate()
                .ok()
                .and_then(|s| Some(s as u32))
                .or(config.sample_rate.map(|s| s.into()))
                .unwrap_or(44_100);
            let buffer_size = requested_asio_buffer_size(&config, sample_rate)?;

            // TODO: If this uses en existing stream, make sure to report sample rate and channels output
            let frames = match streams.input {
                Some(ref input) => Ok(input.buffer_size as usize),
                None => {
                    let input = streams.input.take();
                    driver
                        .prepare_input_stream(input, channels as usize, buffer_size)
                        .map(|new_streams| {
                            let bs = match new_streams.input {
                                Some(ref input) => input.buffer_size as usize,
                                None => unreachable!(),
                            };
                            *streams = new_streams;
                            bs
                        })
                }
            };

            let Ok(frames) = frames else {
                post(LogLevel::Error, "Could not create input stream");
                return Err(MaudioError::other("Could not create input stream"));
            };

            let sample_type = driver.input_data_type().map_err(MaudioError::other)?;

            // Safety: We call `unregister_callback` when the AsioDriver is dropped
            let device = unsafe { device.clone_static_unchecked() };

            let callback_id = match sample_type {
                AsioSampleType::ASIOSTFloat32LSB => {
                    register_capture_callback::<f32>(device, &driver, frames, channels as usize)
                }

                AsioSampleType::ASIOSTInt16LSB => {
                    register_capture_callback::<i16>(device, &driver, frames, channels as usize)
                }

                AsioSampleType::ASIOSTInt32LSB => {
                    register_capture_callback::<i32>(device, &driver, frames, channels as usize)
                }

                other => {
                    return Err(MaudioError::other(format!(
                        "Unsupported ASIO sample type: {other:?}"
                    )))
                }
            };

            report_stream_spec(sample_rate, frames, channels, sample_type, descriptor)?;

            return Ok(AsioDriver {
                driver,
                callback_id,
            });
        }

        unreachable!() // we already checked for loopback
    }

    fn device_start<'device>(
        device: &BackendDeviceHandle<'device, Self>,
        log: Option<&LogRef>,
    ) -> MaResult<()>
    where
        Self: Sized,
    {
        let post = |level: LogLevel, message: &str| {
            if let Some(log) = log.as_ref() {
                let _ = log.post(level, message);
            }
        };

        let Some(dev) = device.backend_device() else {
            post(LogLevel::Error, "Backend device not available.");
            return Err(MaudioError::other("Backend device not available"));
        };

        if let Err(e) = dev.driver.start() {
            let message = format!("Failed to start backend device: {}", e);
            post(LogLevel::Error, &message);
            return Err(MaudioError::other(&message));
        };

        Ok(())
    }

    fn device_stop<'device>(
        device: &BackendDeviceHandle<'device, Self>,
        log: Option<&LogRef>,
    ) -> MaResult<()>
    where
        Self: Sized,
    {
        let post = |level: LogLevel, message: &str| {
            if let Some(log) = log.as_ref() {
                let _ = log.post(level, message);
            }
        };

        let Some(dev) = device.backend_device() else {
            post(LogLevel::Error, "Backend device not available.");
            return Err(MaudioError::other("Backend device not available"));
        };

        if let Err(e) = dev.driver.stop() {
            let message = format!("Failed to stop backend device: {}", e);
            post(LogLevel::Error, &message);
            return Err(MaudioError::other(&message));
        };

        Ok(())
    }
}

fn main() -> MaResult<()> {
    let log = Log::new()?;
    log.print_level(LogLevel::Info)?;
    log.print_level(LogLevel::Debug)?;
    log.print_level(LogLevel::Error)?;

    println!("Enter command: \"enumerate\" or \"play\"");

    let mut command = String::new();
    std::io::stdin().read_line(&mut command)?;

    match command.trim() {
        "enumerate" => {
            let ctx = ContextBuilder::new()
                .log(&log)
                .preferred_backends([Backend::Custom])
                .build_custom::<AsioBackend>()?;

            // Use the same numbering as the name-only lookup in play.
            let asio = asio_sys::Asio::new();
            let names = asio.driver_names();

            ctx.enumerate_devices(|ty, info| {
                if matches!(ty, DeviceType::Playback) {
                    if let Some(index) = names.iter().position(|name| name == info.name()) {
                        println!("{}. {}", index + 1, info.name());
                    }
                }

                EnumerateControl::Continue
            })?;

            Ok(())
        }

        "play" => {
            // Query names only. Do not load drivers to query capabilities.
            let asio = asio_sys::Asio::new();
            let names = asio.driver_names();

            if names.is_empty() {
                return Err(MaudioError::other("No ASIO drivers found"));
            }

            println!("Enter the output device ID from the enumerate command:");

            let name = loop {
                let mut line = String::new();

                if std::io::stdin().read_line(&mut line)? == 0 {
                    return Err(MaudioError::other("No device ID provided"));
                }

                let Ok(number) = line.trim().parse::<usize>() else {
                    println!("Not a valid number.");
                    continue;
                };

                let Some(name) = number.checked_sub(1).and_then(|index| names.get(index)) else {
                    println!("ID {number} out of range.");
                    continue;
                };

                break name;
            };

            // Construct the same custom ID used by enumeration.
            let info = DeviceInfoBuilder::from_name(name)?.build();
            let device_id = info.device_id();

            println!("Initializing ASIO backend on output: {name}");

            let engine = EngineBuilder::new()
                .device_id(&device_id)
                .no_auto_start(true)
                .custom_backend::<AsioBackend>([Backend::Custom])
                .build()?;

            drop(engine);

            Ok(())
        }

        _ => Err(MaudioError::other(
            "Unknown command. Expected enumerate or play",
        )),
    }
}
