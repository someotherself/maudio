use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    pcm_frames::PcmFormat,
    Binding, MaResult,
};

pub struct LoShelf2<F: PcmFormat> {
    inner: *mut sys::ma_loshelf2,
    channels: u32,
    format: Format,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for LoShelf2<F> {}

impl<F: PcmFormat> Binding for LoShelf2<F> {
    type Raw = *mut sys::ma_loshelf2;

    /// !!! unimplemented !!!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> LoShelf2<F> {
    fn build(config: &sys::ma_loshelf2_config, format: Format) -> MaResult<LoShelf2<F>> {
        let channels = config.channels;
        let mut inner: MaybeUninit<sys::ma_loshelf2> = MaybeUninit::uninit();
        loshelf2_ffi::ma_loshelf2_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));
        Ok(LoShelf2 {
            inner: inner_ptr,
            format,
            channels,
            _format: PhantomData,
        })
    }

    pub fn reinit(
        &mut self,
        sample_rate: SampleRate,
        gain_db: f64,
        slope: f64,
        frequency: f64,
    ) -> MaResult<()> {
        let config = unsafe {
            sys::ma_loshelf2_config_init(
                self.format.into(),
                self.channels,
                sample_rate.into(),
                gain_db,
                slope,
                frequency,
            )
        };
        loshelf2_ffi::ma_loshelf2_reinit(&config, self)
    }

    pub fn get_latency(&self) -> u32 {
        loshelf2_ffi::ma_loshelf2_get_latency(self)
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        loshelf2_ffi::ma_loshelf2_process_pcm_frames(self, frames_out, frames_in)
    }
}

pub struct LoShelf2Builder {
    config: sys::ma_loshelf2_config,
}

impl LoShelf2Builder {
    pub fn new(
        channels: u32,
        sample_rate: SampleRate,
        slope: f64,
        gain_db: f64,
        frequency: f64,
    ) -> Self {
        let config = unsafe {
            sys::ma_loshelf2_config_init(
                Format::S16.into(),
                channels,
                sample_rate.into(),
                gain_db,
                slope,
                frequency,
            )
        };
        Self { config }
    }

    pub fn build_i16(&mut self) -> MaResult<LoShelf2<i16>> {
        self.config.format = Format::S16.into();
        LoShelf2::<i16>::build(&self.config, Format::S16)
    }

    pub fn build_f32(&mut self) -> MaResult<LoShelf2<f32>> {
        self.config.format = Format::F32.into();
        LoShelf2::<f32>::build(&self.config, Format::F32)
    }
}

mod loshelf2_ffi {
    use std::sync::Arc;

    use crate::{
        audio::dsp::filters::loshelf2_filter::LoShelf2, engine::AllocationCallbacks,
        pcm_frames::PcmFormat, AsRawRef, Binding, MaResult, MaudioError,
    };
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_loshelf2_init(
        config: &sys::ma_loshelf2_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        loshelf2: *mut sys::ma_loshelf2,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_loshelf2_init(config as *const _, alloc_cb, loshelf2) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_loshelf2_uninit<F: PcmFormat>(loshelf2: &mut LoShelf2<F>) {
        unsafe {
            sys::ma_loshelf2_uninit(loshelf2.to_raw(), std::ptr::null_mut());
        };
    }

    #[inline]
    pub fn ma_loshelf2_reinit<F: PcmFormat>(
        config: &sys::ma_loshelf2_config,
        loshelf2: &mut LoShelf2<F>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_loshelf2_reinit(config as *const _, loshelf2.to_raw()) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_loshelf2_process_pcm_frames<F: PcmFormat>(
        loshelf2: &mut LoShelf2<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = loshelf2.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_loshelf2_process_pcm_frames(
                loshelf2.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_loshelf2_get_latency<F: PcmFormat>(loshelf2: &LoShelf2<F>) -> u32 {
        unsafe { sys::ma_loshelf2_get_latency(loshelf2.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for LoShelf2<F> {
    fn drop(&mut self) {
        loshelf2_ffi::ma_loshelf2_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const CHANNELS: u32 = 2;
    const SAMPLE_RATE: SampleRate = SampleRate::Sr44100;
    const SLOPE: f64 = 1.0;
    const GAIN_DB: f64 = 6.0;
    const FREQUENCY: f64 = 1000.0;

    #[test]
    fn loshelf2_filter_test_build_i16() -> MaResult<()> {
        let loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_i16()?;

        assert!(!loshelf2.to_raw().is_null());
        assert_eq!(loshelf2.format, Format::S16);
        assert_eq!(loshelf2.channels, CHANNELS);

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_build_f32() -> MaResult<()> {
        let loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_f32()?;

        assert!(!loshelf2.to_raw().is_null());
        assert_eq!(loshelf2.format, Format::F32);
        assert_eq!(loshelf2.channels, CHANNELS);

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_get_latency_i16() -> MaResult<()> {
        let loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_i16()?;

        let latency = loshelf2.get_latency();

        assert_eq!(latency, loshelf2_ffi::ma_loshelf2_get_latency(&loshelf2));

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_get_latency_f32() -> MaResult<()> {
        let loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_f32()?;

        let latency = loshelf2.get_latency();

        assert_eq!(latency, loshelf2_ffi::ma_loshelf2_get_latency(&loshelf2));

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_process_pcm_frames_i16() -> MaResult<()> {
        let mut loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_i16()?;

        let frames_in = [
            0_i16, 0_i16, 1000_i16, -1000_i16, 2000_i16, -2000_i16, 3000_i16, -3000_i16,
        ];
        let mut frames_out = [0_i16; 8];

        loshelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_process_pcm_frames_f32() -> MaResult<()> {
        let mut loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_f32()?;

        let frames_in = [
            0.0_f32, 0.0_f32, 0.25_f32, -0.25_f32, 0.5_f32, -0.5_f32, 0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        loshelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_process_pcm_frames_silence_i16_stays_silent() -> MaResult<()> {
        let mut loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_i16()?;

        let frames_in = [0_i16; 16];
        let mut frames_out = [123_i16; 16];

        loshelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out, [0_i16; 16]);

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_process_pcm_frames_silence_f32_stays_silent() -> MaResult<()> {
        let mut loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_f32()?;

        let frames_in = [0.0_f32; 16];
        let mut frames_out = [1.0_f32; 16];

        loshelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out, [0.0_f32; 16]);

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_reinit_i16_keeps_cached_values() -> MaResult<()> {
        let mut loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_i16()?;

        loshelf2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        assert_eq!(loshelf2.format, Format::S16);
        assert_eq!(loshelf2.channels, CHANNELS);
        assert!(!loshelf2.to_raw().is_null());

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_reinit_f32_keeps_cached_values() -> MaResult<()> {
        let mut loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_f32()?;

        loshelf2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        assert_eq!(loshelf2.format, Format::F32);
        assert_eq!(loshelf2.channels, CHANNELS);
        assert!(!loshelf2.to_raw().is_null());

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_reinit_i16_can_process_afterwards() -> MaResult<()> {
        let mut loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_i16()?;

        loshelf2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        let frames_in = [
            0_i16, 0_i16, 1000_i16, -1000_i16, 2000_i16, -2000_i16, 3000_i16, -3000_i16,
        ];
        let mut frames_out = [0_i16; 8];

        loshelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());

        Ok(())
    }

    #[test]
    fn loshelf2_filter_test_reinit_f32_can_process_afterwards() -> MaResult<()> {
        let mut loshelf2 =
            LoShelf2Builder::new(CHANNELS, SAMPLE_RATE, SLOPE, GAIN_DB, FREQUENCY).build_f32()?;

        loshelf2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        let frames_in = [
            0.0_f32, 0.0_f32, 0.25_f32, -0.25_f32, 0.5_f32, -0.5_f32, 0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        loshelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }
}
