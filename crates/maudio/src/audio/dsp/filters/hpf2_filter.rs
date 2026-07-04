use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    pcm_frames::PcmFormat,
    Binding, MaResult,
};

pub struct Hpf2<F: PcmFormat> {
    inner: *mut sys::ma_hpf2,
    format: Format,
    channels: u32,
    quality: f64,
    _format: PhantomData<F>,
}

impl<F: PcmFormat> Binding for Hpf2<F> {
    type Raw = *mut sys::ma_hpf2;

    /// !!! unimplemented !!!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Hpf2<F> {
    fn build(config: &sys::ma_hpf2_config, format: Format) -> MaResult<Hpf2<F>> {
        let channels = config.channels;
        let quality = config.q;
        let mut inner: MaybeUninit<sys::ma_hpf2> = MaybeUninit::uninit();
        hpf2_ffi::ma_hpf2_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));
        Ok(Hpf2 {
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
            sys::ma_hpf2_config_init(
                self.format.into(),
                self.channels,
                sample_rate.into(),
                cutoff_freq,
                quality,
            )
        };
        self.quality = quality;
        hpf2_ffi::ma_hpf2_reinit(&config, self)
    }

    pub fn get_latency(&self) -> u32 {
        hpf2_ffi::ma_hpf2_get_latency(self)
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        hpf2_ffi::ma_hpf2_process_pcm_frames(self, frames_out, frames_in)
    }
}

pub struct Hpf2Builder {
    config: sys::ma_hpf2_config,
}

impl Hpf2Builder {
    pub fn new(channels: u32, sample_rate: SampleRate, cutoff_freq: f64, quality: f64) -> Self {
        let config = unsafe {
            sys::ma_hpf2_config_init(
                Format::S16.into(),
                channels,
                sample_rate.into(),
                cutoff_freq,
                quality,
            )
        };
        Self { config }
    }

    pub fn build_i16(&mut self) -> MaResult<Hpf2<i16>> {
        self.config.format = Format::S16.into();
        Hpf2::<i16>::build(&self.config, Format::S16)
    }

    pub fn build_f32(&mut self) -> MaResult<Hpf2<f32>> {
        self.config.format = Format::F32.into();
        Hpf2::<f32>::build(&self.config, Format::F32)
    }
}

pub(crate) mod hpf2_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::dsp::filters::hpf2_filter::Hpf2, engine::AllocationCallbacks, pcm_frames::PcmFormat,
        AsRawRef, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_hpf2_init(
        config: &sys::ma_hpf2_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        hpf2: *mut sys::ma_hpf2,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_hpf2_init(config as *const _, alloc_cb, hpf2) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_hpf2_reinit<F: PcmFormat>(
        config: &sys::ma_hpf2_config,
        hpf2: &mut Hpf2<F>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_hpf2_reinit(config as *const _, hpf2.to_raw()) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_hpf2_uninit<F: PcmFormat>(hpf2: &mut Hpf2<F>) {
        unsafe {
            sys::ma_hpf2_uninit(hpf2.to_raw(), std::ptr::null_mut());
        };
    }

    #[inline]
    pub fn ma_hpf2_process_pcm_frames<F: PcmFormat>(
        hpf2: &mut Hpf2<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = hpf2.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_hpf2_process_pcm_frames(
                hpf2.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_hpf2_get_latency<F: PcmFormat>(hpf2: &Hpf2<F>) -> u32 {
        unsafe { sys::ma_hpf2_get_latency(hpf2.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for Hpf2<F> {
    fn drop(&mut self) {
        hpf2_ffi::ma_hpf2_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rate() -> SampleRate {
        SampleRate::Sr44100
    }

    #[test]
    fn hpf2_filter_test_build_i16_uses_s16_format() {
        let mut builder = Hpf2Builder::new(2, sample_rate(), 1_000.0, 0.707);

        let hpf2 = builder.build_i16().unwrap();

        assert_eq!(hpf2.format, Format::S16);
        assert_eq!(hpf2.channels, 2);
        assert_eq!(hpf2.quality, 0.707);
        assert!(!hpf2.to_raw().is_null());
    }

    #[test]
    fn hpf2_filter_test_build_f32_uses_f32_format() {
        let mut builder = Hpf2Builder::new(2, sample_rate(), 1_000.0, 0.707);

        let hpf2 = builder.build_f32().unwrap();

        assert_eq!(hpf2.format, Format::F32);
        assert_eq!(hpf2.channels, 2);
        assert_eq!(hpf2.quality, 0.707);
        assert!(!hpf2.to_raw().is_null());
    }

    #[test]
    fn hpf2_filter_test_build_i16_after_build_f32_resets_config_format() {
        let mut builder = Hpf2Builder::new(2, sample_rate(), 1_000.0, 0.707);

        let _hpf2_f32 = builder.build_f32().unwrap();
        let hpf2_i16 = builder.build_i16().unwrap();

        assert_eq!(hpf2_i16.format, Format::S16);
        assert_eq!(hpf2_i16.channels, 2);
        assert_eq!(hpf2_i16.quality, 0.707);
        assert!(!hpf2_i16.to_raw().is_null());
    }

    #[test]
    fn hpf2_filter_test_build_f32_after_build_i16_uses_f32_format() {
        let mut builder = Hpf2Builder::new(2, sample_rate(), 1_000.0, 0.707);

        let _hpf2_i16 = builder.build_i16().unwrap();
        let hpf2_f32 = builder.build_f32().unwrap();

        assert_eq!(hpf2_f32.format, Format::F32);
        assert_eq!(hpf2_f32.channels, 2);
        assert_eq!(hpf2_f32.quality, 0.707);
        assert!(!hpf2_f32.to_raw().is_null());
    }

    #[test]
    fn hpf2_filter_test_reinit_keeps_original_format_and_channels_for_i16() {
        let mut builder = Hpf2Builder::new(2, sample_rate(), 1_000.0, 0.707);

        let mut hpf2 = builder.build_i16().unwrap();

        hpf2.reinit(SampleRate::Sr48000, 2_000.0, 1.25).unwrap();

        assert_eq!(hpf2.format, Format::S16);
        assert_eq!(hpf2.channels, 2);
        assert_eq!(hpf2.quality, 1.25);
        assert!(!hpf2.to_raw().is_null());
    }

    #[test]
    fn hpf2_filter_test_reinit_keeps_original_format_and_channels_for_f32() {
        let mut builder = Hpf2Builder::new(2, sample_rate(), 1_000.0, 0.707);

        let mut hpf2 = builder.build_f32().unwrap();

        hpf2.reinit(SampleRate::Sr48000, 2_000.0, 1.25).unwrap();

        assert_eq!(hpf2.format, Format::F32);
        assert_eq!(hpf2.channels, 2);
        assert_eq!(hpf2.quality, 1.25);
        assert!(!hpf2.to_raw().is_null());
    }

    #[test]
    fn hpf2_filter_test_process_pcm_frames_f32_accepts_interleaved_input() {
        let channels = 2;
        let frames = 128;

        let mut builder = Hpf2Builder::new(channels, sample_rate(), 1_000.0, 0.707);

        let mut hpf2 = builder.build_f32().unwrap();

        let frames_in = vec![0.5_f32; frames * channels as usize];
        let mut frames_out = vec![0.0_f32; frames * channels as usize];

        hpf2.process_pcm_frames(&mut frames_out, &frames_in)
            .unwrap();

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn hpf2_filter_test_process_pcm_frames_i16_accepts_interleaved_input() {
        let channels = 2;
        let frames = 128;

        let mut builder = Hpf2Builder::new(channels, sample_rate(), 1_000.0, 0.707);

        let mut hpf2 = builder.build_i16().unwrap();

        let frames_in = vec![1_000_i16; frames * channels as usize];
        let mut frames_out = vec![0_i16; frames * channels as usize];

        hpf2.process_pcm_frames(&mut frames_out, &frames_in)
            .unwrap();

        assert_eq!(frames_out.len(), frames_in.len());
    }

    #[test]
    fn hpf2_filter_test_processing_silence_outputs_silence_for_f32() {
        let channels = 2;
        let frames = 128;

        let mut builder = Hpf2Builder::new(channels, sample_rate(), 1_000.0, 0.707);

        let mut hpf2 = builder.build_f32().unwrap();

        let frames_in = vec![0.0_f32; frames * channels as usize];
        let mut frames_out = vec![1.0_f32; frames * channels as usize];

        hpf2.process_pcm_frames(&mut frames_out, &frames_in)
            .unwrap();

        assert!(frames_out.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn hpf2_filter_test_get_latency_is_callable() {
        let mut builder = Hpf2Builder::new(2, sample_rate(), 1_000.0, 0.707);

        let hpf2 = builder.build_f32().unwrap();

        let _latency = hpf2.get_latency();
    }
}
