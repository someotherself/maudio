use maudio_sys::ffi as sys;

use crate::{Binding, MaResult, audio::sample_rate::SampleRate, engine::Engine};

pub struct EngineBuilder {
    inner: sys::ma_engine_config,
    device: Option<*mut sys::ma_device>,
    resource_manager: Option<*mut sys::ma_resource_manager>,
    no_device: bool,
    channels: Option<u32>,
    sample_rate: Option<SampleRate>,
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

impl EngineBuilder {
    pub fn new() -> Self {
        let ptr = unsafe { sys::ma_engine_config_init() };
        Self::from_ptr(ptr)
    }

    // TODO: Implement wrapper for sys::ma_device
    // If set, the caller is responsible for calling ma_engine_data_callback() in the device's data callback.
    pub fn device(mut self, device: *mut sys::ma_device) -> Self {
        self.inner.pDevice = device;
        self
    }

    // TODO: Implement wrapper for sys::ma_device
    pub fn resource_manager(mut self, manager: *mut sys::ma_resource_manager) -> Self {
        self.inner.pResourceManager = manager;
        self.resource_manager = Some(manager);
        self
    }

    // Check that channels and sample rate are set manually if this is set to false?
    pub fn no_device(mut self, enabled: bool) -> Self {
        self.inner.noDevice = enabled as u32;
        self.no_device = enabled;
        self
    }

    pub fn set_channels(mut self, channels: u32) -> Self {
        self.inner.channels = channels;
        self.channels = Some(channels);
        self
    }

    pub fn set_sample_rate(mut self, sample_rate: SampleRate) -> Self {
        self.inner.sampleRate = sample_rate.into();
        self.sample_rate = Some(sample_rate);
        self
    }

    pub fn build(self) -> MaResult<Engine> {
        Engine::new_with_config(Some(&self))
    }
}
