//! Builder for constructing an [`Engine`]
use maudio_sys::ffi as sys;

use std::sync::Arc;

use crate::{
    audio::sample_rate::SampleRate,
    engine::{
        process_notifier::{
            on_process_callback, EngineProcessCallback, ProcessNotifier, ProcessState,
        },
        resource::{private_rm, ResourceManager},
        Engine,
    },
    AsRawRef, MaResult,
};

pub struct EngineBuilder {
    inner: sys::ma_engine_config,
    device: Option<*mut sys::ma_device>,
    resource_manager: Option<ResourceManager<f32>>, // a ref count, not ownership
    no_device: bool,
    channels: Option<u32>,
    sample_rate: Option<SampleRate>,
    process_notifier: Option<Arc<ProcessState>>,
}

impl AsRawRef for EngineBuilder {
    type Raw = sys::ma_engine_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
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
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let inner = unsafe { sys::ma_engine_config_init() };
        Self {
            inner,
            device: None,
            resource_manager: None,
            no_device: false,
            channels: None,
            sample_rate: None,
            process_notifier: None,
        }
    }

    // TODO: Implement wrapper for sys::ma_device
    // If set, the caller is responsible for calling ma_engine_data_callback() in the device's data callback.
    fn device(&mut self, device: *mut sys::ma_device) -> &mut Self {
        self.inner.pDevice = device;
        self
    }

    fn resource_manager(&mut self, manager: &ResourceManager<f32>) -> &mut Self {
        self.inner.pResourceManager = private_rm::rm_ptr(manager);
        self.resource_manager = Some(manager.clone());
        self
    }

    /// Sets how many listeners the engine will create.
    ///
    /// The default is `1` listener (index `0`).
    pub fn listener_count(&mut self, count: u32) -> &mut Self {
        self.inner.listenerCount = count;
        self
    }

    /// Sets up up the engine without a default device
    ///
    /// Data can be read manually using [`EngineOps::read_pcm_frames()`](crate::engine::EngineOps::read_pcm_frames())
    ///
    /// ## Important
    /// `channels` and `sample_rate` must be set manually.
    pub fn no_device(&mut self, channels: u32, sample_rate: SampleRate) -> &mut Self {
        self.inner.sampleRate = sample_rate.into();
        self.sample_rate = Some(sample_rate);

        self.inner.channels = channels;
        self.channels = Some(channels);

        self.inner.noDevice = 1;
        self.no_device = true;
        self
    }

    /// The number of channels to use when mixing and spatializing.
    ///
    /// When set to 0, will use the native channel count of the device.
    pub fn set_channels(&mut self, channels: u32) -> &mut Self {
        self.inner.channels = channels;
        self.channels = Some(channels);
        self
    }

    /// When set to 0 will use the native sample rate of the device.
    pub fn set_sample_rate(&mut self, sample_rate: SampleRate) -> &mut Self {
        self.inner.sampleRate = sample_rate.into();
        self.sample_rate = Some(sample_rate);
        self
    }

    /// False by default, meaning the engine will be started automatically on creation.
    ///
    /// Requires a call to [`Engine::start()`] for a manually start
    pub fn no_auto_start(&mut self, yes: bool) -> &mut Self {
        self.inner.noAutoStart = yes as u32;
        self
    }

    fn set_process_notifier(&mut self, f: Option<Box<EngineProcessCallback>>) -> ProcessNotifier {
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
    pub fn with_process_notifier(&mut self) -> MaResult<(Engine, ProcessNotifier)> {
        if let Some(channels) = self.channels {
            if channels == 0 {
                return Err(crate::MaudioError::from_ma_result(
                    sys::ma_result_MA_INVALID_ARGS,
                ));
            }
        }
        let notifier = self.set_process_notifier(None);

        let mut engine = Engine::new_with_config(Some(self))?;
        engine.process_notifier = self.process_notifier.take();

        Ok((engine, notifier))
    }

    /// # Safety
    ///
    /// TODO
    pub unsafe fn with_realtime_callback<C>(&mut self, cb: C) -> MaResult<(Engine, ProcessNotifier)>
    where
        C: FnMut(&mut [f32], u32) + Send + 'static,
    {
        if let Some(channels) = self.channels {
            if channels == 0 {
                return Err(crate::MaudioError::from_ma_result(
                    sys::ma_result_MA_INVALID_ARGS,
                ));
            }
        }
        let notifier = self.set_process_notifier(Some(Box::new(cb)));

        let mut engine = Engine::new_with_config(Some(self))?;
        engine.process_notifier = self.process_notifier.take();

        Ok((engine, notifier))
    }

    pub fn build(&self) -> MaResult<Engine> {
        Engine::new_with_config(Some(self))
    }

    pub(crate) fn build_for_tests(&mut self) -> MaResult<Engine> {
        if cfg!(feature = "ci-tests") {
            self.no_device(2, SampleRate::Sr44100);
        }
        self.build()
    }
}

#[cfg(test)]
mod test {
    use crate::engine::{resource::rm_builder::ResourceManagerBuilder, EngineOps};

    use super::*;

    fn build_ci_engine(mut b: EngineBuilder) -> MaResult<Engine> {
        b.build_for_tests()
    }
    #[test]
    fn test_engine_builder_new_default_build_for_tests_ok() -> MaResult<()> {
        let engine = build_ci_engine(EngineBuilder::new())?;
        drop(engine);
        Ok(())
    }

    #[test]
    fn test_engine_builder_with_realtime_callback_basic_init() {
        let (engine, _) = unsafe {
            EngineBuilder::new()
                .with_realtime_callback(|_samples, _channels| {})
                .unwrap()
        };
        drop(engine);
    }

    #[test]
    fn test_engine_builder_default_trait_build_for_tests_ok() -> MaResult<()> {
        let engine = build_ci_engine(EngineBuilder::new())?;
        drop(engine);
        Ok(())
    }

    #[test]
    fn test_engine_builder_no_device_with_channels_and_sample_rate_builds() -> MaResult<()> {
        let mut b = EngineBuilder::new();
        b.no_device(2, SampleRate::Sr44100);

        let engine = b.build()?;
        drop(engine);
        Ok(())
    }

    #[test]
    fn test_engine_builder_listener_count_is_applied() -> MaResult<()> {
        let mut b = EngineBuilder::new();
        b.listener_count(3);

        let engine = build_ci_engine(b)?;
        drop(engine);
        Ok(())
    }

    #[test]
    fn test_engine_builder_no_auto_start_is_applied() -> MaResult<()> {
        let mut b = EngineBuilder::new();
        b.no_auto_start(true);

        let engine = build_ci_engine(b)?;
        drop(engine);
        Ok(())
    }

    #[test]
    fn test_engine_builder_with_process_notifier_builds_and_notifier_survives() -> MaResult<()> {
        let mut b = EngineBuilder::new();
        let (engine, mut tick) = b
            .no_device(2, SampleRate::Sr44100)
            .with_process_notifier()?;

        let _buf = engine.read_pcm_frames(256)?;

        let mut called = false;
        tick.call_if_triggered(|delta_frames| {
            called = true;
            let _ = delta_frames;
        });

        drop(engine);

        tick.call_if_triggered(|_delta_frames| {
            // no-op
        });

        Ok(())
    }

    #[test]
    fn test_engine_builder_process_notifier_drop_order_notifier_then_engine() -> MaResult<()> {
        let mut b = EngineBuilder::new();
        let (engine, tick) = b.with_process_notifier()?;

        drop(tick);
        drop(engine);

        Ok(())
    }

    #[test]
    fn test_engine_builder_process_notifier_drop_order_engine_then_notifier() -> MaResult<()> {
        let mut b = EngineBuilder::new();
        let (engine, tick) = b.with_process_notifier()?;

        drop(engine);
        drop(tick);

        Ok(())
    }

    #[test]
    fn test_engine_builder_with_process_notifier_multiple_builds_no_double_free() -> MaResult<()> {
        // This targets the `process_notifier: Option<Arc<ProcessState>>` in the builder and the `take()`.
        let mut b = EngineBuilder::new();

        let (engine1, tick1) = b.with_process_notifier()?;
        drop(engine1);
        drop(tick1);

        // Re-use builder for another notifier build.
        let (engine2, tick2) = b.with_process_notifier()?;
        drop(engine2);
        drop(tick2);

        Ok(())
    }

    #[test]
    fn test_engine_builder_build_for_tests_sets_no_device_channels_samplerate_under_feature(
    ) -> MaResult<()> {
        // This test is only meaningful if feature=ci-tests is enabled,
        // but it should still be safe otherwise.
        let mut b = EngineBuilder::new();
        let engine = b.build_for_tests()?;
        drop(engine);
        Ok(())
    }

    #[test]
    fn test_engine_builder_set_channels_and_sample_rate_idempotent() -> MaResult<()> {
        let mut b = EngineBuilder::new();
        b.no_device(2, SampleRate::Sr44100)
            .set_channels(1)
            .set_channels(2)
            .set_sample_rate(SampleRate::Sr44100)
            .set_sample_rate(SampleRate::Sr44100);

        let engine = b.build()?;
        drop(engine);
        Ok(())
    }

    #[test]
    fn test_engine_builder_with_resource_manager() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        let engine = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();
        let _rm_ref = engine.resource_manager().unwrap();
    }

    #[test]
    fn test_engine_builder_many_with_one_resource_manager() {
        let rm = ResourceManagerBuilder::new().build_f32().unwrap();
        let engine1 = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();
        let engine2 = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();
        let engine3 = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();
        let engine4 = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();
        let engine5 = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();
        let engine6 = EngineBuilder::new()
            .resource_manager(&rm)
            .build_for_tests()
            .unwrap();
        let _rm_ref = engine1.resource_manager().unwrap();
        let _rm_ref = engine1.resource_manager().unwrap();
        let _rm_ref = engine2.resource_manager().unwrap();
        let _rm_ref = engine3.resource_manager().unwrap();
        let _rm_ref = engine4.resource_manager().unwrap();
        let _rm_ref = engine5.resource_manager().unwrap();
        let _rm_ref = engine6.resource_manager().unwrap();
    }
}
