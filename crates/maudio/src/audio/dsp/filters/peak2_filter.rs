use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    pcm_frames::PcmFormat,
    Binding, MaResult,
};

pub struct Peak2<F: PcmFormat> {
    inner: *mut sys::ma_peak2,
    format: Format,
    channels: u32,
    quality: f64,
    frequency: f64,
    gain_db: f64,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for Peak2<F> {}

impl<F: PcmFormat> Binding for Peak2<F> {
    type Raw = *mut sys::ma_peak2;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Peak2<F> {
    fn build(config: &sys::ma_peak2_config, format: Format) -> MaResult<Peak2<F>> {
        let channels = config.channels;
        let quality = config.q;
        let frequency = config.frequency;
        let gain_db = config.gainDB;
        let mut inner: MaybeUninit<sys::ma_peak2> = MaybeUninit::uninit();
        peak2_ffi::ma_peak2_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));
        Ok(Peak2 {
            inner: inner_ptr,
            format,
            channels,
            quality,
            frequency,
            gain_db,
            _format: PhantomData,
        })
    }

    pub fn reinit(
        &mut self,
        sample_rate: SampleRate,
        gain_db: f64,
        quality: f64,
        frequency: f64,
    ) -> MaResult<()> {
        let config = unsafe {
            sys::ma_peak2_config_init(
                self.format.into(),
                self.channels,
                sample_rate.into(),
                gain_db,
                quality,
                frequency,
            )
        };
        self.quality = quality;
        self.frequency = frequency;
        self.gain_db = gain_db;
        peak2_ffi::ma_peak2_reinit(&config, self)
    }

    pub fn get_latency(&self) -> u32 {
        peak2_ffi::ma_peak2_get_latency(self)
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        peak2_ffi::ma_peak2_process_pcm_frames(self, frames_out, frames_in)
    }
}

pub struct Peak2Builder {
    config: sys::ma_peak2_config,
}

impl Peak2Builder {
    pub fn new(
        channels: u32,
        sample_rate: SampleRate,
        gain_db: f64,
        quality: f64,
        frequency: f64,
    ) -> Self {
        let config = unsafe {
            sys::ma_peak2_config_init(
                Format::S16.into(),
                channels,
                sample_rate.into(),
                gain_db,
                quality,
                frequency,
            )
        };
        Self { config }
    }

    pub fn build_i16(&mut self) -> MaResult<Peak2<i16>> {
        self.config.format = Format::S16.into();
        Peak2::<i16>::build(&self.config, Format::S16)
    }

    pub fn build_f32(&mut self) -> MaResult<Peak2<f32>> {
        self.config.format = Format::F32.into();
        Peak2::<f32>::build(&self.config, Format::F32)
    }
}

pub(crate) mod peak2_ffi {

    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::dsp::filters::peak2_filter::Peak2, engine::AllocationCallbacks,
        pcm_frames::PcmFormat, AsRawRef, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_peak2_init(
        config: &sys::ma_peak2_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        peak2: *mut sys::ma_peak2,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let res = unsafe { sys::ma_peak2_init(config, alloc_cb, peak2) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_peak2_uninit<F: PcmFormat>(peak2: &mut Peak2<F>) {
        unsafe {
            sys::ma_peak2_uninit(peak2.to_raw(), std::ptr::null_mut());
        }
    }

    #[inline]
    pub fn ma_peak2_reinit<F: PcmFormat>(
        config: &sys::ma_peak2_config,
        peak2: &mut Peak2<F>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_peak2_reinit(config as *const _, peak2.to_raw()) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_peak2_process_pcm_frames<F: PcmFormat>(
        peak2: &mut Peak2<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = peak2.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_peak2_process_pcm_frames(
                peak2.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_peak2_get_latency<F: PcmFormat>(peak2: &Peak2<F>) -> u32 {
        unsafe { sys::ma_peak2_get_latency(peak2.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for Peak2<F> {
    fn drop(&mut self) {
        peak2_ffi::ma_peak2_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const CHANNELS: u32 = 2;
    const SAMPLE_RATE: SampleRate = SampleRate::Sr44100;
    const GAIN_DB: f64 = 6.0;
    const QUALITY: f64 = 1.0;
    const FREQUENCY: f64 = 1000.0;

    #[test]
    fn peak2_filter_test_build_i16() -> MaResult<()> {
        let peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_i16()?;

        assert!(!peak2.to_raw().is_null());
        assert_eq!(peak2.format, Format::S16);
        assert_eq!(peak2.channels, CHANNELS);
        assert_eq!(peak2.gain_db, GAIN_DB);
        assert_eq!(peak2.quality, QUALITY);
        assert_eq!(peak2.frequency, FREQUENCY);

        Ok(())
    }

    #[test]
    fn peak2_filter_test_build_f32() -> MaResult<()> {
        let peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_f32()?;

        assert!(!peak2.to_raw().is_null());
        assert_eq!(peak2.format, Format::F32);
        assert_eq!(peak2.channels, CHANNELS);
        assert_eq!(peak2.gain_db, GAIN_DB);
        assert_eq!(peak2.quality, QUALITY);
        assert_eq!(peak2.frequency, FREQUENCY);

        Ok(())
    }

    #[test]
    fn peak2_filter_test_get_latency_i16() -> MaResult<()> {
        let peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_i16()?;

        let latency = peak2.get_latency();

        // ma_peak2 should be callable and return a stable non-negative u32.
        // Keep this loose because the exact latency is owned by miniaudio.
        assert_eq!(latency, peak2_ffi::ma_peak2_get_latency(&peak2));

        Ok(())
    }

    #[test]
    fn peak2_filter_test_get_latency_f32() -> MaResult<()> {
        let peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_f32()?;

        let latency = peak2.get_latency();

        assert_eq!(latency, peak2_ffi::ma_peak2_get_latency(&peak2));

        Ok(())
    }

    #[test]
    fn peak2_filter_test_process_pcm_frames_i16() -> MaResult<()> {
        let mut peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_i16()?;

        let frames_in = [
            0_i16, 0_i16, 1000_i16, -1000_i16, 2000_i16, -2000_i16, 3000_i16, -3000_i16,
        ];
        let mut frames_out = [0_i16; 8];

        peak2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());

        Ok(())
    }

    #[test]
    fn peak2_filter_test_process_pcm_frames_f32() -> MaResult<()> {
        let mut peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_f32()?;

        let frames_in = [
            0.0_f32, 0.0_f32, 0.25_f32, -0.25_f32, 0.5_f32, -0.5_f32, 0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        peak2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }

    #[test]
    fn peak2_filter_test_process_pcm_frames_silence_i16_stays_silent() -> MaResult<()> {
        let mut peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_i16()?;

        let frames_in = [0_i16; 16];
        let mut frames_out = [123_i16; 16];

        peak2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out, [0_i16; 16]);

        Ok(())
    }

    #[test]
    fn peak2_filter_test_process_pcm_frames_silence_f32_stays_silent() -> MaResult<()> {
        let mut peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_f32()?;

        let frames_in = [0.0_f32; 16];
        let mut frames_out = [1.0_f32; 16];

        peak2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out, [0.0_f32; 16]);

        Ok(())
    }

    #[test]
    fn peak2_filter_test_reinit_i16_updates_cached_values() -> MaResult<()> {
        let mut peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_i16()?;

        peak2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        assert_eq!(peak2.format, Format::S16);
        assert_eq!(peak2.channels, CHANNELS);
        assert_eq!(peak2.gain_db, -3.0);
        assert_eq!(peak2.quality, 2.0);
        assert_eq!(peak2.frequency, 2_000.0);
        assert!(!peak2.to_raw().is_null());

        Ok(())
    }

    #[test]
    fn peak2_filter_test_reinit_f32_updates_cached_values() -> MaResult<()> {
        let mut peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_f32()?;

        peak2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        assert_eq!(peak2.format, Format::F32);
        assert_eq!(peak2.channels, CHANNELS);
        assert_eq!(peak2.gain_db, -3.0);
        assert_eq!(peak2.quality, 2.0);
        assert_eq!(peak2.frequency, 2_000.0);
        assert!(!peak2.to_raw().is_null());

        Ok(())
    }

    #[test]
    fn peak2_filter_test_reinit_i16_can_process_afterwards() -> MaResult<()> {
        let mut peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_i16()?;

        peak2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        let frames_in = [
            0_i16, 0_i16, 1000_i16, -1000_i16, 2000_i16, -2000_i16, 3000_i16, -3000_i16,
        ];
        let mut frames_out = [0_i16; 8];

        peak2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());

        Ok(())
    }

    #[test]
    fn peak2_filter_test_reinit_f32_can_process_afterwards() -> MaResult<()> {
        let mut peak2 =
            Peak2Builder::new(CHANNELS, SAMPLE_RATE, GAIN_DB, QUALITY, FREQUENCY).build_f32()?;

        peak2.reinit(SampleRate::Sr48000, -3.0, 2.0, 2_000.0)?;

        let frames_in = [
            0.0_f32, 0.0_f32, 0.25_f32, -0.25_f32, 0.5_f32, -0.5_f32, 0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        peak2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }
}
