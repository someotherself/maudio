use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    pcm_frames::PcmFormat,
    Binding, MaResult,
};

pub struct Lpf<F: PcmFormat> {
    inner: *mut sys::ma_lpf,
    format: Format,
    channels: u32,
    order: u32,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for Lpf<F> {}

impl<F: PcmFormat> Binding for Lpf<F> {
    type Raw = *mut sys::ma_lpf;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Lpf<F> {
    fn build(config: &sys::ma_lpf_config, format: Format) -> MaResult<Lpf<F>> {
        let channels = config.channels;
        let order = config.order;
        let mut inner: Box<MaybeUninit<sys::ma_lpf>> = Box::new(MaybeUninit::uninit());
        lpf_ffi::ma_lpf_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(inner) as *mut sys::ma_lpf;
        Ok(Lpf {
            inner: inner_ptr,
            format,
            channels,
            order,
            _format: PhantomData,
        })
    }

    pub fn reinit(&mut self, sample_rate: SampleRate, cutoff_freq: f64) -> MaResult<()> {
        let config = unsafe {
            sys::ma_lpf_config_init(
                self.format.into(),
                self.channels,
                sample_rate.into(),
                cutoff_freq,
                self.order,
            )
        };
        lpf_ffi::ma_lpf_reinit(&config, self)
    }

    pub fn get_latency(&self) -> u32 {
        lpf_ffi::ma_lpf_get_latency(self)
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        lpf_ffi::ma_lpf_process_pcm_frames(self, frames_out, frames_in)
    }
}

pub struct LpfBuilder {
    config: sys::ma_lpf_config,
}

impl LpfBuilder {
    pub fn new(channels: u32, sample_rate: SampleRate, cutoff_freq: f64, order: u32) -> Self {
        let config = unsafe {
            sys::ma_lpf_config_init(
                Format::S16.into(),
                channels,
                sample_rate.into(),
                cutoff_freq,
                order,
            )
        };
        Self { config }
    }

    pub fn build_i16(&mut self) -> MaResult<Lpf<i16>> {
        self.config.format = Format::S16.into();
        Lpf::<i16>::build(&self.config, Format::S16)
    }

    pub fn build_f32(&mut self) -> MaResult<Lpf<f32>> {
        self.config.format = Format::F32.into();
        Lpf::<f32>::build(&self.config, Format::F32)
    }
}

pub(crate) mod lpf_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::dsp::filters::lpf_filter::Lpf, engine::AllocationCallbacks, pcm_frames::PcmFormat,
        AsRawRef, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_lpf_init(
        config: &sys::ma_lpf_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        lpf: *mut sys::ma_lpf,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_lpf_init(config as *const _, alloc_cb, lpf) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_lpf_reinit<F: PcmFormat>(
        config: &sys::ma_lpf_config,
        lpf: &mut Lpf<F>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_lpf_reinit(config as *const _, lpf.to_raw()) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_lpf_uninit<F: PcmFormat>(lpf: &mut Lpf<F>) {
        unsafe {
            sys::ma_lpf_uninit(lpf.to_raw(), std::ptr::null_mut());
        };
    }

    #[inline]
    pub fn ma_lpf_process_pcm_frames<F: PcmFormat>(
        lpf: &mut Lpf<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = lpf.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_lpf_process_pcm_frames(
                lpf.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_lpf_get_latency<F: PcmFormat>(lpf: &Lpf<F>) -> u32 {
        unsafe { sys::ma_lpf_get_latency(lpf.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for Lpf<F> {
    fn drop(&mut self) {
        lpf_ffi::ma_lpf_uninit(self);
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
    fn lpf_filter_test_build_i16_uses_s16_format() {
        let mut builder = LpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let lpf = builder.build_i16().unwrap();

        assert_eq!(lpf.format, Format::S16);
        assert_eq!(lpf.channels, 2);
        assert_eq!(lpf.order, 2);
        assert!(!lpf.to_raw().is_null());
    }

    #[test]
    fn lpf_filter_test_build_f32_uses_f32_format() {
        let mut builder = LpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let lpf = builder.build_f32().unwrap();

        assert_eq!(lpf.format, Format::F32);
        assert_eq!(lpf.channels, 2);
        assert_eq!(lpf.order, 2);
        assert!(!lpf.to_raw().is_null());
    }

    #[test]
    fn lpf_filter_test_build_i16_after_build_f32_resets_config_format() {
        let mut builder = LpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let _lpf_f32 = builder.build_f32().unwrap();
        let lpf_i16 = builder.build_i16().unwrap();

        assert_eq!(lpf_i16.format, Format::S16);
        assert_eq!(lpf_i16.channels, 2);
        assert_eq!(lpf_i16.order, 2);
        assert!(!lpf_i16.to_raw().is_null());
    }

    #[test]
    fn lpf_filter_test_build_f32_after_build_i16_uses_f32_format() {
        let mut builder = LpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let _lpf_i16 = builder.build_i16().unwrap();
        let lpf_f32 = builder.build_f32().unwrap();

        assert_eq!(lpf_f32.format, Format::F32);
        assert_eq!(lpf_f32.channels, 2);
        assert_eq!(lpf_f32.order, 2);
        assert!(!lpf_f32.to_raw().is_null());
    }

    #[test]
    fn lpf_filter_test_reinit_keeps_original_format_channels_and_order_for_i16() {
        let mut builder = LpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let mut lpf = builder.build_i16().unwrap();

        lpf.reinit(SampleRate::Sr48000, 2_000.0).unwrap();

        assert_eq!(lpf.format, Format::S16);
        assert_eq!(lpf.channels, 2);
        assert_eq!(lpf.order, 2);
        assert!(!lpf.to_raw().is_null());
    }

    #[test]
    fn lpf_filter_test_reinit_keeps_original_format_channels_and_order_for_f32() {
        let mut builder = LpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let mut lpf = builder.build_f32().unwrap();

        lpf.reinit(SampleRate::Sr48000, 2_000.0).unwrap();

        assert_eq!(lpf.format, Format::F32);
        assert_eq!(lpf.channels, 2);
        assert_eq!(lpf.order, 2);
        assert!(!lpf.to_raw().is_null());
    }

    #[test]
    fn lpf_filter_test_process_pcm_frames_f32_accepts_interleaved_input() {
        let channels = 2;
        let frames = 128;

        let mut builder = LpfBuilder::new(channels, sample_rate(), 1_000.0, 2);

        let mut lpf = builder.build_f32().unwrap();

        let frames_in = vec![0.5_f32; frames * channels as usize];
        let mut frames_out = vec![0.0_f32; frames * channels as usize];

        lpf.process_pcm_frames(&mut frames_out, &frames_in).unwrap();

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn lpf_filter_test_process_pcm_frames_i16_accepts_interleaved_input() {
        let channels = 2;
        let frames = 128;

        let mut builder = LpfBuilder::new(channels, sample_rate(), 1_000.0, 2);

        let mut lpf = builder.build_i16().unwrap();

        let frames_in = vec![1_000_i16; frames * channels as usize];
        let mut frames_out = vec![0_i16; frames * channels as usize];

        lpf.process_pcm_frames(&mut frames_out, &frames_in).unwrap();

        assert_eq!(frames_out.len(), frames_in.len());
    }

    #[test]
    fn lpf_filter_test_processing_silence_outputs_silence_for_f32() {
        let channels = 2;
        let frames = 128;

        let mut builder = LpfBuilder::new(channels, sample_rate(), 1_000.0, 2);

        let mut lpf = builder.build_f32().unwrap();

        let frames_in = vec![0.0_f32; frames * channels as usize];
        let mut frames_out = vec![1.0_f32; frames * channels as usize];

        lpf.process_pcm_frames(&mut frames_out, &frames_in).unwrap();

        assert!(frames_out.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn lpf_filter_test_get_latency_is_callable() {
        let mut builder = LpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let lpf = builder.build_f32().unwrap();

        let _latency = lpf.get_latency();
    }
}
