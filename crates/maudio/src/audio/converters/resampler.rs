use std::{marker::PhantomData, mem::MaybeUninit, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    engine::AllocationCallbacks,
    pcm_frames::PcmFormat,
    AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum ResampleAlgorithm {
    Linear,
    Custom,
}

impl From<ResampleAlgorithm> for sys::ma_resample_algorithm {
    fn from(value: ResampleAlgorithm) -> Self {
        match value {
            ResampleAlgorithm::Linear => sys::ma_resample_algorithm_ma_resample_algorithm_linear,
            ResampleAlgorithm::Custom => sys::ma_resample_algorithm_ma_resample_algorithm_custom,
        }
    }
}

impl TryFrom<sys::ma_resample_algorithm> for ResampleAlgorithm {
    type Error = MaudioError;

    fn try_from(value: sys::ma_resample_algorithm) -> Result<Self, Self::Error> {
        match value {
            sys::ma_resample_algorithm_ma_resample_algorithm_linear => {
                Ok(ResampleAlgorithm::Linear)
            }
            sys::ma_resample_algorithm_ma_resample_algorithm_custom => {
                Ok(ResampleAlgorithm::Custom)
            }
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                ResampleAlgorithm,
            >(other as i64))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResamplerOutput {
    pub frames_consumed: usize,
    pub frames_produced: usize,
    pub channels: u32,
}

pub struct Resampler<F: PcmFormat> {
    inner: *mut sys::ma_resampler,
    channels: u32,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for Resampler<F> {}

impl<F: PcmFormat> Binding for Resampler<F> {
    type Raw = *mut sys::ma_resampler;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Resampler<F> {
    pub fn process_pcm_frames_into(
        &mut self,
        input: &[F::StorageUnit],
        output: &mut [F::StorageUnit],
    ) -> MaResult<ResamplerOutput> {
        resampler_ffi::ma_resampler_process_pcm_frames_into(self, input, output)
    }

    pub fn process_pcm_frames(
        &mut self,
        input: &[F::StorageUnit],
    ) -> MaResult<Vec<F::StorageUnit>> {
        resampler_ffi::ma_resampler_process_pcm_frames(self, input)
    }

    pub fn set_rate(&mut self, rate_in: SampleRate, rate_out: SampleRate) -> MaResult<()> {
        resampler_ffi::ma_resampler_set_rate(self, rate_in, rate_out)
    }

    pub fn set_rate_ratio(&mut self, ratio: f32) -> MaResult<()> {
        resampler_ffi::ma_resampler_set_rate_ratio(self, ratio)
    }

    pub fn input_latency(&self) -> u64 {
        resampler_ffi::ma_resampler_get_input_latency(self)
    }

    pub fn output_latency(&self) -> u64 {
        resampler_ffi::ma_resampler_get_output_latency(self)
    }

    pub fn required_input_frames(&self, output_frame_count: u64) -> MaResult<u64> {
        resampler_ffi::ma_resampler_get_required_input_frame_count(self, output_frame_count)
    }

    pub fn required_output_frames(&self, input_frame_count: u64) -> MaResult<u64> {
        resampler_ffi::ma_resampler_get_expected_output_frame_count(self, input_frame_count)
    }

    pub fn reset(&mut self) -> MaResult<()> {
        resampler_ffi::ma_resampler_reset(self)
    }
}

// Private methods
impl<F: PcmFormat> Resampler<F> {
    fn new_with_config(
        config: &ResamplerBuilder,
        alloc_cb: Option<Arc<AllocationCallbacks>>,
    ) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_resampler>> = Box::new(MaybeUninit::uninit());

        resampler_ffi::ma_resampler_init(config, alloc_cb.clone(), mem.as_mut_ptr())?;

        let inner: *mut sys::ma_resampler = Box::into_raw(mem) as *mut sys::ma_resampler;

        Ok(Self {
            inner,
            channels: config.channels,
            alloc_cb,
            _format: PhantomData,
        })
    }
}

mod resampler_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::{
            converters::resampler::{Resampler, ResamplerBuilder, ResamplerOutput},
            sample_rate::SampleRate,
        },
        engine::AllocationCallbacks,
        pcm_frames::PcmFormat,
        AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_resampler_init(
        config: &ResamplerBuilder,
        alloc: Option<Arc<AllocationCallbacks>>,
        resampler: *mut sys::ma_resampler,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_resampler_init(config.as_raw_ptr(), alloc_cb, resampler) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_resampler_uninit<F: PcmFormat>(
        resampler: &mut Resampler<F>,
        alloc: Option<Arc<AllocationCallbacks>>,
    ) {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        unsafe {
            sys::ma_resampler_uninit(resampler.to_raw(), alloc_cb);
        }
    }

    #[inline]
    pub fn ma_resampler_process_pcm_frames<F: PcmFormat>(
        resampler: &mut Resampler<F>,
        input: &[F::StorageUnit],
    ) -> MaResult<Vec<F::StorageUnit>> {
        if input.len() / resampler.channels as usize == 0 {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Input length too small",
            )));
        }

        let in_frames = (input.len() / resampler.channels as usize) as u64;

        let req_frames_out = resampler.required_output_frames(in_frames)?;
        let req_frames_len = req_frames_out as usize * resampler.channels as usize;
        let mut out = vec![F::STORE_SILENCE; req_frames_len];

        let res = resampler.process_pcm_frames_into(input, &mut out)?;
        // TODO: Test this
        if res.frames_consumed != in_frames as usize
            || res.frames_produced != req_frames_out as usize
        {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Invalid length",
            )));
        }

        Ok(out)
    }

    #[inline]
    pub fn ma_resampler_process_pcm_frames_into<F: PcmFormat>(
        resampler: &mut Resampler<F>,
        input: &[F::StorageUnit],
        output: &mut [F::StorageUnit],
    ) -> MaResult<ResamplerOutput> {
        if input.len() / resampler.channels as usize == 0 {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Input length too small",
            )));
        }

        if output.len() / resampler.channels as usize == 0 {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Output length too small",
            )));
        }

        let mut in_frames = (input.len() / resampler.channels as usize) as u64;
        let mut out_frames = (output.len() / resampler.channels as usize) as u64;

        let res = unsafe {
            sys::ma_resampler_process_pcm_frames(
                resampler.to_raw(),
                input.as_ptr().cast(),
                &mut in_frames,
                output.as_mut_ptr().cast(),
                &mut out_frames,
            )
        };

        MaudioError::check(res)?;
        Ok(ResamplerOutput {
            frames_consumed: in_frames as usize,
            frames_produced: out_frames as usize,
            channels: resampler.channels,
        })
    }

    #[inline]
    pub fn ma_resampler_set_rate<F: PcmFormat>(
        resampler: &Resampler<F>,
        sampler_rate_in: SampleRate,
        sampler_rate_out: SampleRate,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resampler_set_rate(
                resampler.to_raw(),
                sampler_rate_in.into(),
                sampler_rate_out.into(),
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_resampler_set_rate_ratio<F: PcmFormat>(
        resampler: &Resampler<F>,
        ratio: f32,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resampler_set_rate_ratio(resampler.to_raw(), ratio) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_resampler_get_input_latency<F: PcmFormat>(resampler: &Resampler<F>) -> u64 {
        unsafe { sys::ma_resampler_get_input_latency(resampler.to_raw()) }
    }

    #[inline]
    pub fn ma_resampler_get_output_latency<F: PcmFormat>(resampler: &Resampler<F>) -> u64 {
        unsafe { sys::ma_resampler_get_output_latency(resampler.to_raw()) }
    }

    #[inline]
    pub fn ma_resampler_get_required_input_frame_count<F: PcmFormat>(
        resampler: &Resampler<F>,
        output_frame_count: u64,
    ) -> MaResult<u64> {
        let mut input_frame_count = 0;

        let res = unsafe {
            sys::ma_resampler_get_required_input_frame_count(
                resampler.to_raw(),
                output_frame_count,
                &mut input_frame_count,
            )
        };
        MaudioError::check(res)?;
        Ok(input_frame_count)
    }

    #[inline]
    pub fn ma_resampler_get_expected_output_frame_count<F: PcmFormat>(
        resampler: &Resampler<F>,
        input_frame_count: u64,
    ) -> MaResult<u64> {
        let mut output_frame_count = 0;

        let res = unsafe {
            sys::ma_resampler_get_expected_output_frame_count(
                resampler.to_raw(),
                input_frame_count,
                &mut output_frame_count,
            )
        };
        MaudioError::check(res)?;
        Ok(output_frame_count)
    }

    #[inline]
    pub fn ma_resampler_reset<F: PcmFormat>(resampler: &mut Resampler<F>) -> MaResult<()> {
        let res = unsafe { sys::ma_resampler_reset(resampler.to_raw()) };
        MaudioError::check(res)
    }
}

impl<F: PcmFormat> Drop for Resampler<F> {
    fn drop(&mut self) {
        resampler_ffi::ma_resampler_uninit(self, self.alloc_cb.clone());
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}
pub struct ResamplerBuilder {
    config: sys::ma_resampler_config,
    channels: u32,
}

impl AsRawRef for ResamplerBuilder {
    type Raw = sys::ma_resampler_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.config
    }
}

impl ResamplerBuilder {
    pub fn new(
        channels: u32,
        sample_rate_in: SampleRate,
        sample_rate_out: SampleRate,
        _algorithm: ResampleAlgorithm,
    ) -> ResamplerBuilder {
        let config = unsafe {
            sys::ma_resampler_config_init(
                Format::S16.into(),
                channels,
                sample_rate_in.into(),
                sample_rate_out.into(),
                ResampleAlgorithm::Linear.into(), // Custom will need a vtable
            )
        };
        Self { config, channels }
    }

    pub fn build_i16(&mut self) -> MaResult<Resampler<i16>> {
        self.config.format = Format::S16.into(); // for consistency sake
        Resampler::new_with_config(self, None)
    }

    pub fn build_f32(&mut self) -> MaResult<Resampler<f32>> {
        self.config.format = Format::F32.into();
        Resampler::new_with_config(self, None)
    }
}

#[cfg(test)]
mod test {
    use crate::audio::{converters::resampler::ResamplerBuilder, sample_rate::SampleRate};

    #[test]
    fn test_resampler_basic_init() {
        let _resampler = ResamplerBuilder::new(
            2,
            SampleRate::Sr44100,
            SampleRate::Sr48000,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32()
        .unwrap();
    }

    #[test]
    fn test_resampler_init_zero_too_few_frames_in() {
        let mut resampler = ResamplerBuilder::new(
            2,
            SampleRate::Sr44100,
            SampleRate::Sr48000,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32()
        .unwrap();

        let input = vec![0.5_f32; 1];
        let mut output = vec![0.0_f32; 2];

        let res = resampler.process_pcm_frames_into(&input, &mut output);

        assert!(res.is_err());
    }

    #[test]
    fn test_resampler_init_zero_too_few_frames_out() {
        let mut resampler = ResamplerBuilder::new(
            2,
            SampleRate::Sr44100,
            SampleRate::Sr48000,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32()
        .unwrap();

        let input = vec![0.5_f32; 2];
        let mut output = vec![0.0_f32; 1];

        let res = resampler.process_pcm_frames_into(&input, &mut output);

        assert!(res.is_err());
    }

    #[test]
    fn test_resampler_init_zero_ch() {
        let res = ResamplerBuilder::new(
            0,
            SampleRate::Sr44100,
            SampleRate::Sr48000,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32();
        assert!(res.is_err());
    }

    #[test]
    fn test_resampler_same_rate_preserves_constant_signal() {
        const CHANNELS: usize = 2;

        let mut resampler = ResamplerBuilder::new(
            CHANNELS as u32,
            SampleRate::Sr44100,
            SampleRate::Sr44100,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32()
        .unwrap();

        let latency_samples = resampler.output_latency() as usize * CHANNELS;

        // Must be longer than the latency region.
        let input = vec![0.25_f32; latency_samples + 64];
        let output = resampler.process_pcm_frames(&input).unwrap();

        assert!(output.len() > latency_samples);

        for &sample in &output[latency_samples..] {
            assert!((sample - 0.25).abs() < 1.0e-6);
        }
    }

    #[test]
    fn test_resampler_upsampling_frame_count() {
        const CHANNELS: usize = 2;
        const INPUT_FRAMES: usize = 441;

        let mut resampler = ResamplerBuilder::new(
            CHANNELS as u32,
            SampleRate::Sr44100,
            SampleRate::Sr48000,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32()
        .unwrap();

        let input = vec![0.5_f32; INPUT_FRAMES * CHANNELS];
        let output = resampler.process_pcm_frames(&input).unwrap();

        let output_frames = output.len() / CHANNELS;

        // 441 input frames at this ratio should correspond to about 480 frames.
        assert!(output_frames.abs_diff(480) <= 1);
        assert_eq!(output.len() % CHANNELS, 0);
    }

    #[test]
    fn test_resampler_respects_output_capacity() {
        let mut resampler = ResamplerBuilder::new(
            2,
            SampleRate::Sr44100,
            SampleRate::Sr48000,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32()
        .unwrap();

        let input = vec![0.5_f32; 100 * 2];
        let mut output = vec![0.0_f32; 10 * 2];

        let result = resampler
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert!(result.frames_produced <= 10);
        assert!(result.frames_consumed <= 100);
    }

    #[test]
    fn test_resampler_allocating_output_has_complete_frames() {
        let mut resampler = ResamplerBuilder::new(
            2,
            SampleRate::Sr44100,
            SampleRate::Sr48000,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32()
        .unwrap();

        let input = vec![0.25_f32; 441 * 2];
        let output = resampler.process_pcm_frames(&input).unwrap();

        assert_eq!(output.len() % 2, 0);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_resampler_preserves_channel_separation() {
        let mut resampler = ResamplerBuilder::new(
            2,
            SampleRate::Sr44100,
            SampleRate::Sr48000,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32()
        .unwrap();

        let mut input = Vec::with_capacity(1000 * 2);

        for _ in 0..1000 {
            input.push(0.75); // Left
            input.push(-0.25); // Right
        }

        let output = resampler.process_pcm_frames(&input).unwrap();

        // Skip the beginning in case the algorithm has startup latency.
        for frame in output.chunks_exact(2).skip(20) {
            assert!((frame[0] - 0.75).abs() < 0.01);
            assert!((frame[1] + 0.25).abs() < 0.01);
        }
    }

    #[test]
    fn test_resampler_reset_restores_initial_state() {
        let mut resampler = ResamplerBuilder::new(
            1,
            SampleRate::Sr44100,
            SampleRate::Sr48000,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32()
        .unwrap();

        let input: Vec<f32> = (0..1000).map(|i| i as f32 / 1000.0).collect();

        let first = resampler.process_pcm_frames(&input).unwrap();

        resampler.reset().unwrap();

        let second = resampler.process_pcm_frames(&input).unwrap();

        assert_eq!(first.len(), second.len());

        for (a, b) in first.iter().zip(&second) {
            assert!((a - b).abs() < 1.0e-6);
        }
    }

    #[test]
    fn test_resampler_frame_count_queries_are_consistent() {
        let resampler = ResamplerBuilder::new(
            2,
            SampleRate::Sr44100,
            SampleRate::Sr48000,
            super::ResampleAlgorithm::Linear,
        )
        .build_f32()
        .unwrap();

        let expected_output = resampler.required_output_frames(44_100).unwrap();
        assert!((expected_output as i64 - 48_000).abs() <= 2);

        let required_input = resampler.required_input_frames(48_000).unwrap();
        assert!((required_input as i64 - 44_100).abs() <= 2);
    }
}
