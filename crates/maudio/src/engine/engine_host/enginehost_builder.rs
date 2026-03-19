use crate::{
    audio::sample_rate::SampleRate,
    device::Device,
    engine::{engine_builder::EngineBuilder, engine_host::Host, resource::ResourceManager},
    MaResult,
};

pub struct EngineHostBuilder {
    builder: EngineBuilder,
}

impl EngineHostBuilder {
    pub fn new() -> Self {
        let builder = EngineBuilder::new();
        Self { builder }
    }

    // If set, the caller is responsible for calling ma_engine_data_callback() in the device's data callback.
    fn device(&mut self, device: Device) -> &mut Self {
        self.builder.device(device);
        self
    }

    pub fn resource_manager(&mut self, manager: &ResourceManager<f32>) -> &mut Self {
        self.builder.resource_manager(manager);
        self
    }

    /// Sets how many listeners the engine will create.
    ///
    /// The default is `1` listener (index `0`).
    pub fn listener_count(&mut self, count: u32) -> &mut Self {
        self.builder.listener_count(count);
        self
    }

    /// Sets up up the engine without a default device
    ///
    /// Data can be read manually using [`EngineOps::read_pcm_frames()`](crate::engine::EngineOps::read_pcm_frames())
    pub fn no_device(&mut self, channels: u32, sample_rate: SampleRate) -> &mut Self {
        self.builder.no_device(channels, sample_rate);
        self
    }

    /// The number of channels to use when mixing and spatializing.
    ///
    /// When set to 0, will use the native channel count of the device.
    pub fn set_channels(&mut self, channels: u32) -> &mut Self {
        self.builder.set_channels(channels);
        self
    }

    /// When set to 0 will use the native sample rate of the device.
    pub fn set_sample_rate(&mut self, sample_rate: SampleRate) -> &mut Self {
        self.builder.set_sample_rate(sample_rate);
        self
    }

    /// False by default, meaning the engine will be started automatically on creation.
    ///
    /// Requires a call to [`Engine::start()`] for a manually start
    pub fn no_auto_start(&mut self, yes: bool) -> &mut Self {
        self.builder.no_auto_start(yes);
        self
    }

    // TODO Doc: Resets the builder on failure
    pub fn build(&mut self) -> MaResult<Host> {
        let mut builder = std::mem::replace(&mut self.builder, EngineBuilder::new());
        Host::spawn_with(move || builder.build())
    }

    // TODO Doc: Resets the builder on failure
    pub fn with_realtime_callback<C>(&mut self, cb: C) -> MaResult<Host>
    where
        C: FnMut(&mut [f32], u32) + Send + 'static,
    {
        let mut builder = std::mem::replace(&mut self.builder, EngineBuilder::new());
        Host::spawn_with(move || builder.with_realtime_callback(cb))
    }

    pub fn with_process_notifier(self) -> MaResult<Host> {
        let mut builder = self.builder;
        Host::spawn_with(move || builder.with_process_notifier())
    }
}
