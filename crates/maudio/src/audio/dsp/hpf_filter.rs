use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    pcm_frames::PcmFormat,
    Binding, MaResult,
};

pub struct Hpf<F: PcmFormat> {
    inner: *mut sys::ma_hpf,
    format: Format,
    channels: u32,
    order: u32,
    _format: PhantomData<F>,
}

impl<F: PcmFormat> Binding for Hpf<F> {
    type Raw = *mut sys::ma_hpf;

    /// !!! unimplemented !!!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Hpf<F> {
    fn build(config: &sys::ma_hpf_config, format: Format) -> MaResult<Hpf<F>> {
        let channels = config.channels;
        let order = config.order;
        let mut inner: MaybeUninit<sys::ma_hpf> = MaybeUninit::uninit();
        hpf_ffi::ma_hpf_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));
        Ok(Hpf {
            inner: inner_ptr,
            format,
            channels,
            order,
            _format: PhantomData,
        })
    }

    pub fn reinit(&mut self, sample_rate: SampleRate, cutoff_freq: f64) -> MaResult<()> {
        let config = unsafe {
            sys::ma_hpf_config_init(
                self.format.into(),
                self.channels,
                sample_rate.into(),
                cutoff_freq,
                self.order,
            )
        };
        hpf_ffi::ma_hpf_reinit(&config, self)
    }

    pub fn get_latency(&self) -> u32 {
        hpf_ffi::ma_hpf_get_latency(self)
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        hpf_ffi::ma_hpf_process_pcm_frames(self, frames_out, frames_in)
    }
}

pub struct HpfBuilder {
    config: sys::ma_hpf_config,
}

impl HpfBuilder {
    pub fn new(channels: u32, sample_rate: SampleRate, cutoff_freq: f64, order: u32) -> Self {
        let config = unsafe {
            sys::ma_hpf_config_init(
                Format::S16.into(),
                channels,
                sample_rate.into(),
                cutoff_freq,
                order,
            )
        };
        Self { config }
    }

    pub fn build_i16(&mut self) -> MaResult<Hpf<i16>> {
        self.config.format = Format::S16.into();
        Hpf::<i16>::build(&self.config, Format::S16)
    }

    pub fn build_f32(&mut self) -> MaResult<Hpf<f32>> {
        self.config.format = Format::F32.into();
        Hpf::<f32>::build(&self.config, Format::F32)
    }
}

pub(crate) mod hpf_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::dsp::hpf_filter::Hpf, engine::AllocationCallbacks, pcm_frames::PcmFormat, AsRawRef,
        Binding, MaResult, MaudioError,
    };

    pub fn ma_hpf_init(
        config: &sys::ma_hpf_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        hpf: *mut sys::ma_hpf,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_hpf_init(config as *const _, alloc_cb, hpf) };
        MaudioError::check(res)
    }

    pub fn ma_hpf_reinit<F: PcmFormat>(
        config: &sys::ma_hpf_config,
        hpf: &mut Hpf<F>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_hpf_reinit(config as *const _, hpf.to_raw()) };
        MaudioError::check(res)
    }

    pub fn ma_hpf_uninit<F: PcmFormat>(hpf: &mut Hpf<F>) {
        unsafe {
            sys::ma_hpf_uninit(hpf.to_raw(), std::ptr::null_mut());
        };
    }

    pub fn ma_hpf_process_pcm_frames<F: PcmFormat>(
        hpf: &mut Hpf<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = hpf.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_hpf_process_pcm_frames(
                hpf.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    pub fn ma_hpf_get_latency<F: PcmFormat>(hpf: &Hpf<F>) -> u32 {
        unsafe { sys::ma_hpf_get_latency(hpf.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for Hpf<F> {
    fn drop(&mut self) {
        hpf_ffi::ma_hpf_uninit(self);
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
    fn hpf_filter_test_build_i16_uses_s16_format() {
        let mut builder = HpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let hpf = builder.build_i16().unwrap();

        assert_eq!(hpf.format, Format::S16);
        assert_eq!(hpf.channels, 2);
        assert_eq!(hpf.order, 2);
        assert!(!hpf.to_raw().is_null());
    }

    #[test]
    fn hpf_filter_test_build_f32_uses_f32_format() {
        let mut builder = HpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let hpf = builder.build_f32().unwrap();

        assert_eq!(hpf.format, Format::F32);
        assert_eq!(hpf.channels, 2);
        assert_eq!(hpf.order, 2);
        assert!(!hpf.to_raw().is_null());
    }

    #[test]
    fn hpf_filter_test_build_i16_after_build_f32_resets_config_format() {
        let mut builder = HpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let _hpf_f32 = builder.build_f32().unwrap();
        let hpf_i16 = builder.build_i16().unwrap();

        assert_eq!(hpf_i16.format, Format::S16);
        assert_eq!(hpf_i16.channels, 2);
        assert_eq!(hpf_i16.order, 2);
        assert!(!hpf_i16.to_raw().is_null());
    }

    #[test]
    fn hpf_filter_test_reinit_keeps_original_format_and_channels_for_i16() {
        let mut builder = HpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let mut hpf = builder.build_i16().unwrap();

        hpf.reinit(SampleRate::Sr48000, 2_000.0).unwrap();

        assert_eq!(hpf.format, Format::S16);
        assert_eq!(hpf.channels, 2);
        assert_eq!(hpf.order, 2);
        assert!(!hpf.to_raw().is_null());
    }

    #[test]
    fn hpf_filter_test_reinit_keeps_original_format_and_channels_for_f32() {
        let mut builder = HpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let mut hpf = builder.build_f32().unwrap();

        hpf.reinit(SampleRate::Sr48000, 2_000.0).unwrap();

        assert_eq!(hpf.format, Format::F32);
        assert_eq!(hpf.channels, 2);
        assert_eq!(hpf.order, 2);
        assert!(!hpf.to_raw().is_null());
    }

    #[test]
    fn hpf_filter_test_process_pcm_frames_f32_accepts_interleaved_input() {
        let channels = 2;
        let frames = 128;

        let mut builder = HpfBuilder::new(channels, sample_rate(), 1_000.0, 2);

        let mut hpf = builder.build_f32().unwrap();

        let frames_in = vec![0.5_f32; frames * channels as usize];
        let mut frames_out = vec![0.0_f32; frames * channels as usize];

        hpf.process_pcm_frames(&mut frames_out, &frames_in).unwrap();

        assert_eq!(frames_out.len(), frames_in.len());
        assert!(frames_out.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn hpf_filter_test_process_pcm_frames_i16_accepts_interleaved_input() {
        let channels = 2;
        let frames = 128;

        let mut builder = HpfBuilder::new(channels, sample_rate(), 1_000.0, 2);

        let mut hpf = builder.build_i16().unwrap();

        let frames_in = vec![1_000_i16; frames * channels as usize];
        let mut frames_out = vec![0_i16; frames * channels as usize];

        hpf.process_pcm_frames(&mut frames_out, &frames_in).unwrap();

        assert_eq!(frames_out.len(), frames_in.len());
    }

    #[test]
    fn hpf_filter_test_processing_silence_outputs_silence_for_f32() {
        let channels = 2;
        let frames = 128;

        let mut builder = HpfBuilder::new(channels, sample_rate(), 1_000.0, 2);

        let mut hpf = builder.build_f32().unwrap();

        let frames_in = vec![0.0_f32; frames * channels as usize];
        let mut frames_out = vec![1.0_f32; frames * channels as usize];

        hpf.process_pcm_frames(&mut frames_out, &frames_in).unwrap();

        assert!(frames_out.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn hpf_filter_test_get_latency_is_callable() {
        let mut builder = HpfBuilder::new(2, sample_rate(), 1_000.0, 2);

        let hpf = builder.build_f32().unwrap();

        let _latency = hpf.get_latency();
    }
}
