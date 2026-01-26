use maudio_sys::ffi as sys;

use std::sync::Arc;

use crate::{
    Binding, MaResult,
    audio::sample_rate::SampleRate,
    engine::{
        Engine,
        process_notifier::{
            EngineProcessCallback, ProcessNotifier, ProcessState, on_process_callback,
        },
    },
};

pub struct EngineBuilder {
    inner: sys::ma_engine_config,
    device: Option<*mut sys::ma_device>,
    resource_manager: Option<*mut sys::ma_resource_manager>,
    no_device: bool,
    channels: Option<u32>,
    sample_rate: Option<SampleRate>,
    process_notifier: Option<Arc<ProcessState>>,
}

impl Binding for EngineBuilder {
    type Raw = sys::ma_engine_config;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            device: None,
            resource_manager: None,
            no_device: false,
            channels: None,
            sample_rate: None,
            process_notifier: None,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// TODO. To add:
// ma_mono_expansion_mode
// volumeSmoothTimeInPCMFrames
// ma_resampler_config
// periodSizeInFrames and periodSizeInMilliseconds
// gainSmoothTimeInFrames and gainSmoothTimeInMilliseconds
// defaultVolumeSmoothTimeInPCMFrames

impl EngineBuilder {
    pub fn new() -> Self {
        let ptr = unsafe { sys::ma_engine_config_init() };
        Self::from_ptr(ptr)
    }

    // TODO: Implement wrapper for sys::ma_device
    // If set, the caller is responsible for calling ma_engine_data_callback() in the device's data callback.
    fn device(mut self, device: *mut sys::ma_device) -> Self {
        self.inner.pDevice = device;
        self
    }

    // TODO: Implement wrapper for sys::ma_resource_manager
    fn resource_manager(mut self, manager: *mut sys::ma_resource_manager) -> Self {
        self.inner.pResourceManager = manager;
        self.resource_manager = Some(manager);
        self
    }

    /// Sets how many listeners the engine will create.
    ///
    /// The default is `1` listener (index `0`).
    pub fn listener_count(mut self, count: u32) -> Self {
        self.inner.listenerCount = count;
        self
    }

    /// Sets up up the engine without a default device
    ///
    /// Data can be read manually using [`EngineOps::read_pcm_frames()`](crate::engine::EngineOps::read_pcm_frames())
    ///
    /// ## Important
    /// `channels` and `sample_rate` must be set manually.
    pub fn no_device(mut self, enabled: bool) -> Self {
        self.inner.noDevice = enabled as u32;
        self.no_device = enabled;
        self
    }

    /// The number of channels to use when mixing and spatializing.
    ///
    /// When set to 0, will use the native channel count of the device.
    pub fn set_channels(mut self, channels: u32) -> Self {
        self.inner.channels = channels;
        self.channels = Some(channels);
        self
    }

    /// When set to 0 will use the native sample rate of the device.
    pub fn set_sample_rate(mut self, sample_rate: SampleRate) -> Self {
        self.inner.sampleRate = sample_rate.into();
        self.sample_rate = Some(sample_rate);
        self
    }

    /// False by default, meaning the engine will be started automatically on creation.
    ///
    /// Requires a call to [`Engine::start()`] for a manually start
    pub fn no_auto_start(mut self, yes: bool) -> Self {
        self.inner.noAutoStart = yes as u32;
        self
    }

    fn set_process_notifier(&mut self, f: Option<Box<EngineProcessCallback>>) -> ProcessNotifier {
        // TODO: Add close as optional param
        let channels = self.channels.unwrap_or(2);
        let notifier = ProcessNotifier::new(channels, f);

        self.process_notifier = Some(notifier.clone_flag());

        self.inner.pProcessUserData = notifier.as_user_data_ptr();
        self.inner.onProcess = Some(on_process_callback);

        notifier
    }

    /// Builds an [`Engine`] configured with a lightweight process “tick” notifier.
    ///
    /// Miniaudio doc:
    /// "Fired at the end of each call to ma_engine_read_pcm_frames() ([`EngineOps::read_pcm_frames()`](crate::engine::EngineOps::read_pcm_frames())).
    /// For engine's that manage their own internal device (the default configuration),
    /// this will be fired from the audio thread, and you do not need to call ma_engine_read_pcm_frames()
    /// manually in order to trigger this."
    ///
    /// This returns a [`ProcessNotifier`] that is updated from the engine's realtime
    /// processing callback (internally). Unlike a user-supplied realtime callback,
    /// the notifier lets you react to progress *outside* the audio thread by polling
    /// (e.g. from a UI loop, game loop, or control thread).
    ///
    /// ## Typical uses
    /// - Drive a UI or progress indicator by polling processed frames.
    /// - Perform control work when processing advances (start/stop, device switching,
    ///   submitting commands), without doing that work in the realtime callback.
    /// - Collect lightweight telemetry (frames processed per interval) from another
    ///   thread.
    ///
    /// ## Example
    /// Polling from a control loop:
    ///
    /// ```no_run
    /// # use std::time::Duration;
    /// # use maudio::engine::engine_builder::EngineBuilder;
    /// # use maudio::*;
    /// # fn main() -> maudio::MaResult<()> {
    /// let (engine, mut tick) = EngineBuilder::new().with_process_notifier()?;
    ///
    /// loop {
    ///     tick.call_if_triggered(|delta_frames| {
    ///         // Runs on this thread (not the audio thread).
    ///         // Safe place to update state, send messages, etc.
    ///         println!("processed {delta_frames} frames");
    ///     });
    ///
    ///     std::thread::sleep(Duration::from_millis(16));
    /// }
    /// Ok(())
    /// }
    /// ```
    ///
    // If you truly need to run a callback on the realtime thread, use [`EngineBuilder::with_realtime_callback()`].
    pub fn with_process_notifier(mut self) -> MaResult<(Engine, ProcessNotifier)> {
        let notifier = self.set_process_notifier(None);

        let mut engine = Engine::new_with_config(Some(&self))?;
        engine.process_notifier = self.process_notifier.take();

        Ok((engine, notifier))
    }

    unsafe fn with_realtime_callback(self) -> MaResult<(Engine, ProcessNotifier)> {
        // let notifier = self.set_process_notifier(Some(Box::new(f)));

        // let mut engine = Engine::new_with_config(Some(&self))?;
        // engine.process_notifier = self.process_notifier.take();

        // Ok((engine, notifier))
        todo!()
    }

    pub fn build(self) -> MaResult<Engine> {
        Engine::new_with_config(Some(&self))
    }

    pub(crate) fn build_for_tests(self) -> MaResult<Engine> {
        if cfg!(feature = "ci-tests") {
            self.no_device(true)
                .set_channels(2)
                .set_sample_rate(SampleRate::Sr44100)
                .build()
        } else {
            self.build()
        }
    }
}
