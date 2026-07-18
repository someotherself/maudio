use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    pcm_frames::PcmFormat,
    Binding, MaResult,
};

pub struct Notch2<F: PcmFormat> {
    inner: *mut sys::ma_notch2,
    format: Format,
    channels: u32,
    quality: f64,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for Notch2<F> {}

impl<F: PcmFormat> Binding for Notch2<F> {
    type Raw = *mut sys::ma_notch2;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Notch2<F> {
    fn build(config: &sys::ma_notch2_config, format: Format) -> MaResult<Notch2<F>> {
        let channels = config.channels;
        let quality = config.q;
        let mut inner: Box<MaybeUninit<sys::ma_notch2>> = Box::new(MaybeUninit::uninit());
        notch2_ffi::ma_notch2_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(inner) as *mut sys::ma_notch2;
        Ok(Notch2 {
            inner: inner_ptr,
            format,
            channels,
            quality,
            _format: PhantomData,
        })
    }

    pub fn reinit(
        &mut self,
        sample_rate: SampleRate,
        cutoff_freq: f64,
        quality: f64,
    ) -> MaResult<()> {
        let config = unsafe {
            sys::ma_notch2_config_init(
                self.format.into(),
                self.channels,
                sample_rate.into(),
                quality,
                cutoff_freq,
            )
        };
        self.quality = quality;
        notch2_ffi::ma_notch2_reinit(&config, self)
    }

    pub fn get_latency(&self) -> u32 {
        notch2_ffi::ma_notch2_get_latency(self)
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        notch2_ffi::ma_notch2_process_pcm_frames(self, frames_out, frames_in)
    }
}

pub struct Notch2Builder {
    config: sys::ma_notch2_config,
}

impl Notch2Builder {
    pub fn new(channels: u32, sample_rate: SampleRate, cutoff_freq: f64, quality: f64) -> Self {
        let config = unsafe {
            sys::ma_notch2_config_init(
                Format::S16.into(),
                channels,
                sample_rate.into(),
                quality,
                cutoff_freq,
            )
        };
        Self { config }
    }

    pub fn build_i16(&mut self) -> MaResult<Notch2<i16>> {
        self.config.format = Format::S16.into();
        Notch2::<i16>::build(&self.config, Format::S16)
    }

    pub fn build_f32(&mut self) -> MaResult<Notch2<f32>> {
        self.config.format = Format::F32.into();
        Notch2::<f32>::build(&self.config, Format::F32)
    }
}

pub(crate) mod notch2_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::dsp::filters::notch2_filter::Notch2, engine::AllocationCallbacks,
        pcm_frames::PcmFormat, AsRawRef, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_notch2_init(
        config: &sys::ma_notch2_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        notch2: *mut sys::ma_notch2,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let res = unsafe { sys::ma_notch2_init(config as *const _, alloc_cb, notch2) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_notch2_uninit<F: PcmFormat>(notch2: &mut Notch2<F>) {
        unsafe {
            sys::ma_notch2_uninit(notch2.to_raw(), std::ptr::null_mut());
        }
    }

    #[inline]
    pub fn ma_notch2_reinit<F: PcmFormat>(
        config: &sys::ma_notch2_config,
        notch2: &mut Notch2<F>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_notch2_reinit(config, notch2.to_raw()) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_notch2_process_pcm_frames<F: PcmFormat>(
        notch2: &mut Notch2<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = notch2.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_notch2_process_pcm_frames(
                notch2.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_notch2_get_latency<F: PcmFormat>(notch2: &Notch2<F>) -> u32 {
        unsafe { sys::ma_notch2_get_latency(notch2.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for Notch2<F> {
    fn drop(&mut self) {
        notch2_ffi::ma_notch2_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const CHANNELS: u32 = 2;
    const SAMPLE_RATE: SampleRate = SampleRate::Sr44100;
    const CUTOFF_FREQ: f64 = 1000.0;
    const QUALITY: f64 = 1.0;

    #[test]
    fn notch2_filter_test_build_i16() -> MaResult<()> {
        let notch2 = Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_i16()?;

        assert!(!notch2.to_raw().is_null());
        assert_eq!(notch2.format, Format::S16);
        assert_eq!(notch2.channels, CHANNELS);
        assert_eq!(notch2.quality, QUALITY);

        Ok(())
    }

    #[test]
    fn notch2_filter_test_build_f32() -> MaResult<()> {
        let notch2 = Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_f32()?;

        assert!(!notch2.to_raw().is_null());
        assert_eq!(notch2.format, Format::F32);
        assert_eq!(notch2.channels, CHANNELS);
        assert_eq!(notch2.quality, QUALITY);

        Ok(())
    }

    #[test]
    fn notch2_filter_test_get_latency_i16() -> MaResult<()> {
        let notch2 = Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_i16()?;

        let latency = notch2.get_latency();

        assert_eq!(latency, notch2_ffi::ma_notch2_get_latency(&notch2));

        Ok(())
    }

    #[test]
    fn notch2_filter_test_get_latency_f32() -> MaResult<()> {
        let notch2 = Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_f32()?;

        let latency = notch2.get_latency();

        assert_eq!(latency, notch2_ffi::ma_notch2_get_latency(&notch2));

        Ok(())
    }

    #[test]
    fn notch2_filter_test_process_pcm_frames_i16() -> MaResult<()> {
        let mut notch2 =
            Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_i16()?;

        let frames_in = [
            0_i16, 0_i16, 1000_i16, -1000_i16, 2000_i16, -2000_i16, 3000_i16, -3000_i16,
        ];
        let mut frames_out = [0_i16; 8];

        notch2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());

        Ok(())
    }

    #[test]
    fn notch2_filter_test_process_pcm_frames_f32() -> MaResult<()> {
        let mut notch2 =
            Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_f32()?;

        let frames_in = [
            0.0_f32, 0.0_f32, 0.25_f32, -0.25_f32, 0.5_f32, -0.5_f32, 0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        notch2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }

    #[test]
    fn notch2_filter_test_process_pcm_frames_silence_i16_stays_silent() -> MaResult<()> {
        let mut notch2 =
            Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_i16()?;

        let frames_in = [0_i16; 16];
        let mut frames_out = [123_i16; 16];

        notch2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out, [0_i16; 16]);

        Ok(())
    }

    #[test]
    fn notch2_filter_test_process_pcm_frames_silence_f32_stays_silent() -> MaResult<()> {
        let mut notch2 =
            Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_f32()?;

        let frames_in = [0.0_f32; 16];
        let mut frames_out = [1.0_f32; 16];

        notch2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out, [0.0_f32; 16]);

        Ok(())
    }

    #[test]
    fn notch2_filter_test_reinit_i16_updates_cached_values() -> MaResult<()> {
        let mut notch2 =
            Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_i16()?;

        notch2.reinit(SampleRate::Sr48000, 2_000.0, 2.0)?;

        assert_eq!(notch2.format, Format::S16);
        assert_eq!(notch2.channels, CHANNELS);
        assert_eq!(notch2.quality, 2.0);
        assert!(!notch2.to_raw().is_null());

        Ok(())
    }

    #[test]
    fn notch2_filter_test_reinit_f32_updates_cached_values() -> MaResult<()> {
        let mut notch2 =
            Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_f32()?;

        notch2.reinit(SampleRate::Sr48000, 2_000.0, 2.0)?;

        assert_eq!(notch2.format, Format::F32);
        assert_eq!(notch2.channels, CHANNELS);
        assert_eq!(notch2.quality, 2.0);
        assert!(!notch2.to_raw().is_null());

        Ok(())
    }

    #[test]
    fn notch2_filter_test_reinit_i16_can_process_afterwards() -> MaResult<()> {
        let mut notch2 =
            Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_i16()?;

        notch2.reinit(SampleRate::Sr48000, 2_000.0, 2.0)?;

        let frames_in = [
            0_i16, 0_i16, 1000_i16, -1000_i16, 2000_i16, -2000_i16, 3000_i16, -3000_i16,
        ];
        let mut frames_out = [0_i16; 8];

        notch2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());

        Ok(())
    }

    #[test]
    fn notch2_filter_test_reinit_f32_can_process_afterwards() -> MaResult<()> {
        let mut notch2 =
            Notch2Builder::new(CHANNELS, SAMPLE_RATE, CUTOFF_FREQ, QUALITY).build_f32()?;

        notch2.reinit(SampleRate::Sr48000, 2_000.0, 2.0)?;

        let frames_in = [
            0.0_f32, 0.0_f32, 0.25_f32, -0.25_f32, 0.5_f32, -0.5_f32, 0.75_f32, -0.75_f32,
        ];
        let mut frames_out = [0.0_f32; 8];

        notch2.process_pcm_frames(&mut frames_out, &frames_in)?;

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));

        Ok(())
    }
}
