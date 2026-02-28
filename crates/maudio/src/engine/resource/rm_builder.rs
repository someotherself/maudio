use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    engine::resource::{rm_flags::RmFlags, ResourceManager},
    pcm_frames::{S24Packed, S24},
    AsRawRef, MaResult,
};

/// At the end, you will set the sample format that audio decoded by
/// this `ResourceManager` will be converted to.
/// The audio will decoded from its native sample format.
pub struct ResourceManagerBuilder {
    inner: sys::ma_resource_manager_config,
    format: Option<Format>,
    channels: Option<u32>,
    sample_rate: Option<SampleRate>,
    flags: RmFlags,
}

impl AsRawRef for ResourceManagerBuilder {
    type Raw = sys::ma_resource_manager_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl ResourceManagerBuilder {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let inner = unsafe { sys::ma_resource_manager_config_init() };
        Self {
            inner,
            format: None,
            channels: None,
            sample_rate: None,
            flags: RmFlags::NONE,
        }
    }

    /// Sets the number of channels that all audio decoded by this ResourceManager
    /// will be converted to.
    ///
    /// By default, audio keeps its original channel count. If set, all resources
    /// loaded through this ResourceManager will be decoded to this channel count
    /// at load time.
    pub fn channels(&mut self, channels: u32) -> &mut Self {
        self.inner.decodedChannels = channels;
        self.channels = Some(channels);
        self
    }

    fn set_format(&mut self, format: Format) -> &mut Self {
        self.inner.decodedFormat = format.into();
        self.format = Some(format);
        self
    }

    /// Sets the [`SampleRate`] that audio decoded by this `ResourceManager`
    /// will be converted to.
    ///
    /// By default, audio is decoded at its original sample rate. If set,
    /// all resources loaded through this `ResourceManager` will be resampled
    /// to this rate during decoding.
    pub fn sample_rate(&mut self, sample_rate: SampleRate) -> &mut Self {
        self.inner.decodedSampleRate = sample_rate.into();
        self.sample_rate = Some(sample_rate);
        self
    }

    /// Sets the [`RmFlags`]. Removes any existing ones.
    fn flags(&mut self, flags: RmFlags) -> &mut Self {
        self.inner.flags = flags.bits();
        self
    }

    fn non_blocking(&mut self, yes: bool) -> &mut Self {
        let mut flags = RmFlags::from_bits(self.inner.flags);
        if yes {
            flags.insert(RmFlags::NON_BLOCKING);
        } else {
            flags.remove(RmFlags::NON_BLOCKING);
        }
        self.inner.flags = flags.bits();
        self.flags = flags;
        self
    }

    /// Also sets the job_thread_count to 0
    pub fn no_threading(&mut self, yes: bool) -> &mut Self {
        let mut flags = RmFlags::from_bits(self.inner.flags);
        if yes {
            flags.insert(RmFlags::NO_THREADING);
        } else {
            flags.remove(RmFlags::NO_THREADING);
        }
        self.inner.flags = flags.bits();
        self.flags = flags;
        self
    }

    pub fn job_thread_count(&mut self, count: u32) -> &mut Self {
        self.inner.jobThreadCount = count;
        self
    }

    pub fn build_u8(&mut self) -> MaResult<ResourceManager<u8>> {
        self.set_format(Format::U8);
        ResourceManager::<u8>::new_with_config(self)
    }

    pub fn build_i16(&mut self) -> MaResult<ResourceManager<i16>> {
        self.set_format(Format::S16);
        ResourceManager::new_with_config(self)
    }

    pub fn build_i32(&mut self) -> MaResult<ResourceManager<i32>> {
        self.set_format(Format::S32);
        ResourceManager::new_with_config(self)
    }

    pub fn build_s24_packed(&mut self) -> MaResult<ResourceManager<S24Packed>> {
        self.set_format(Format::S24);
        ResourceManager::new_with_config(self)
    }

    pub fn build_s24(&mut self) -> MaResult<ResourceManager<S24>> {
        self.set_format(Format::S24);
        ResourceManager::new_with_config(self)
    }

    pub fn build_f32(&mut self) -> MaResult<ResourceManager<f32>> {
        self.set_format(Format::F32);
        ResourceManager::<f32>::new_with_config(self)
    }
}
