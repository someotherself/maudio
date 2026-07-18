use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, pan::PanMode},
    pcm_frames::{PcmFormat, S24Packed},
    Binding, MaResult,
};

pub struct Panner<F: PcmFormat> {
    inner: *mut sys::ma_panner,
    channels: u32,
    #[allow(unused)]
    format: Format,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for Panner<F> {}

impl<F: PcmFormat> Binding for Panner<F> {
    type Raw = *mut sys::ma_panner;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Panner<F> {
    fn build(config: &sys::ma_panner_config, format: Format) -> MaResult<Panner<F>> {
        let channels = config.channels;
        let mut inner: Box<MaybeUninit<sys::ma_panner>> = Box::new(MaybeUninit::uninit());
        panner_ffi::ma_panner_init(config, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(inner) as *mut sys::ma_panner;
        Ok(Panner {
            inner: inner_ptr,
            channels,
            format,
            _format: PhantomData,
        })
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        panner_ffi::ma_panner_process_pcm_frames(self, frames_out, frames_in)
    }

    pub fn set_pan(&mut self, pan: f32) {
        panner_ffi::ma_panner_set_pan(self, pan);
    }

    pub fn get_pan(&self) -> f32 {
        panner_ffi::ma_panner_get_pan(self)
    }

    pub fn set_mode(&mut self, mode: PanMode) {
        panner_ffi::ma_panner_set_mode(self, mode);
    }

    pub fn get_mode(&self) -> MaResult<PanMode> {
        panner_ffi::ma_panner_get_mode(self)
    }
}

pub struct PannerBuilder {
    config: sys::ma_panner_config,
}

impl PannerBuilder {
    pub fn new(channels: u32) -> Self {
        let config = unsafe { sys::ma_panner_config_init(Format::U8.into(), channels) };
        Self { config }
    }

    pub fn build_u8(&mut self) -> MaResult<Panner<u8>> {
        self.config.format = Format::U8.into();
        Panner::<u8>::build(&self.config, Format::U8)
    }

    pub fn build_i16(&mut self) -> MaResult<Panner<i16>> {
        self.config.format = Format::S16.into();
        Panner::<i16>::build(&self.config, Format::S16)
    }

    pub fn build_i32(&mut self) -> MaResult<Panner<i32>> {
        self.config.format = Format::S32.into();
        Panner::<i32>::build(&self.config, Format::S32)
    }

    pub fn build_s24_packed(&mut self) -> MaResult<Panner<S24Packed>> {
        self.config.format = Format::S24Packed.into();
        Panner::<S24Packed>::build(&self.config, Format::S24Packed)
    }

    pub fn build_f32(&mut self) -> MaResult<Panner<f32>> {
        self.config.format = Format::F32.into();
        Panner::<f32>::build(&self.config, Format::F32)
    }
}

mod panner_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        audio::{dsp::stereo_panner::Panner, pan::PanMode},
        pcm_frames::PcmFormat,
        Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_panner_init(
        config: &sys::ma_panner_config,
        gainer: *mut sys::ma_panner,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_panner_init(config as *const _, gainer) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_panner_process_pcm_frames<F: PcmFormat>(
        panner: &mut Panner<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = panner.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_panner_process_pcm_frames(
                panner.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_panner_set_mode<F: PcmFormat>(panner: &mut Panner<F>, mode: PanMode) {
        unsafe {
            sys::ma_panner_set_mode(panner.to_raw(), mode.into());
        }
    }

    #[inline]
    pub fn ma_panner_get_mode<F: PcmFormat>(panner: &Panner<F>) -> MaResult<PanMode> {
        let mode = unsafe { sys::ma_panner_get_mode(panner.to_raw()) };
        mode.try_into()
    }

    #[inline]
    pub fn ma_panner_set_pan<F: PcmFormat>(panner: &mut Panner<F>, pan: f32) {
        unsafe { sys::ma_panner_set_pan(panner.to_raw(), pan) };
    }

    #[inline]
    pub fn ma_panner_get_pan<F: PcmFormat>(panner: &Panner<F>) -> f32 {
        unsafe { sys::ma_panner_get_pan(panner.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for Panner<F> {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx_eq(actual: f32, expected: f32) {
        let diff = (actual - expected).abs();
        assert!(
            diff <= 1e-5,
            "expected {expected}, got {actual}, diff {diff}"
        );
    }

    fn assert_not_approx_eq(actual: f32, expected: f32) {
        let diff = (actual - expected).abs();
        assert!(
            diff > 1e-5,
            "expected {actual} to be different from {expected}"
        );
    }

    #[test]
    fn panner_test_build_f32_initializes_expected_fields() {
        let mut builder = PannerBuilder::new(2);
        let panner = builder.build_f32().unwrap();

        assert_eq!(panner.channels, 2);
        assert_eq!(panner.format, Format::F32);
    }

    #[test]
    fn panner_test_build_u8_initializes_expected_fields() {
        let mut builder = PannerBuilder::new(2);
        let panner = builder.build_u8().unwrap();

        assert_eq!(panner.channels, 2);
        assert_eq!(panner.format, Format::U8);
    }

    #[test]
    fn panner_test_build_i16_initializes_expected_fields() {
        let mut builder = PannerBuilder::new(2);
        let panner = builder.build_i16().unwrap();

        assert_eq!(panner.channels, 2);
        assert_eq!(panner.format, Format::S16);
    }

    #[test]
    fn panner_test_build_i32_initializes_expected_fields() {
        let mut builder = PannerBuilder::new(2);
        let panner = builder.build_i32().unwrap();

        assert_eq!(panner.channels, 2);
        assert_eq!(panner.format, Format::S32);
    }

    #[test]
    fn panner_test_build_s24_packed_initializes_expected_fields() {
        let mut builder = PannerBuilder::new(2);
        let panner = builder.build_s24_packed().unwrap();

        assert_eq!(panner.channels, 2);
        assert_eq!(panner.format, Format::S24Packed);
    }

    #[test]
    fn panner_test_default_pan_is_center() {
        let mut builder = PannerBuilder::new(2);
        let panner = builder.build_f32().unwrap();

        assert_approx_eq(panner.get_pan(), 0.0);
    }

    #[test]
    fn panner_test_set_pan_updates_pan_value() {
        let mut builder = PannerBuilder::new(2);
        let mut panner = builder.build_f32().unwrap();

        panner.set_pan(-0.5);
        assert_approx_eq(panner.get_pan(), -0.5);

        panner.set_pan(0.75);
        assert_approx_eq(panner.get_pan(), 0.75);
    }

    #[test]
    fn panner_test_set_mode_updates_mode_value() {
        let mut builder = PannerBuilder::new(2);
        let mut panner = builder.build_f32().unwrap();

        panner.set_mode(PanMode::Balance);
        assert_eq!(panner.get_mode().unwrap(), PanMode::Balance);

        panner.set_mode(PanMode::Pan);
        assert_eq!(panner.get_mode().unwrap(), PanMode::Pan);
    }

    #[test]
    fn panner_test_process_pcm_frames_center_pan_preserves_identical_stereo_signal() {
        let mut builder = PannerBuilder::new(2);
        let mut panner = builder.build_f32().unwrap();

        panner.set_pan(0.0);

        let input = [
            1.0, 1.0, //
            0.5, 0.5, //
            -0.25, -0.25,
        ];
        let mut output = [0.0; 6];

        panner.process_pcm_frames(&mut output, &input).unwrap();

        for (actual, expected) in output.iter().zip(input.iter()) {
            assert_approx_eq(*actual, *expected);
        }
    }

    #[test]
    fn panner_test_process_pcm_frames_left_pan_changes_right_channel() {
        let mut builder = PannerBuilder::new(2);
        let mut panner = builder.build_f32().unwrap();

        panner.set_pan(-1.0);

        let input = [
            1.0, 1.0, //
            1.0, 1.0,
        ];
        let mut output = [0.0; 4];

        panner.process_pcm_frames(&mut output, &input).unwrap();

        assert_approx_eq(output[0], 1.0);
        assert_not_approx_eq(output[1], 1.0);

        assert_approx_eq(output[2], 1.0);
        assert_not_approx_eq(output[3], 1.0);
    }

    #[test]
    fn panner_test_process_pcm_frames_right_pan_changes_left_channel() {
        let mut builder = PannerBuilder::new(2);
        let mut panner = builder.build_f32().unwrap();

        panner.set_pan(1.0);

        let input = [
            1.0, 1.0, //
            1.0, 1.0,
        ];
        let mut output = [0.0; 4];

        panner.process_pcm_frames(&mut output, &input).unwrap();

        assert_not_approx_eq(output[0], 1.0);
        assert_approx_eq(output[1], 1.0);

        assert_not_approx_eq(output[2], 1.0);
        assert_approx_eq(output[3], 1.0);
    }

    #[test]
    fn panner_test_process_pcm_frames_only_processes_minimum_frame_count() {
        let mut builder = PannerBuilder::new(2);
        let mut panner = builder.build_f32().unwrap();

        // 1 stereo frame in, 2 stereo frames out.
        let input = [0.25, -0.25];
        let mut output = [99.0, 99.0, 99.0, 99.0];

        panner.process_pcm_frames(&mut output, &input).unwrap();

        assert_approx_eq(output[0], 0.25);
        assert_approx_eq(output[1], -0.25);

        // The second output frame should be untouched because only 1 frame was processed.
        assert_approx_eq(output[2], 99.0);
        assert_approx_eq(output[3], 99.0);
    }

    #[test]
    fn panner_test_process_pcm_frames_with_empty_input_does_not_touch_output() {
        let mut builder = PannerBuilder::new(2);
        let mut panner = builder.build_f32().unwrap();

        let input = [];
        let mut output = [42.0, 42.0];

        panner.process_pcm_frames(&mut output, &input).unwrap();

        assert_eq!(output, [42.0, 42.0]);
    }

    #[test]
    fn panner_test_process_pcm_frames_u8_center_pan_runs_successfully() {
        let mut builder = PannerBuilder::new(2);
        let mut panner = builder.build_u8().unwrap();

        let input = [
            128_u8, 128_u8, //
            255_u8, 255_u8,
        ];
        let mut output = [0_u8; 4];

        panner.process_pcm_frames(&mut output, &input).unwrap();

        assert_eq!(output, input);
    }

    #[test]
    fn panner_test_process_pcm_frames_i16_center_pan_runs_successfully() {
        let mut builder = PannerBuilder::new(2);
        let mut panner = builder.build_i16().unwrap();

        let input = [
            100_i16, 100_i16, //
            -100_i16, -100_i16,
        ];
        let mut output = [0_i16; 4];

        panner.process_pcm_frames(&mut output, &input).unwrap();

        assert_eq!(output, input);
    }

    #[test]
    fn panner_test_process_pcm_frames_i32_center_pan_runs_successfully() {
        let mut builder = PannerBuilder::new(2);
        let mut panner = builder.build_i32().unwrap();

        let input = [
            100_i32, 100_i32, //
            -100_i32, -100_i32,
        ];
        let mut output = [0_i32; 4];

        panner.process_pcm_frames(&mut output, &input).unwrap();

        assert_eq!(output, input);
    }
}
