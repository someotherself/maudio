use asio_sys::{Asio, AsioSampleType, BufferCallbackId, Driver};
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
    MaResult, MaudioError,
};

// Logging helpers
fn post(log: Option<&LogRef>, level: LogLevel, message: &str) {
    if let Some(log) = log.as_ref() {
        let _ = log.post(level, message);
    }
}

fn fail(log: Option<&LogRef>, message: &str) -> MaudioError {
    if let Some(log) = log.as_ref() {
        let _ = log.post(LogLevel::Error, message);
    };
    MaudioError::invalid_backend()
}

struct AsioBackend;

/// ASIO can only represent one opened maudio device
/// Whether is playback, capture or duplex depends on streams opened
struct AsioDriver {
    driver: Driver,
    callback_id: BufferCallbackId,
}

// We need to explicitly remove the callbacks
impl Drop for AsioDriver {
    fn drop(&mut self) {
        self.driver.remove_callback(self.callback_id);
    }
}

/// Helper to report the asio configuration to maudio
fn report_stream_spec(
    sample_rate: u32,
    buffer_size: usize,
    channels: u32,
    sample_type: &AsioSampleType,
    descriptor: &mut DeviceDescriptor,
) -> MaResult<()> {
    descriptor.channels = Some(channels);
    descriptor.sample_rate = Some(sample_rate.try_into()?);
    descriptor.period_size_frames = buffer_size as u32;
    default_channel_map_into(&mut descriptor.channel_map, Some(target_channel_position()));
    descriptor.format = direct_asio_format(sample_type)?;

    Ok(())
}

/// Helper to calculate the buffer size, if the user requests a specific one
fn requested_asio_buffer_size(
    config: &BackendDeviceConfig,
    sample_rate: u32,
) -> MaResult<Option<i32>> {
    let frames = if config.period_size_frames != 0 {
        u64::from(config.period_size_frames)
    } else if config.period_size_millis != 0 {
        // Round up so the period is at least the requested duration.
        let numerator = u64::from(config.period_size_millis) * u64::from(sample_rate);
        numerator / 1000 + u64::from(numerator % 1000 != 0)
    } else {
        return Ok(None); // ASIO's preferred buffer size
    };

    let frames =
        i32::try_from(frames).map_err(|_| MaudioError::other("ASIO buffer size is too large"))?;

    if frames == 0 {
        return Err(MaudioError::other("ASIO buffer size is zero"));
    }

    Ok(Some(frames))
}

/// Helper to convert between asio and maudio sample formats
/// We reject formats not supported by the miniaudio device
fn direct_asio_format(sample_type: &asio_sys::AsioSampleType) -> MaResult<Format> {
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

/// Function responsible for creating a playback AsioDriver
/// Either opens an AsioStream, or re-uses an existing one
fn create_playback_driver<'device>(
    context: &Asio,
    device: BackendDeviceHandle<'device, AsioBackend>,
    config: &BackendDeviceConfig,
    playback: Option<&mut DeviceDescriptor>,
    log: Option<&LogRef<'_>>,
) -> MaResult<AsioDriver> {
    let descriptor = playback.ok_or_else(|| fail(log, "Missing capture descriptor"))?;

    let Some(device_id) = descriptor.device_id.as_ref() else {
        return Err(fail(log, "A device id must be provided."));
    };

    let Some(name) = device_id.get_custom_name() else {
        return Err(fail(log, "A device id must be provided."));
    };

    post(log, LogLevel::Debug, &format!("Opening device: {}.", name));

    let driver = match context.load_driver(&name) {
        Ok(driver) => driver,
        Err(e) => {
            return Err(fail(log, &format!("Could not load device: {}.", e)));
        }
    };

    let asio_streams = driver.streams();
    let Ok(mut streams) = asio_streams.lock() else {
        return Err(fail(log, "Stream lock poisoned"));
    };

    let channels = config.playback_channels.unwrap_or(2);

    let sample_rate = driver
        .sample_rate()
        .ok()
        .map(|s| s as u32)
        .or(config.sample_rate.map(|s| s.into()))
        .unwrap_or(44_100);
    let buffer_size = requested_asio_buffer_size(config, sample_rate)?;

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
        return Err(fail(log, "Could not create output stream"));
    };

    let sample_type = driver.output_data_type().map_err(MaudioError::other)?;

    post(log, LogLevel::Debug, &format!(
                    "ASIO playback stream opened on driver: {}.\nChannels: {channels}, sample rate: {sample_rate}, format: {sample_type:?}, buffer size: {frames}",
                    driver.name(),
                ));

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

    report_stream_spec(sample_rate, frames, channels, &sample_type, descriptor)?;

    Ok(AsioDriver {
        driver,
        callback_id,
    })
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

        interleaved.fill(F::STORE_SILENCE);
        playback_callback::<F>(handle.clone(), &mut interleaved);

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
            let buffers = channel_info.buffers;
            let ptr = buffers[buffer_index].cast::<F::StorageUnit>();
            if ptr.is_null() {
                return;
            }

            let channel = unsafe { std::slice::from_raw_parts_mut(ptr, frames) };

            for (frame_index, sample) in channel.iter_mut().enumerate() {
                *sample = interleaved[frame_index * channels + channel_index];
            }
        }
    })
}

/// Function responsible for creating a capture AsioDriver
/// Either opens an AsioStream, or re-uses an existing one
fn create_capture_driver<'device>(
    context: &Asio,
    device: BackendDeviceHandle<'device, AsioBackend>,
    config: &BackendDeviceConfig,
    capture: Option<&mut DeviceDescriptor>,
    log: Option<&LogRef<'_>>,
) -> MaResult<AsioDriver> {
    let descriptor = capture.ok_or_else(|| fail(log, "Missing capture descriptor"))?;

    // TODO: How to handle picking a 'default' device?
    let Some(device_id) = descriptor.device_id.as_ref() else {
        return Err(fail(log, "A device id must be provided."));
    };

    let Some(name) = device_id.get_custom_name() else {
        return Err(fail(log, "A device id must be provided."));
    };

    let Ok(driver) = context.load_driver(&name) else {
        return Err(fail(log, "Could not load device."));
    };

    let asio_streams = driver.streams();
    let Ok(mut streams) = asio_streams.lock() else {
        return Err(fail(log, "Stream lock poisoned"));
    };

    let channels = config.capture_channels.unwrap_or(2);

    let sample_rate = driver
        .sample_rate()
        .ok()
        .map(|s| s as u32)
        .or(config.sample_rate.map(|s| s.into()))
        .unwrap_or(44_100);
    let buffer_size = requested_asio_buffer_size(config, sample_rate)?;

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
        return Err(fail(log, "Could not create input stream"));
    };

    let sample_type = driver.input_data_type().map_err(MaudioError::other)?;

    post(log, LogLevel::Debug, &format!(
                    "ASIO capture stream opened on driver: {}.\nChannels: {channels}, sample rate: {sample_rate}, format: {sample_type:?}, buffer size: {frames}",
                    driver.name(),
                ));

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

    report_stream_spec(sample_rate, frames, channels, &sample_type, descriptor)?;

    Ok(AsioDriver {
        driver,
        callback_id,
    })
}

fn playback_callback<F: MaSampleFormat>(
    handle: BackendDeviceHandle<'static, AsioBackend>,
    buffer: &mut [F::StorageUnit],
) {
    let _ = handle.handle_backend_data_callback::<F, F>(Some(buffer), None);
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

fn capture_callback<F: MaSampleFormat>(
    handle: BackendDeviceHandle<'static, AsioBackend>,
    buffer: &[F::StorageUnit],
) {
    let _ = handle.handle_backend_data_callback::<F, F>(None, Some(buffer));
}

/// Function responsible for creating a duplex AsioDriver
///
/// Either opens an AsioStream, or re-uses an existing one
///
/// Re-uses `playback_callback` and `capture_callback` functions
fn create_duplex_driver<'device>(
    context: &Asio,
    device: BackendDeviceHandle<'device, AsioBackend>,
    config: &BackendDeviceConfig,
    playback: Option<&mut DeviceDescriptor>,
    capture: Option<&mut DeviceDescriptor>,
    log: Option<&LogRef<'_>>,
) -> MaResult<AsioDriver> {
    let playback = playback.ok_or_else(|| fail(log, "Missing playback descriptor"))?;
    let capture = capture.ok_or_else(|| fail(log, "Missing capture descriptor"))?;

    let playback_id = playback
        .device_id
        .as_ref()
        .ok_or_else(|| fail(log, "A playback device ID is required"))?;

    let capture_id = capture
        .device_id
        .as_ref()
        .ok_or_else(|| fail(log, "A capture device ID is required"))?;

    if playback_id != capture_id {
        return Err(MaudioError::other(
            "ASIO duplex requires the same driver for playback and capture",
        ));
    }

    let name = playback_id
        .get_custom_name()
        .ok_or_else(|| fail(log, "Invalid ASIO device ID"))?;

    let driver = context.load_driver(&name).map_err(MaudioError::other)?;

    let playback_channels = config.playback_channels.unwrap_or(2);
    let capture_channels = config.capture_channels.unwrap_or(2);

    let available = driver.channels().map_err(MaudioError::other)?;

    if playback_channels == 0 || i64::from(playback_channels) > i64::from(available.outs) {
        return Err(MaudioError::other(
            "Requested playback channel count is unavailable",
        ));
    }

    if capture_channels == 0 || i64::from(capture_channels) > i64::from(available.ins) {
        return Err(MaudioError::other(
            "Requested capture channel count is unavailable",
        ));
    }

    // Use and report the driver's current rate for both directions.
    let rate = driver.sample_rate().map_err(MaudioError::other)?;
    if !rate.is_finite() || rate <= 0.0 || rate > u32::MAX as f64 {
        return Err(MaudioError::other("Invalid ASIO sample rate"));
    }
    let sample_rate = rate as u32;

    let playback_type = driver.output_data_type().map_err(MaudioError::other)?;
    let capture_type = driver.input_data_type().map_err(MaudioError::other)?;

    // Reject unsupported formats before preparing streams.
    direct_asio_format(&playback_type)?;
    direct_asio_format(&capture_type)?;

    let buffer_size = requested_asio_buffer_size(config, sample_rate)?;

    // TODO: Try to re-use existing streams
    let frames = {
        let asio_streams = driver.streams();
        let mut streams = asio_streams
            .lock()
            .map_err(|_| MaudioError::other("Stream lock poisoned"))?;

        if streams.input.is_some() || streams.output.is_some() {
            return Err(MaudioError::other(
                "ASIO driver already has prepared streams",
            ));
        }

        // Prepare input, then recreate the buffers with output added.
        *streams = driver
            .prepare_input_stream(None, capture_channels as usize, buffer_size)
            .map_err(MaudioError::other)?;

        let input = streams
            .input
            .take()
            .ok_or_else(|| fail(log, "Missing prepared input stream"))?;

        let shared_buffer_size = input.buffer_size;

        *streams = driver
            .prepare_output_stream(
                Some(input),
                playback_channels as usize,
                Some(shared_buffer_size),
            )
            .map_err(MaudioError::other)?;

        let input = streams
            .input
            .as_ref()
            .ok_or_else(|| fail(log, "Missing duplex input stream"))?;
        let output = streams
            .output
            .as_ref()
            .ok_or_else(|| fail(log, "Missing duplex output stream"))?;

        if input.buffer_size <= 0
            || input.buffer_size != output.buffer_size
            || input.buffer_infos.len() != capture_channels as usize
            || output.buffer_infos.len() != playback_channels as usize
        {
            return Err(fail(
                log,
                "Prepared ASIO duplex streams do not match the requested configuration",
            ));
        }

        input.buffer_size as usize
    };

    let message = format!(
                    "ASIO duplex stream opened on driver: {}.\n
                    Playback channels: {playback_channels}, Capture channels: {capture_channels},\n
                    sample rate: {sample_rate}, playback format: {:?}, capture format: {:?}, buffer size: {frames}",
                    driver.name(), playback_type, capture_type,
                );
    post(log, LogLevel::Info, &message);

    report_stream_spec(
        sample_rate,
        frames,
        playback_channels,
        &playback_type,
        playback,
    )?;

    report_stream_spec(
        sample_rate,
        frames,
        capture_channels,
        &capture_type,
        capture,
    )?;

    // SAFETY: The backend must remove the callback and ensure any invocation
    // has completed before the maudio device is destroyed.
    let device = unsafe { device.clone_static_unchecked() };

    use AsioSampleType::{ASIOSTFloat32LSB, ASIOSTInt16LSB, ASIOSTInt32LSB};

    let register = match (&playback_type, &capture_type) {
        (ASIOSTFloat32LSB, ASIOSTFloat32LSB) => register_duplex_callback::<f32, f32>,
        (ASIOSTFloat32LSB, ASIOSTInt16LSB) => register_duplex_callback::<f32, i16>,
        (ASIOSTFloat32LSB, ASIOSTInt32LSB) => register_duplex_callback::<f32, i32>,

        (ASIOSTInt16LSB, ASIOSTFloat32LSB) => register_duplex_callback::<i16, f32>,
        (ASIOSTInt16LSB, ASIOSTInt16LSB) => register_duplex_callback::<i16, i16>,
        (ASIOSTInt16LSB, ASIOSTInt32LSB) => register_duplex_callback::<i16, i32>,

        (ASIOSTInt32LSB, ASIOSTFloat32LSB) => register_duplex_callback::<i32, f32>,
        (ASIOSTInt32LSB, ASIOSTInt16LSB) => register_duplex_callback::<i32, i16>,
        (ASIOSTInt32LSB, ASIOSTInt32LSB) => register_duplex_callback::<i32, i32>,

        _ => return Err(fail(log, "Unsupported ASIO duplex formats")),
    };

    let callback_id = register(
        device,
        &driver,
        frames,
        playback_channels as usize,
        capture_channels as usize,
    );

    Ok(AsioDriver {
        driver,
        callback_id,
    })
}

fn register_duplex_callback<P: MaSampleFormat, C: MaSampleFormat>(
    handle: BackendDeviceHandle<'static, AsioBackend>,
    driver: &asio_sys::Driver,
    frames: usize,
    playback_channels: usize,
    capture_channels: usize,
) -> BufferCallbackId
where
    P::StorageUnit: 'static + Send,
    C::StorageUnit: 'static + Send,
{
    let streams = driver.streams();

    let mut playback = vec![P::STORE_SILENCE; frames * playback_channels];
    let mut capture = vec![C::STORE_SILENCE; frames * capture_channels];

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

            if input.buffer_size as usize != frames || input.buffer_infos.len() != capture_channels
            {
                return;
            }

            for (channel_index, channel_info) in input.buffer_infos.iter().enumerate() {
                let buffers = channel_info.buffers;
                let ptr = buffers[buffer_index].cast::<C::StorageUnit>();
                if ptr.is_null() {
                    return;
                }

                // SAFETY: Preparation established a readable input buffer
                // containing `frames` samples of the selected capture format.
                let channel = unsafe { std::slice::from_raw_parts(ptr.cast_const(), frames) };

                for (frame_index, &sample) in channel.iter().enumerate() {
                    capture[frame_index * capture_channels + channel_index] = sample;
                }
            }
        }

        playback.fill(P::STORE_SILENCE);

        if handle
            .handle_backend_data_callback::<P, C>(Some(&mut playback), Some(&capture))
            .is_err()
        {
            playback.fill(P::STORE_SILENCE);
        }

        {
            let Ok(streams) = streams.lock() else {
                return;
            };
            let Some(output) = streams.output.as_ref() else {
                return;
            };

            if output.buffer_size as usize != frames
                || output.buffer_infos.len() != playback_channels
            {
                return;
            }

            for (channel_index, channel_info) in output.buffer_infos.iter().enumerate() {
                let buffers = channel_info.buffers;
                let ptr = buffers[buffer_index].cast::<P::StorageUnit>();
                if ptr.is_null() {
                    return;
                }

                let channel = unsafe { std::slice::from_raw_parts_mut(ptr, frames) };

                for (frame_index, sample) in channel.iter_mut().enumerate() {
                    *sample = playback[frame_index * playback_channels + channel_index];
                }
            }
        }
    })
}

impl CustomBackend for AsioBackend {
    type Context = asio_sys::Asio;
    type Device<'device> = AsioDriver;

    fn init_context(log: Option<&LogRef>) -> maudio::MaResult<Self::Context> {
        post(
            log,
            LogLevel::Debug,
            "Attempting to initialize ASIO backend",
        );
        Ok(asio_sys::Asio::new())
    }

    fn context_query_device_info(
        context: &mut Self::Context,
        device_type: DeviceType,
        device_id: maudio::device::device_id::DeviceId,
        log: Option<&maudio::logging::LogRef>,
    ) -> maudio::MaResult<maudio::device::device_info::DeviceInfo> {
        if matches!(device_type, DeviceType::Loopback) {
            return Err(fail(log, "Loopback is not supported"));
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
        if config.device_type == DeviceType::Loopback {
            return Err(fail(log, "Loopback is not supported"));
        }

        let context = device.backend_context();

        if matches!(config.device_type, DeviceType::Duplex) {
            return create_duplex_driver(context, device.clone(), &config, playback, capture, log);
        }

        if matches!(config.device_type, DeviceType::Playback) {
            return create_playback_driver(context, device.clone(), &config, playback, log);
        }

        if matches!(config.device_type, DeviceType::Capture) {
            return create_capture_driver(context, device.clone(), &config, capture, log);
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
        let Some(dev) = device.backend_device() else {
            return Err(fail(log, "Backend device not available"));
        };

        if let Err(e) = dev.driver.start() {
            return Err(fail(log, &format!("Failed to start backend device: {}", e)));
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
        let Some(dev) = device.backend_device() else {
            return Err(fail(log, "Backend device not available"));
        };

        if let Err(e) = dev.driver.stop() {
            return Err(fail(log, &format!("Failed to stop backend device: {}", e)));
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
