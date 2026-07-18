use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    data_source::DataFormat,
    pcm_frames::PcmFormat,
    Binding, MaResult,
};

pub struct Fader<F: PcmFormat> {
    inner: *mut sys::ma_fader,
    format: Format,
    channels: u32,
    sample_rate: SampleRate,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for Fader<F> {}

impl<F: PcmFormat> Binding for Fader<F> {
    type Raw = *mut sys::ma_fader;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Fader<F> {
    fn build(config: &sys::ma_fader_config, format: Format) -> MaResult<Fader<F>> {
        let channels = config.channels;
        let sample_rate: SampleRate = config.sampleRate.try_into()?;
        let mut inner: Box<MaybeUninit<sys::ma_fader>> = Box::new(MaybeUninit::uninit());
        fader_ffi::ma_fader_init(config, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(inner) as *mut sys::ma_fader;
        Ok(Fader {
            inner: inner_ptr,
            channels,
            format,
            sample_rate,
            _format: PhantomData,
        })
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        fader_ffi::ma_fader_process_pcm_frames(self, frames_out, frames_in)
    }

    pub fn get_data_format(&self) -> DataFormat {
        DataFormat {
            format: self.format,
            channels: self.channels,
            sample_rate: self.sample_rate,
            channel_map: None,
        }
    }

    pub fn set_fade(&mut self, vol_start: f32, vol_end: f32, length_frames: u64) {
        fader_ffi::ma_fader_set_fade(self, vol_start, vol_end, length_frames);
    }

    pub fn set_fade_with_offset(
        &mut self,
        vol_start: f32,
        vol_end: f32,
        length_frames: u64,
        start_off_frames: i64,
    ) {
        fader_ffi::ma_fader_set_fade_ex(self, vol_start, vol_end, length_frames, start_off_frames);
    }

    pub fn current_volume(&self) -> f32 {
        fader_ffi::ma_fader_get_current_volume(self)
    }
}

pub struct FaderBuilder {
    config: sys::ma_fader_config,
}

impl FaderBuilder {
    pub fn new(channels: u32, sample_rate: SampleRate) -> Self {
        let config =
            unsafe { sys::ma_fader_config_init(Format::F32.into(), channels, sample_rate.into()) };
        Self { config }
    }

    pub fn build_f32(&mut self) -> MaResult<Fader<f32>> {
        self.config.format = Format::F32.into();
        Fader::<f32>::build(&self.config, Format::F32)
    }
}

mod fader_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        audio::dsp::fader::Fader, data_source::DataFormat, pcm_frames::PcmFormat, Binding,
        MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_fader_init(config: &sys::ma_fader_config, fader: *mut sys::ma_fader) -> MaResult<()> {
        let res = unsafe { sys::ma_fader_init(config as *const _, fader) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_fader_process_pcm_frames<F: PcmFormat>(
        fader: &mut Fader<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = fader.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_fader_process_pcm_frames(
                fader.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    // Data is stored on the Fader struct instead.
    #[inline]
    #[allow(unused)]
    pub fn ma_fader_get_data_format<F: PcmFormat>(fader: &Fader<F>) -> MaResult<DataFormat> {
        let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        let mut channels: u32 = 0;
        let mut sample_rate: u32 = 0;
        unsafe {
            sys::ma_fader_get_data_format(
                fader.to_raw(),
                &mut format_raw,
                &mut channels,
                &mut sample_rate,
            )
        };

        Ok(DataFormat {
            format: format_raw.try_into()?,
            channels,
            sample_rate: sample_rate.try_into()?,
            channel_map: None,
        })
    }

    #[inline]
    pub fn ma_fader_set_fade<F: PcmFormat>(
        fader: &mut Fader<F>,
        vol_start: f32,
        vol_end: f32,
        length_frames: u64,
    ) {
        unsafe {
            sys::ma_fader_set_fade(fader.to_raw(), vol_start, vol_end, length_frames);
        }
    }

    #[inline]
    pub fn ma_fader_set_fade_ex<F: PcmFormat>(
        fader: &mut Fader<F>,
        vol_beg: f32,
        vol_enf: f32,
        length_frames: u64,
        start_offset_frames: i64,
    ) {
        unsafe {
            sys::ma_fader_set_fade_ex(
                fader.to_raw(),
                vol_beg,
                vol_enf,
                length_frames,
                start_offset_frames,
            );
        }
    }

    #[inline]
    pub fn ma_fader_get_current_volume<F: PcmFormat>(fader: &Fader<F>) -> f32 {
        unsafe { sys::ma_fader_get_current_volume(fader.to_raw()) }
    }
}

impl<F: PcmFormat> Drop for Fader<F> {
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

    fn assert_between(value: f32, min: f32, max: f32) {
        assert!(
            value >= min && value <= max,
            "expected {value} to be between {min} and {max}"
        );
    }

    #[test]
    fn fader_test_build_f32_initializes_expected_data_format() {
        let mut builder = FaderBuilder::new(2, SampleRate::Sr44100);
        let fader = builder.build_f32().unwrap();

        let format = fader.get_data_format();

        assert_eq!(format.format, Format::F32);
        assert_eq!(format.channels, 2);
        assert_eq!(format.sample_rate, SampleRate::Sr44100);
        assert!(format.channel_map.is_none());
    }

    #[test]
    fn fader_test_ffi_data_format_matches_config() {
        let mut builder = FaderBuilder::new(2, SampleRate::Sr44100);
        let fader = builder.build_f32().unwrap();

        let format = fader_ffi::ma_fader_get_data_format(&fader).unwrap();

        assert_eq!(format.format, Format::F32);
        assert_eq!(format.channels, 2);
        assert_eq!(format.sample_rate, SampleRate::Sr44100);
        assert!(format.channel_map.is_none());
    }

    #[test]
    fn fader_test_process_pcm_frames_without_fade_copies_input() {
        let mut builder = FaderBuilder::new(2, SampleRate::Sr44100);
        let mut fader = builder.build_f32().unwrap();

        let input = [0.25, -0.25, 0.5, -0.5, 1.0, -1.0];
        let mut output = [0.0; 6];

        fader.process_pcm_frames(&mut output, &input).unwrap();

        assert_eq!(output, input);
        assert_approx_eq(fader.current_volume(), 1.0);
    }

    #[test]
    fn fader_test_set_fade_to_zero_reduces_output_over_time() {
        let mut builder = FaderBuilder::new(1, SampleRate::Sr44100);
        let mut fader = builder.build_f32().unwrap();

        fader.set_fade(1.0, 0.0, 8);

        let input = [1.0; 8];
        let mut output = [0.0; 8];

        fader.process_pcm_frames(&mut output, &input).unwrap();

        assert_between(output[0], 0.0, 1.0);
        assert_between(output[7], 0.0, 1.0);

        for pair in output.windows(2) {
            assert!(
                pair[0] >= pair[1],
                "fade-out output should not increase: {:?}",
                output
            );
        }

        assert!(
            output[0] > output[7],
            "fade-out should reduce volume: {:?}",
            output
        );

        assert_between(fader.current_volume(), 0.0, 1.0);
    }

    #[test]
    fn fader_test_set_fade_to_one_increases_output_over_time() {
        let mut builder = FaderBuilder::new(1, SampleRate::Sr44100);
        let mut fader = builder.build_f32().unwrap();

        fader.set_fade(0.0, 1.0, 8);

        let input = [1.0; 8];
        let mut output = [0.0; 8];

        fader.process_pcm_frames(&mut output, &input).unwrap();

        assert_between(output[0], 0.0, 1.0);
        assert_between(output[7], 0.0, 1.0);

        for pair in output.windows(2) {
            assert!(
                pair[0] <= pair[1],
                "fade-in output should not decrease: {:?}",
                output
            );
        }

        assert!(
            output[0] < output[7],
            "fade-in should increase volume: {:?}",
            output
        );

        assert_between(fader.current_volume(), 0.0, 1.0);
    }

    #[test]
    fn fader_test_set_fade_with_offset_delays_the_fade() {
        let mut builder = FaderBuilder::new(1, SampleRate::Sr44100);
        let mut fader = builder.build_f32().unwrap();

        fader.set_fade_with_offset(1.0, 0.0, 4, 2);

        let input = [1.0; 8];
        let mut output = [0.0; 8];

        fader.process_pcm_frames(&mut output, &input).unwrap();

        assert_approx_eq(output[0], 1.0);
        assert_approx_eq(output[1], 1.0);

        assert!(
            output[2] >= output[3],
            "fade should have started after offset: {:?}",
            output
        );

        assert!(
            output[1] > output[7],
            "output after fade should be lower than pre-offset output: {:?}",
            output
        );
    }

    #[test]
    fn fader_test_process_pcm_frames_only_processes_minimum_frame_count() {
        let mut builder = FaderBuilder::new(2, SampleRate::Sr44100);
        let mut fader = builder.build_f32().unwrap();

        // 1 stereo frame in, 2 stereo frames out.
        let input = [0.25, -0.25];
        let mut output = [99.0, 99.0, 99.0, 99.0];

        fader.process_pcm_frames(&mut output, &input).unwrap();

        assert_approx_eq(output[0], 0.25);
        assert_approx_eq(output[1], -0.25);

        // The second output frame should be untouched because only 1 frame was processed.
        assert_approx_eq(output[2], 99.0);
        assert_approx_eq(output[3], 99.0);
    }

    #[test]
    fn fader_test_process_pcm_frames_with_empty_input_does_not_touch_output() {
        let mut builder = FaderBuilder::new(2, SampleRate::Sr44100);
        let mut fader = builder.build_f32().unwrap();

        let input = [];
        let mut output = [42.0, 42.0];

        fader.process_pcm_frames(&mut output, &input).unwrap();

        assert_eq!(output, [42.0, 42.0]);
    }

    #[test]
    fn fader_test_multiple_process_calls_continue_fade_state() {
        let mut builder = FaderBuilder::new(1, SampleRate::Sr44100);
        let mut fader = builder.build_f32().unwrap();

        fader.set_fade(1.0, 0.0, 8);

        let input_a = [1.0; 4];
        let input_b = [1.0; 4];

        let mut output_a = [0.0; 4];
        let mut output_b = [0.0; 4];

        fader.process_pcm_frames(&mut output_a, &input_a).unwrap();
        fader.process_pcm_frames(&mut output_b, &input_b).unwrap();

        assert!(
            output_a[0] > output_a[3],
            "first process call should fade down: {:?}",
            output_a
        );

        assert!(
            output_a[3] >= output_b[0],
            "second process call should continue from previous fade state: {:?} then {:?}",
            output_a,
            output_b
        );

        assert!(
            output_b[0] > output_b[3],
            "second process call should keep fading down: {:?}",
            output_b
        );
    }
}
