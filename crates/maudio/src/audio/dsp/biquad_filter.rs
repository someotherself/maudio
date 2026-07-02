use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{audio::formats::Format, pcm_frames::PcmFormat, Binding, MaResult};

pub struct Biquad<F: PcmFormat> {
    inner: *mut sys::ma_biquad,
    format: Format,
    channels: u32,
    _format: PhantomData<F>,
}

impl<F: PcmFormat> Binding for Biquad<F> {
    type Raw = *mut sys::ma_biquad;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Biquad<F> {
    fn build(config: &sys::ma_biquad_config, format: Format) -> MaResult<Biquad<F>> {
        let channels = config.channels;
        let mut inner: MaybeUninit<sys::ma_biquad> = MaybeUninit::uninit();
        biquad_ffi::ma_biquad_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));
        Ok(Biquad {
            inner: inner_ptr,
            format,
            channels,
            _format: PhantomData,
        })
    }

    pub fn reinit(&mut self, b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> MaResult<()> {
        let config = unsafe {
            sys::ma_biquad_config_init(self.format.into(), self.channels, b0, b1, b2, a0, a1, a2)
        };

        biquad_ffi::ma_biquad_reinit(&config, self)
    }

    pub fn get_latency(&self) -> u32 {
        biquad_ffi::ma_biquad_get_latency(self)
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        biquad_ffi::ma_biquad_process_pcm_frames(self, frames_out, frames_in)
    }
}

pub struct BiquadBuilder {
    config: sys::ma_biquad_config,
}

impl BiquadBuilder {
    pub fn new(channels: u32, b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        let config = unsafe {
            sys::ma_biquad_config_init(Format::S16.into(), channels, b0, b1, b2, a0, a1, a2)
        };
        Self { config }
    }

    pub fn build_i16(&mut self) -> MaResult<Biquad<i16>> {
        self.config.format = Format::S16.into();
        Biquad::<i16>::build(&self.config, Format::S16)
    }

    pub fn build_f32(&mut self) -> MaResult<Biquad<f32>> {
        self.config.format = Format::F32.into();
        Biquad::<f32>::build(&self.config, Format::F32)
    }
}

impl<F: PcmFormat> Drop for Biquad<F> {
    fn drop(&mut self) {
        biquad_ffi::ma_biquad_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

pub(crate) mod biquad_ffi {
    use std::sync::Arc;

    use crate::{
        audio::dsp::biquad_filter::Biquad, engine::AllocationCallbacks, pcm_frames::PcmFormat,
        AsRawRef, Binding, MaResult, MaudioError,
    };
    use maudio_sys::ffi as sys;

    pub fn ma_biquad_init(
        config: &sys::ma_biquad_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        biquad: *mut sys::ma_biquad,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_biquad_init(config as *const _, alloc_cb, biquad) };
        MaudioError::check(res)
    }

    pub fn ma_biquad_reinit<F: PcmFormat>(
        config: &sys::ma_biquad_config,
        biquad: &mut Biquad<F>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_biquad_reinit(config as *const _, biquad.to_raw()) };
        MaudioError::check(res)
    }

    pub fn ma_biquad_uninit<F: PcmFormat>(biquad: &mut Biquad<F>) {
        unsafe {
            sys::ma_biquad_uninit(biquad.to_raw(), std::ptr::null_mut());
        };
    }

    pub fn ma_biquad_process_pcm_frames<F: PcmFormat>(
        biquad: &mut Biquad<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = biquad.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_biquad_process_pcm_frames(
                biquad.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    pub fn ma_biquad_get_latency<F: PcmFormat>(biquad: &Biquad<F>) -> u32 {
        unsafe { sys::ma_biquad_get_latency(biquad.to_raw()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B0: f64 = 1.0;
    const B1: f64 = 0.0;
    const B2: f64 = 0.0;
    const A0: f64 = 1.0;
    const A1: f64 = 0.0;
    const A2: f64 = 0.0;

    #[test]
    fn biquad_filter_test_new_i16_does_not_panic() {
        let biquad = BiquadBuilder::new(2, B0, B1, B2, A0, A1, A2)
            .build_i16()
            .unwrap();

        assert!(!biquad.to_raw().is_null());
        assert_eq!(biquad.channels, 2);
        assert_eq!(biquad.format, Format::S16);
    }

    #[test]
    fn biquad_filter_test_new_f32_does_not_panic() {
        let biquad = BiquadBuilder::new(2, B0, B1, B2, A0, A1, A2)
            .build_f32()
            .unwrap();

        assert!(!biquad.to_raw().is_null());
        assert_eq!(biquad.channels, 2);
        assert_eq!(biquad.format, Format::F32);
    }

    #[test]
    fn biquad_filter_test_get_latency_does_not_panic() {
        let biquad = BiquadBuilder::new(2, B0, B1, B2, A0, A1, A2)
            .build_f32()
            .unwrap();

        let _ = biquad.get_latency();
    }

    #[test]
    fn biquad_filter_test_reinit_does_not_panic() {
        let mut biquad = BiquadBuilder::new(2, B0, B1, B2, A0, A1, A2)
            .build_f32()
            .unwrap();

        biquad.reinit(0.5, 0.25, 0.25, 1.0, 0.0, 0.0).unwrap();

        assert!(!biquad.to_raw().is_null());
        assert_eq!(biquad.channels, 2);
        assert_eq!(biquad.format, Format::F32);
    }

    #[test]
    fn biquad_filter_test_can_be_created_reinitialized_and_dropped_repeatedly() {
        for _ in 0..1024 {
            let mut biquad = BiquadBuilder::new(2, B0, B1, B2, A0, A1, A2)
                .build_f32()
                .unwrap();

            biquad.reinit(B0, B1, B2, A0, A1, A2).unwrap();

            assert!(!biquad.to_raw().is_null());
        }
    }

    #[test]
    fn biquad_filter_test_process_f32_identity_filter_does_not_panic() {
        let mut biquad = BiquadBuilder::new(2, B0, B1, B2, A0, A1, A2)
            .build_f32()
            .unwrap();

        let input = [0.0_f32, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7];

        let mut output = [0.0_f32; 8];

        biquad.process_pcm_frames(&mut output, &input).unwrap();

        assert_eq!(output, input);
    }

    #[test]
    fn biquad_filter_test_process_after_reinit_does_not_panic() {
        let mut biquad = BiquadBuilder::new(2, B0, B1, B2, A0, A1, A2)
            .build_f32()
            .unwrap();

        biquad.reinit(0.5, 0.25, 0.25, 1.0, 0.0, 0.0).unwrap();

        let input = [0.0_f32, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7];

        let mut output = [0.0_f32; 8];

        biquad.process_pcm_frames(&mut output, &input).unwrap();

        assert!(!output.iter().any(|sample| sample.is_nan()));
    }
}
