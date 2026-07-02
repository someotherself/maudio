use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    pcm_frames::PcmFormat,
    Binding, MaResult,
};

pub struct HiShelf2<F: PcmFormat> {
    inner: *mut sys::ma_hishelf2,
    format: Format,
    channels: u32,
    gain_db: f64,
    slope: f64,
    frequency: f64,
    _format: PhantomData<F>,
}

impl<F: PcmFormat> Binding for HiShelf2<F> {
    type Raw = *mut sys::ma_hishelf2;

    /// !!! unimplemented !!!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> HiShelf2<F> {
    fn build(config: &sys::ma_hishelf2_config, format: Format) -> MaResult<HiShelf2<F>> {
        let channels = config.channels;
        let gain_db = config.gainDB;
        let slope = config.shelfSlope;
        let frequency = config.frequency;
        let mut inner: MaybeUninit<sys::ma_hishelf2> = MaybeUninit::uninit();
        hishelf2_ffi::ma_hishelf2_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));
        Ok(HiShelf2 {
            inner: inner_ptr,
            format,
            channels,
            gain_db,
            slope,
            frequency,
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
            sys::ma_hishelf2_config_init(
                self.format.into(),
                self.channels,
                sample_rate.into(),
                gain_db,
                slope,
                frequency,
            )
        };
        self.gain_db = gain_db;
        self.slope = slope;
        self.frequency = frequency;
        hishelf2_ffi::ma_hishelf2_reinit(&config, self)
    }

    pub fn get_latency(&self) -> u32 {
        hishelf2_ffi::ma_hishelf2_get_latency(self)
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        hishelf2_ffi::ma_hishelf2_process_pcm_frames(self, frames_out, frames_in)
    }
}

pub struct HiShelf2Builder {
    config: sys::ma_hishelf2_config,
}

impl HiShelf2Builder {
    pub fn new(
        channels: u32,
        sample_rate: SampleRate,
        gain_db: f64,
        slope: f64,
        frequency: f64,
    ) -> Self {
        let config = unsafe {
            sys::ma_hishelf2_config_init(
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

    pub fn build_i16(&mut self) -> MaResult<HiShelf2<i16>> {
        self.config.format = Format::S16.into();
        HiShelf2::<i16>::build(&self.config, Format::S16)
    }

    pub fn build_f32(&mut self) -> MaResult<HiShelf2<f32>> {
        self.config.format = Format::F32.into();
        HiShelf2::<f32>::build(&self.config, Format::F32)
    }
}

pub(crate) mod hishelf2_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::dsp::hishelf2_filter::HiShelf2, engine::AllocationCallbacks, pcm_frames::PcmFormat,
        AsRawRef, Binding, MaResult, MaudioError,
    };

    pub fn ma_hishelf2_init(
        config: &sys::ma_hishelf2_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        hishelf2: *mut sys::ma_hishelf2,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_hishelf2_init(config as *const _, alloc_cb, hishelf2) };
        MaudioError::check(res)
    }

    pub fn ma_hishelf2_uninit<F: PcmFormat>(hishelf2: &mut HiShelf2<F>) {
        unsafe {
            sys::ma_hishelf2_uninit(hishelf2.to_raw(), std::ptr::null_mut());
        };
    }

    pub fn ma_hishelf2_reinit<F: PcmFormat>(
        config: &sys::ma_hishelf2_config,
        hishelf2: &mut HiShelf2<F>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_hishelf2_reinit(config as *const _, hishelf2.to_raw()) };
        MaudioError::check(res)
    }

    pub fn ma_hishelf2_process_pcm_frames<F: PcmFormat>(
        hishelf2: &mut HiShelf2<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = hishelf2.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_hishelf2_process_pcm_frames(
                hishelf2.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    pub fn ma_hishelf2_get_latency<F: PcmFormat>(hishelf2: &HiShelf2<F>) -> u32 {
        unsafe { sys::ma_hishelf2_get_latency(hishelf2.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for HiShelf2<F> {
    fn drop(&mut self) {
        hishelf2_ffi::ma_hishelf2_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const CHANNELS: u32 = 2;
    const SAMPLE_RATE: SampleRate = SampleRate::Sr44100;
    const GAIN_DB: f64 = 6.0;
    const SLOPE: f64 = 1.0;
    const FREQUENCY: f64 = 1000.0;

    #[test]
    fn hishelf2_filter_test_build_i16() -> MaResult<()> {
        let hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_i16()?;

        assert!(!hishelf2.to_raw().is_null());
        assert_eq!(hishelf2.format, Format::S16);
        assert_eq!(hishelf2.channels, CHANNELS);
        assert_eq!(hishelf2.gain_db, GAIN_DB);
        assert_eq!(hishelf2.slope, SLOPE);
        assert_eq!(hishelf2.frequency, FREQUENCY);

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_build_f32() -> MaResult<()> {
        let hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_f32()?;

        assert!(!hishelf2.to_raw().is_null());
        assert_eq!(hishelf2.format, Format::F32);
        assert_eq!(hishelf2.channels, CHANNELS);
        assert_eq!(hishelf2.gain_db, GAIN_DB);
        assert_eq!(hishelf2.slope, SLOPE);
        assert_eq!(hishelf2.frequency, FREQUENCY);

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_get_latency_i16() -> MaResult<()> {
        let hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_i16()?;

        let latency = hishelf2.get_latency();

        assert_eq!(latency, hishelf2_ffi::ma_hishelf2_get_latency(&hishelf2));

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_get_latency_f32() -> MaResult<()> {
        let hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_f32()?;

        let latency = hishelf2.get_latency();

        assert_eq!(latency, hishelf2_ffi::ma_hishelf2_get_latency(&hishelf2));

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_process_pcm_frames_i16() -> MaResult<()> {
        let mut hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_i16()?;

        let frames_in = [
            0_i16, 0_i16,
            1000_i16, -1000_i16,
            2000_i16, -2000_i16,
            3000_i16, -3000_i16,
        ];
        let mut frames_out = [0_i16; 8];

        hishelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_process_pcm_frames_f32() -> MaResult<()> {
        let mut hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_f32()?;

        let frames_in = [
            0.0_f32, 0.0_f32,
            0.25_f32, -0.25_f32,
            0.5_f32, -0.5_f32,
            0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        hishelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_process_pcm_frames_silence_i16_stays_silent() -> MaResult<()> {
        let mut hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_i16()?;

        let frames_in = [0_i16; 16];
        let mut frames_out = [123_i16; 16];

        hishelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out, [0_i16; 16]);

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_process_pcm_frames_silence_f32_stays_silent() -> MaResult<()> {
        let mut hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_f32()?;

        let frames_in = [0.0_f32; 16];
        let mut frames_out = [1.0_f32; 16];

        hishelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out, [0.0_f32; 16]);

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_reinit_i16_updates_cached_values() -> MaResult<()> {
        let mut hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_i16()?;

        hishelf2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        assert_eq!(hishelf2.format, Format::S16);
        assert_eq!(hishelf2.channels, CHANNELS);
        assert_eq!(hishelf2.gain_db, -3.0);
        assert_eq!(hishelf2.slope, 2.0);
        assert_eq!(hishelf2.frequency, 2_000.0);
        assert!(!hishelf2.to_raw().is_null());

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_reinit_f32_updates_cached_values() -> MaResult<()> {
        let mut hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_f32()?;

        hishelf2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        assert_eq!(hishelf2.format, Format::F32);
        assert_eq!(hishelf2.channels, CHANNELS);
        assert_eq!(hishelf2.gain_db, -3.0);
        assert_eq!(hishelf2.slope, 2.0);
        assert_eq!(hishelf2.frequency, 2_000.0);
        assert!(!hishelf2.to_raw().is_null());

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_reinit_i16_can_process_afterwards() -> MaResult<()> {
        let mut hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_i16()?;

        hishelf2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        let frames_in = [
            0_i16, 0_i16,
            1000_i16, -1000_i16,
            2000_i16, -2000_i16,
            3000_i16, -3000_i16,
        ];
        let mut frames_out = [0_i16; 8];

        hishelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());

        Ok(())
    }

    #[test]
    fn hishelf2_filter_test_reinit_f32_can_process_afterwards() -> MaResult<()> {
        let mut hishelf2 = HiShelf2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, SLOPE, FREQUENCY)
            .build_f32()?;

        hishelf2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        let frames_in = [
            0.0_f32, 0.0_f32,
            0.25_f32, -0.25_f32,
            0.5_f32, -0.5_f32,
            0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        hishelf2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }
}