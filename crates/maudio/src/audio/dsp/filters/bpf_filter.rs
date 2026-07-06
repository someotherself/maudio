use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    pcm_frames::PcmFormat,
    Binding, MaResult,
};

pub struct Bpf<F: PcmFormat> {
    inner: *mut sys::ma_bpf,
    format: Format,
    channels: u32,
    order: u32,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for Bpf<F> {}

impl<F: PcmFormat> Bpf<F> {
    fn build(config: &sys::ma_bpf_config, format: Format) -> MaResult<Bpf<F>> {
        let channels = config.channels;
        let order = config.order;
        let mut inner: MaybeUninit<sys::ma_bpf> = MaybeUninit::uninit();
        bpf_ffi::ma_bpf_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));
        Ok(Bpf {
            inner: inner_ptr,
            format,
            channels,
            order,
            _format: PhantomData,
        })
    }

    pub fn reinit(&mut self, sample_rate: SampleRate, cutoff_freq: f64) -> MaResult<()> {
        let config = unsafe {
            sys::ma_bpf_config_init(
                self.format.into(),
                self.channels,
                sample_rate.into(),
                cutoff_freq,
                self.order,
            )
        };
        bpf_ffi::ma_bpf_reinit(&config, self)
    }

    pub fn get_latency(&self) -> u32 {
        bpf_ffi::ma_bpf_get_latency(self)
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        bpf_ffi::ma_bpf_process_pcm_frames(self, frames_out, frames_in)
    }
}

impl<F: PcmFormat> Binding for Bpf<F> {
    type Raw = *mut sys::ma_bpf;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

pub struct BpfBuilder {
    config: sys::ma_bpf_config,
}

impl BpfBuilder {
    pub fn new(channels: u32, sample_rate: SampleRate, cutoff_freq: f64, order: u32) -> Self {
        let config = unsafe {
            sys::ma_bpf_config_init(
                Format::S16.into(),
                channels,
                sample_rate.into(),
                cutoff_freq,
                order,
            )
        };
        Self { config }
    }

    pub fn build_i16(&mut self) -> MaResult<Bpf<i16>> {
        self.config.format = Format::S16.into();
        Bpf::<i16>::build(&self.config, Format::S16)
    }

    pub fn build_f32(&mut self) -> MaResult<Bpf<f32>> {
        self.config.format = Format::F32.into();
        Bpf::<f32>::build(&self.config, Format::F32)
    }
}

pub(crate) mod bpf_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::dsp::filters::bpf_filter::Bpf, engine::AllocationCallbacks, pcm_frames::PcmFormat,
        AsRawRef, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_bpf_init(
        config: &sys::ma_bpf_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        bpf: *mut sys::ma_bpf,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let res = unsafe { sys::ma_bpf_init(config as *const _, alloc_cb, bpf) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_bpf_uninit<F: PcmFormat>(bpf: &mut Bpf<F>) {
        unsafe {
            sys::ma_bpf_uninit(bpf.to_raw(), std::ptr::null_mut());
        }
    }

    #[inline]
    pub fn ma_bpf_reinit<F: PcmFormat>(
        config: &sys::ma_bpf_config,
        bpf: &mut Bpf<F>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_bpf_reinit(config, bpf.to_raw()) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_bpf_process_pcm_frames<F: PcmFormat>(
        bpf: &mut Bpf<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = bpf.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_bpf_process_pcm_frames(
                bpf.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_bpf_get_latency<F: PcmFormat>(bpf: &Bpf<F>) -> u32 {
        unsafe { sys::ma_bpf_get_latency(bpf.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for Bpf<F> {
    fn drop(&mut self) {
        bpf_ffi::ma_bpf_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const CHANNELS: u32 = 2;
    const CUTOFF_FREQ: f64 = 1_000.0;
    const ORDER: u32 = 2;

    #[test]
    fn bpf_builder_build_i16_does_not_panic() {
        let mut builder = BpfBuilder::new(CHANNELS, SampleRate::Sr48000, CUTOFF_FREQ, ORDER);

        let bpf = builder.build_i16().unwrap();

        assert!(!bpf.to_raw().is_null());
        assert_eq!(bpf.channels, CHANNELS);
        assert_eq!(bpf.order, ORDER);
        assert_eq!(bpf.format, Format::S16);
    }

    #[test]
    fn bpf_filter_test_builder_build_f32_does_not_panic() {
        let mut builder = BpfBuilder::new(CHANNELS, SampleRate::Sr48000, CUTOFF_FREQ, ORDER);

        let bpf = builder.build_f32().unwrap();

        assert!(!bpf.to_raw().is_null());
        assert_eq!(bpf.channels, CHANNELS);
        assert_eq!(bpf.order, ORDER);
        assert_eq!(bpf.format, Format::F32);
    }

    #[test]
    fn bpf_filter_test_get_latency_does_not_panic() {
        let mut builder = BpfBuilder::new(CHANNELS, SampleRate::Sr48000, CUTOFF_FREQ, ORDER);

        let bpf = builder.build_f32().unwrap();

        let _ = bpf.get_latency();
    }

    #[test]
    fn bpf_filter_test_reinit_does_not_panic() {
        let mut builder = BpfBuilder::new(CHANNELS, SampleRate::Sr48000, CUTOFF_FREQ, ORDER);

        let mut bpf = builder.build_f32().unwrap();

        bpf.reinit(SampleRate::Sr48000, 2_000.0).unwrap();

        assert!(!bpf.to_raw().is_null());
        assert_eq!(bpf.channels, CHANNELS);
        assert_eq!(bpf.format, Format::F32);
    }

    #[test]
    fn bpf_filter_test_can_be_created_reinitialized_and_dropped_repeatedly() {
        for _ in 0..1024 {
            let mut builder = BpfBuilder::new(CHANNELS, SampleRate::Sr48000, CUTOFF_FREQ, ORDER);

            let mut bpf = builder.build_f32().unwrap();

            bpf.reinit(SampleRate::Sr48000, 2_000.0).unwrap();

            assert!(!bpf.to_raw().is_null());
        }
    }

    #[test]
    fn bpf_filter_test_process_f32_does_not_panic() {
        let mut builder = BpfBuilder::new(CHANNELS, SampleRate::Sr48000, CUTOFF_FREQ, ORDER);

        let mut bpf = builder.build_f32().unwrap();

        let input = [0.0_f32, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7];

        let mut output = [0.0_f32; 8];

        bpf.process_pcm_frames(&mut output, &input).unwrap();

        assert!(!output.iter().any(|sample| sample.is_nan()));
    }

    #[test]
    fn bpf_filter_test_process_i16_does_not_panic() {
        let mut builder = BpfBuilder::new(CHANNELS, SampleRate::Sr48000, CUTOFF_FREQ, ORDER);

        let mut bpf = builder.build_i16().unwrap();

        let input = [0_i16, 100, 200, 300, 400, 500, 600, 700];

        let mut output = [0_i16; 8];

        bpf.process_pcm_frames(&mut output, &input).unwrap();
    }

    #[test]
    #[should_panic]
    fn bpf_filter_test_process_panics_when_input_and_output_lengths_differ() {
        let mut builder = BpfBuilder::new(CHANNELS, SampleRate::Sr48000, CUTOFF_FREQ, ORDER);

        let mut bpf = builder.build_f32().unwrap();

        let input = [0.0_f32; 8];
        let mut output = [0.0_f32; 6];

        let res = bpf.process_pcm_frames(&mut output, &input);
        assert!(res.is_err())
    }

    #[test]
    #[should_panic]
    fn bpf_filter_test_process_panics_when_input_does_not_contain_whole_frames() {
        let mut builder = BpfBuilder::new(CHANNELS, SampleRate::Sr48000, CUTOFF_FREQ, ORDER);

        let mut bpf = builder.build_f32().unwrap();

        let input = [0.0_f32; 7];
        let mut output = [0.0_f32; 7];

        let res = bpf.process_pcm_frames(&mut output, &input);
        assert!(res.is_err())
    }
}
