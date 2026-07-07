use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{audio::sample_rate::SampleRate, pcm_frames::PcmFormat, Binding, MaResult};

pub struct Delay<F: PcmFormat> {
    inner: *mut sys::ma_delay,
    channels: u32,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for Delay<F> {}

impl<F: PcmFormat> Binding for Delay<F> {
    type Raw = *mut sys::ma_delay;

    /// !!! unimplemented !!!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Delay<F> {
    fn build(config: &sys::ma_delay_config) -> MaResult<Delay<F>> {
        let channels = config.channels;
        let mut inner: MaybeUninit<sys::ma_delay> = MaybeUninit::uninit();
        delay_ffi::ma_delay_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));
        Ok(Delay {
            inner: inner_ptr,
            channels,
            _format: PhantomData,
        })
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        delay_ffi::ma_delay_process_pcm_frames(self, frames_out, frames_in)
    }

    pub fn set_wet(&mut self, wet: f32) {
        delay_ffi::ma_delay_set_wet(self, wet)
    }

    pub fn get_wet(&self) -> f32 {
        delay_ffi::ma_delay_get_wet(self)
    }

    pub fn set_dry(&mut self, dry: f32) {
        delay_ffi::ma_delay_set_dry(self, dry)
    }

    pub fn get_dry(&self) -> f32 {
        delay_ffi::ma_delay_get_dry(self)
    }

    pub fn set_decay(&mut self, decay: f32) {
        delay_ffi::ma_delay_set_decay(self, decay)
    }

    pub fn get_decay(&self) -> f32 {
        delay_ffi::ma_delay_get_decay(self)
    }
}

pub struct DelayBuilder {
    config: sys::ma_delay_config,
}

impl DelayBuilder {
    pub fn new(channels: u32, sample_rate: SampleRate, delay_frames: u32, decay: f32) -> Self {
        let config =
            unsafe { sys::ma_delay_config_init(channels, sample_rate.into(), delay_frames, decay) };
        Self { config }
    }

    pub fn build_f32(&mut self) -> MaResult<Delay<f32>> {
        Delay::<f32>::build(&self.config)
    }
}

pub(crate) mod delay_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::dsp::delay_effect::Delay, engine::AllocationCallbacks, pcm_frames::PcmFormat,
        AsRawRef, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_delay_init(
        config: &sys::ma_delay_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        delay: *mut sys::ma_delay,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_delay_init(config as *const _, alloc_cb, delay) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_delay_uninit<F: PcmFormat>(delay: &mut Delay<F>) {
        unsafe {
            sys::ma_delay_uninit(delay.to_raw(), std::ptr::null_mut());
        };
    }

    #[inline]
    pub fn ma_delay_process_pcm_frames<F: PcmFormat>(
        delay: &mut Delay<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = delay.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_delay_process_pcm_frames(
                delay.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u32,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_delay_set_wet<F: PcmFormat>(delay: &mut Delay<F>, wet: f32) {
        unsafe { sys::ma_delay_set_wet(delay.to_raw(), wet) }
    }

    #[inline]
    pub fn ma_delay_get_wet<F: PcmFormat>(delay: &Delay<F>) -> f32 {
        unsafe { sys::ma_delay_get_wet(delay.to_raw()) }
    }

    #[inline]
    pub fn ma_delay_set_dry<F: PcmFormat>(delay: &mut Delay<F>, dry: f32) {
        unsafe { sys::ma_delay_set_dry(delay.to_raw(), dry) }
    }

    #[inline]
    pub fn ma_delay_get_dry<F: PcmFormat>(delay: &Delay<F>) -> f32 {
        unsafe { sys::ma_delay_get_dry(delay.to_raw()) }
    }

    #[inline]
    pub fn ma_delay_set_decay<F: PcmFormat>(delay: &mut Delay<F>, decay: f32) {
        unsafe { sys::ma_delay_set_decay(delay.to_raw(), decay) }
    }

    #[inline]
    pub fn ma_delay_get_decay<F: PcmFormat>(delay: &Delay<F>) -> f32 {
        unsafe { sys::ma_delay_get_decay(delay.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for Delay<F> {
    fn drop(&mut self) {
        delay_ffi::ma_delay_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const CHANNELS: u32 = 2;
    const SAMPLE_RATE: SampleRate = SampleRate::Sr44100;
    const DELAY_FRAMES: u32 = 128;
    const DECAY: f32 = 0.5;

    #[test]
    fn delay_filter_test_build_f32() -> MaResult<()> {
        let delay = DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        assert!(!delay.to_raw().is_null());
        assert_eq!(delay.channels, CHANNELS);

        Ok(())
    }

    #[test]
    fn delay_filter_test_process_pcm_frames_f32() -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        let frames_in = [
            0.0_f32, 0.0_f32, 0.25_f32, -0.25_f32, 0.5_f32, -0.5_f32, 0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        delay.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }

    #[test]
    fn delay_filter_test_process_pcm_frames_silence_f32_stays_silent() -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        let frames_in = [0.0_f32; 16];
        let mut frames_out = [1.0_f32; 16];

        delay.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out, [0.0_f32; 16]);

        Ok(())
    }

    #[test]
    fn delay_filter_test_set_get_wet() -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        delay.set_wet(0.25);

        assert_eq!(delay.get_wet(), 0.25);

        Ok(())
    }

    #[test]
    fn delay_filter_test_set_get_dry() -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        delay.set_dry(0.75);

        assert_eq!(delay.get_dry(), 0.75);

        Ok(())
    }

    #[test]
    fn delay_filter_test_set_get_decay() -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        delay.set_decay(0.35);

        assert_eq!(delay.get_decay(), 0.35);

        Ok(())
    }

    #[test]
    fn delay_filter_test_set_get_wet_via_ffi_matches_method() -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        delay.set_wet(0.4);

        assert_eq!(delay.get_wet(), delay_ffi::ma_delay_get_wet(&delay));

        Ok(())
    }

    #[test]
    fn delay_filter_test_set_get_dry_via_ffi_matches_method() -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        delay.set_dry(0.6);

        assert_eq!(delay.get_dry(), delay_ffi::ma_delay_get_dry(&delay));

        Ok(())
    }

    #[test]
    fn delay_filter_test_set_get_decay_via_ffi_matches_method() -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        delay.set_decay(0.8);

        assert_eq!(delay.get_decay(), delay_ffi::ma_delay_get_decay(&delay));

        Ok(())
    }

    #[test]
    fn delay_filter_test_process_after_setters() -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        delay.set_wet(0.5);
        delay.set_dry(0.5);
        delay.set_decay(0.25);

        let frames_in = [
            0.0_f32, 0.0_f32, 0.25_f32, -0.25_f32, 0.5_f32, -0.5_f32, 0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        delay.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }

    #[test]
    fn delay_filter_test_delay_line_outputs_dry_signal_when_wet_is_zero() -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        delay.set_wet(0.0);
        delay.set_dry(1.0);

        assert_eq!(delay.get_wet(), 0.0);
        assert_eq!(delay.get_dry(), 1.0);

        let frames_in = [
            0.0_f32, 0.0_f32, 0.25_f32, -0.25_f32, 0.5_f32, -0.5_f32, 0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        delay.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }

    #[test]
    fn delay_filter_test_delay_line_outputs_no_immediate_wet_signal_when_dry_is_zero(
    ) -> MaResult<()> {
        let mut delay =
            DelayBuilder::new(CHANNELS, SAMPLE_RATE, DELAY_FRAMES, DECAY).build_f32()?;

        delay.set_wet(1.0);
        delay.set_dry(0.0);

        let frames_in = [
            0.0_f32, 0.0_f32, 0.25_f32, -0.25_f32, 0.5_f32, -0.5_f32, 0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [1.0_f32; 8];

        delay.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out, [0.0_f32; 8]);

        Ok(())
    }
}
