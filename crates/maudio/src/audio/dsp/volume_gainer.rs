use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{pcm_frames::PcmFormat, Binding, MaResult};

pub struct Gainer<F: PcmFormat> {
    inner: *mut sys::ma_gainer,
    channels: u32,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for Gainer<F> {}

impl<F: PcmFormat> Binding for Gainer<F> {
    type Raw = *mut sys::ma_gainer;

    /// !!! unimplemented !!!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Gainer<F> {
    fn build(config: &sys::ma_gainer_config) -> MaResult<Gainer<F>> {
        let channels = config.channels;
        let mut inner: MaybeUninit<sys::ma_gainer> = MaybeUninit::uninit();
        gainer_ffi::ma_gainer_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));
        Ok(Gainer {
            inner: inner_ptr,
            channels,
            _format: PhantomData,
        })
    }

    pub fn process_pcm_frames(
        &mut self,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        gainer_ffi::ma_gainer_process_pcm_frames(self, frames_out, frames_in)
    }

    pub fn set_gain(&mut self, gain: f32) -> MaResult<()> {
        gainer_ffi::ma_gainer_set_gain(self, gain)
    }

    pub fn set_gains(&mut self, gains: &[f32]) -> MaResult<()> {
        gainer_ffi::ma_gainer_set_gains(self, gains)
    }

    pub fn get_master_volume(&self) -> MaResult<f32> {
        gainer_ffi::ma_gainer_get_master_volume(self)
    }

    pub fn set_master_volume(&mut self, volume: f32) -> MaResult<()> {
        gainer_ffi::ma_gainer_set_master_volume(self, volume)
    }
}

pub struct GainerBuilder {
    config: sys::ma_gainer_config,
}

impl GainerBuilder {
    pub fn new(channels: u32, smooth_time_frames: u32) -> Self {
        let config = unsafe { sys::ma_gainer_config_init(channels, smooth_time_frames) };
        Self { config }
    }

    pub fn build_f32(&self) -> MaResult<Gainer<f32>> {
        Gainer::build(&self.config)
    }
}

mod gainer_ffi {
    use std::sync::Arc;

    use crate::{
        audio::dsp::volume_gainer::Gainer, engine::AllocationCallbacks, pcm_frames::PcmFormat,
        AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
    };
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_gainer_init(
        config: &sys::ma_gainer_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        gainer: *mut sys::ma_gainer,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_gainer_init(config as *const _, alloc_cb, gainer) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_gainer_uninit<F: PcmFormat>(gainer: &mut Gainer<F>) {
        unsafe {
            sys::ma_gainer_uninit(gainer.to_raw(), std::ptr::null_mut());
        };
    }

    #[inline]
    pub fn ma_gainer_process_pcm_frames<F: PcmFormat>(
        gainer: &mut Gainer<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels = gainer.channels as usize;

        let frame_in = frames_in.len() / channels;
        let frame_out = frames_out.len() / channels;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_gainer_process_pcm_frames(
                gainer.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_gainer_set_gain<F: PcmFormat>(gainer: &mut Gainer<F>, gain: f32) -> MaResult<()> {
        let res = unsafe { sys::ma_gainer_set_gain(gainer.to_raw(), gain) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_gainer_set_gains<F: PcmFormat>(
        gainer: &mut Gainer<F>,
        gains: &[f32],
    ) -> MaResult<()> {
        if gains.len() != gainer.channels as usize {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Number of gains equal the number of channels.",
            )));
        }
        let res = unsafe { sys::ma_gainer_set_gains(gainer.to_raw(), gains.as_ptr() as *mut _) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_gainer_set_master_volume<F: PcmFormat>(
        gainer: &mut Gainer<F>,
        volume: f32,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_gainer_set_master_volume(gainer.to_raw(), volume) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_gainer_get_master_volume<F: PcmFormat>(gainer: &Gainer<F>) -> MaResult<f32> {
        let mut volume = 0.0;
        let res = unsafe { sys::ma_gainer_get_master_volume(gainer.to_raw(), &mut volume) };
        MaudioError::check(res)?;
        Ok(volume)
    }
}

impl<F: PcmFormat> Drop for Gainer<F> {
    fn drop(&mut self) {
        gainer_ffi::ma_gainer_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn assert_f32_slice_close(actual: &[f32], expected: &[f32]) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "slice lengths differ: actual={}, expected={}",
            actual.len(),
            expected.len()
        );

        for (i, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (*actual - *expected).abs() < 0.00001,
                "sample {i} differs: actual={actual}, expected={expected}"
            );
        }
    }

    #[test]
    fn gainer_test_build_f32_creates_valid_gainer() {
        let gainer = GainerBuilder::new(2, 0).build_f32().unwrap();

        assert!(!gainer.to_raw().is_null());
        assert_eq!(gainer.channels, 2);
    }

    #[test]
    fn gainer_test_master_volume_round_trip() {
        let mut gainer = GainerBuilder::new(2, 0).build_f32().unwrap();

        gainer.set_master_volume(0.75).unwrap();

        let volume = gainer.get_master_volume().unwrap();
        assert!((volume - 0.75).abs() < 0.00001);
    }

    #[test]
    fn gainer_test_set_gain_applies_same_gain_to_all_channels() {
        let mut gainer = GainerBuilder::new(2, 0).build_f32().unwrap();

        gainer.set_gain(2.0).unwrap();

        let frames_in = [
            0.25, -0.5, //
            1.0, -1.0, //
        ];
        let mut frames_out = [0.0; 4];

        gainer
            .process_pcm_frames(&mut frames_out, &frames_in)
            .unwrap();

        assert_f32_slice_close(
            &frames_out,
            &[
                0.5, -1.0, //
                2.0, -2.0, //
            ],
        );
    }

    #[test]
    fn gainer_test_set_gains_applies_gain_per_channel() {
        let mut gainer = GainerBuilder::new(2, 0).build_f32().unwrap();

        gainer.set_gains(&[2.0, 0.5]).unwrap();

        let frames_in = [
            1.0, 1.0, //
            -2.0, -2.0, //
        ];
        let mut frames_out = [0.0; 4];

        gainer
            .process_pcm_frames(&mut frames_out, &frames_in)
            .unwrap();

        assert_f32_slice_close(
            &frames_out,
            &[
                2.0, 0.5, //
                -4.0, -1.0, //
            ],
        );
    }

    #[test]
    fn set_gains_requires_one_gain_per_channel() {
        let mut gainer = GainerBuilder::new(2, 0).build_f32().unwrap();

        assert!(gainer.set_gains(&[]).is_err());
        assert!(gainer.set_gains(&[1.0]).is_err());
        assert!(gainer.set_gains(&[1.0, 1.0]).is_ok());
        assert!(gainer.set_gains(&[1.0, 1.0, 1.0]).is_err());
    }

    #[test]
    fn gainer_test_process_pcm_frames_accepts_in_place_processing() {
        let mut gainer = GainerBuilder::new(2, 0).build_f32().unwrap();

        gainer.set_gains(&[2.0, 0.5]).unwrap();

        let mut frames = [
            1.0, 1.0, //
            2.0, 2.0, //
        ];

        let input = frames;

        gainer.process_pcm_frames(&mut frames, &input).unwrap();

        assert_f32_slice_close(
            &frames,
            &[
                2.0, 0.5, //
                4.0, 1.0, //
            ],
        );
    }
}
