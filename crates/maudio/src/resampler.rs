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

pub struct Resampler<F: PcmFormat> {
    inner: *mut sys::ma_resampler,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    _format: PhantomData<F>,
}

impl<F: PcmFormat> Binding for Resampler<F> {
    type Raw = *mut sys::ma_resampler;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
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
            alloc_cb,
            _format: PhantomData,
        })
    }
}

mod resampler_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::sample_rate::SampleRate,
        engine::AllocationCallbacks,
        pcm_frames::PcmFormat,
        resampler::{Resampler, ResamplerBuilder},
        AsRawRef, Binding, MaResult, MaudioError,
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
        frames_in: *const core::ffi::c_void,
        frame_count_in: *mut u64,
        frames_out: *mut core::ffi::c_void,
        frame_count_out: *mut u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resampler_process_pcm_frames(
                resampler.to_raw(),
                frames_in,
                frame_count_in,
                frames_out,
                frame_count_out,
            )
        };
        MaudioError::check(res)
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
        input_frame_count: *mut u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resampler_get_required_input_frame_count(
                resampler.to_raw(),
                output_frame_count,
                input_frame_count,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_resampler_get_expected_output_frame_count<F: PcmFormat>(
        resampler: &Resampler<F>,
        input_frame_count: u64,
        output_frame_count: *mut u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resampler_get_expected_output_frame_count(
                resampler.to_raw(),
                input_frame_count,
                output_frame_count,
            )
        };
        MaudioError::check(res)
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
    inner: sys::ma_resampler_config,
}

impl AsRawRef for ResamplerBuilder {
    type Raw = sys::ma_resampler_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl ResamplerBuilder {
    pub fn new(
        channels: u32,
        sample_rate_in: SampleRate,
        sample_rate_out: SampleRate,
        _algorithm: ResampleAlgorithm,
    ) -> ResamplerBuilder {
        let inner = unsafe {
            sys::ma_resampler_config_init(
                Format::S16.into(),
                channels,
                sample_rate_in.into(),
                sample_rate_out.into(),
                ResampleAlgorithm::Linear.into(), // Custom will need a vtable
            )
        };
        Self { inner }
    }

    pub fn build_i16(&mut self) -> MaResult<Resampler<i16>> {
        self.inner.format = Format::S16.into(); // for consistency sake
        Resampler::new_with_config(self, None)
    }

    pub fn build_f32(&mut self) -> MaResult<Resampler<f32>> {
        self.inner.format = Format::F32.into();
        Resampler::new_with_config(self, None)
    }
}

#[cfg(test)]
mod test {
    use crate::{audio::sample_rate::SampleRate, resampler::ResamplerBuilder};

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
}
