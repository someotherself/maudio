use std::{marker::PhantomData, mem::MaybeUninit, sync::Arc};

use maudio_sys::ffi as sys;

use crate::{
    audio::formats::{Format, SampleBuffer},
    engine::AllocationCallbacks,
    pcm_frames::{PcmFormat, S24Packed, S24},
    AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
};

pub struct Noise<F: PcmFormat> {
    inner: *mut sys::ma_noise,
    channels: u32,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
    _sample_format: PhantomData<F>,
}

impl<F: PcmFormat> Binding for Noise<F> {
    type Raw = *mut sys::ma_noise;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Noise<F> {
    /// Generates PCM frames into `dst`, returning the number of frames written.
    pub fn read_pcm_frames_into(&mut self, dst: &mut [F::PcmUnit]) -> MaResult<usize> {
        noise_ffi::ma_noise_read_pcm_frames_into::<F>(self, dst)
    }

    /// Allocates and generates `frames` PCM frames.
    pub fn read_pcm_frames(&mut self, frames: u64) -> MaResult<SampleBuffer<F>> {
        noise_ffi::ma_noise_read_pcm_frames(self, frames)
    }

    /// Sets the output amplitude of the noise generator.
    ///
    /// Larger values produce louder noise. The exact practical range depends on
    /// the sample format and how the underlying miniaudio noise source is used.
    pub fn set_amplitude(&mut self, amplitude: f64) -> MaResult<()> {
        noise_ffi::ma_noise_set_amplitude(self, amplitude)
    }

    /// Sets the random seed used by the generator.
    ///
    /// A fixed seed is useful for reproducible output, especially in tests.
    /// A seed of `0` uses miniaudio's randomized default behavior.
    pub fn set_seed(&mut self, seed: i32) -> MaResult<()> {
        noise_ffi::ma_noise_set_seed(self, seed)
    }
}

mod noise_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::formats::SampleBuffer,
        data_source::sources::noise::{Noise, NoiseBuilder},
        engine::AllocationCallbacks,
        pcm_frames::{PcmFormat, PcmFormatInternal},
        AsRawRef, Binding, MaResult, MaudioError,
    };

    // Only used for custom allocators (when alloc is done by rust)
    #[inline]
    pub fn ma_noise_get_heap_size(config: &NoiseBuilder) -> MaResult<usize> {
        let mut heap_size: usize = 0;
        let res = unsafe { sys::ma_noise_get_heap_size(config.as_raw_ptr(), &mut heap_size) };
        MaudioError::check(res)?;
        Ok(heap_size)
    }

    // Only used for custom allocators (when alloc is done by rust)
    #[inline]
    pub fn ma_noise_init_preallocated(
        config: &NoiseBuilder,
        heap_alloc: *mut core::ffi::c_void,
        noise: *mut sys::ma_noise,
    ) -> MaResult<()> {
        let res =
            unsafe { sys::ma_noise_init_preallocated(config.as_raw_ptr(), heap_alloc, noise) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_noise_init(
        config: &NoiseBuilder,
        alloc: Option<Arc<AllocationCallbacks>>,
        noise: *mut sys::ma_noise,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());

        let res = unsafe { sys::ma_noise_init(config.as_raw_ptr(), alloc_cb, noise) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_noise_uninit<F: PcmFormat>(noise: &mut Noise<F>) {
        let alloc_cb: *const sys::ma_allocation_callbacks = noise
            .alloc_cb
            .clone()
            .map_or(core::ptr::null(), |c| c.as_raw_ptr());

        unsafe {
            sys::ma_noise_uninit(noise.to_raw(), alloc_cb);
        }
    }

    pub fn ma_noise_read_pcm_frames_into<F: PcmFormat>(
        noise: &mut Noise<F>,
        dst: &mut [F::PcmUnit],
    ) -> MaResult<usize> {
        let channels = noise.channels;
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        let frame_count = (dst.len() / channels as usize / F::VEC_PCM_UNITS_PER_FRAME) as u64;

        match F::DIRECT_READ {
            true => {
                let frames_read = ma_noise_read_pcm_frames_internal(
                    noise,
                    frame_count,
                    dst.as_mut_ptr() as *mut core::ffi::c_void,
                )?;
                Ok(frames_read as usize)
            }
            false => {
                let tmp_len = SampleBuffer::<F>::required_len(
                    frame_count as usize,
                    channels,
                    F::VEC_STORE_UNITS_PER_FRAME,
                )?;

                let mut tmp = vec![F::StorageUnit::default(); tmp_len];
                let frames_read = ma_noise_read_pcm_frames_internal(
                    noise,
                    frame_count,
                    tmp.as_mut_ptr() as *mut core::ffi::c_void,
                )?;

                let _ = <F as PcmFormatInternal>::read_from_storage_internal(
                    &tmp,
                    dst,
                    frames_read as usize,
                    channels as usize,
                )?;

                Ok(frames_read as usize)
            }
        }
    }

    pub fn ma_noise_read_pcm_frames<F: PcmFormat>(
        noise: &mut Noise<F>,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<F>> {
        let mut buffer = SampleBuffer::<F>::new_zeroed(frame_count as usize, noise.channels)?;

        let frames_read = ma_noise_read_pcm_frames_internal(
            noise,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<F>::from_storage(buffer, frames_read as usize, noise.channels)
    }

    #[inline]
    fn ma_noise_read_pcm_frames_internal<F: PcmFormat>(
        noise: &mut Noise<F>,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_noise_read_pcm_frames(noise.to_raw(), buffer, frame_count, &mut frames_read)
        };
        MaudioError::check(res)?;
        Ok(frames_read)
    }

    #[inline]
    pub fn ma_noise_set_amplitude<F: PcmFormat>(
        noise: &mut Noise<F>,
        amplitude: f64,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_noise_set_amplitude(noise.to_raw(), amplitude) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_noise_set_seed<F: PcmFormat>(noise: &mut Noise<F>, seed: i32) -> MaResult<()> {
        let res = unsafe { sys::ma_noise_set_seed(noise.to_raw(), seed) };
        MaudioError::check(res)
    }
}

// !!! This does not check for any extra alloc done in rust. They are not possible (yet)
impl<F: PcmFormat> Drop for Noise<F> {
    fn drop(&mut self) {
        noise_ffi::ma_noise_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

/// Procedural noise generator backed by miniaudio's `ma_noise`.
///
/// A `Noise` value generates PCM frames on demand in the sample format `F`.
/// It can be used anywhere a synthetic audio source is useful, such as:
///
/// - test signals
/// - placeholder audio
/// - procedural sound design
/// - generating white, pink, or brown noise buffers
///
/// The generator is stateful. Repeated reads continue the stream from the
/// current internal state rather than restarting from the beginning.
///
/// `Noise` owns the underlying native `ma_noise` instance and cleans it up
/// automatically on drop.
pub struct NoiseBuilder {
    inner: sys::ma_noise_config,
    channels: u32,
    noise_type: NoiseType,
    seed: i32,
    amplitude: f64,
}

impl AsRawRef for NoiseBuilder {
    type Raw = sys::ma_noise_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl NoiseBuilder {
    /// Sets the random seed for the noise generator.
    ///
    /// The default seed is `0`, which tells miniaudio to randomize the seed.
    /// Set an explicit seed when reproducible output is required, such as in
    /// tests or examples.
    ///
    /// This can also be changed after `Noise` is created
    pub fn seed(&mut self, seed: i32) -> &mut Self {
        self.inner.seed = seed;
        self.seed = seed;
        self
    }

    /// Controls whether all output channels receive the same generated signal.
    ///
    /// By default, miniaudio generates different noise for each channel.
    /// Setting this to `true` duplicates the same noise signal across channels.
    pub fn duplicate_channels(&mut self, yes: bool) -> &mut Self {
        self.inner.duplicateChannels = yes as u32;
        self
    }

    pub fn new(channels: u32, noise_type: NoiseType, amplitude: f64) -> Self {
        // Format::U8 is a placeholder
        let inner = unsafe {
            sys::ma_noise_config_init(Format::U8.into(), channels, noise_type.into(), 0, amplitude)
        };
        Self {
            inner,
            channels,
            noise_type,
            seed: 0,
            amplitude,
        }
    }

    /// Builds a noise generator that outputs `u8` PCM samples.
    pub fn build_u8(&mut self) -> MaResult<Noise<u8>> {
        self.inner.format = Format::U8.into();

        let inner = self.new_inner()?;

        Ok(Noise {
            inner,
            channels: self.channels,
            alloc_cb: None,
            _sample_format: PhantomData,
        })
    }

    /// Builds a noise generator that outputs `i16` PCM samples.
    pub fn build_i16(&mut self) -> MaResult<Noise<i16>> {
        self.inner.format = Format::S16.into();

        let inner = self.new_inner()?;

        Ok(Noise {
            inner,
            channels: self.channels,
            alloc_cb: None,
            _sample_format: PhantomData,
        })
    }

    /// Builds a noise generator that outputs `i32` PCM samples.
    pub fn build_i32(&mut self) -> MaResult<Noise<i32>> {
        self.inner.format = Format::S32.into();

        let inner = self.new_inner()?;

        Ok(Noise {
            inner,
            channels: self.channels,
            alloc_cb: None,
            _sample_format: PhantomData,
        })
    }

    /// Builds a noise generator that outputs 24-bit samples represented as i32 with extended sign.
    ///
    /// Internally, miniaudio uses packed 24-bit storage and conversion may be
    /// performed when reading into Rust-facing sample units.
    pub fn build_s24(&mut self) -> MaResult<Noise<S24>> {
        self.inner.format = Format::S24Packed.into();

        let inner = self.new_inner()?;

        Ok(Noise {
            inner,
            channels: self.channels,
            alloc_cb: None,
            _sample_format: PhantomData,
        })
    }

    /// Builds a noise generator that outputs 3-byte packed 24-bit samples via
    /// [`S24Packed`].
    pub fn build_s24_packed(&mut self) -> MaResult<Noise<S24Packed>> {
        self.inner.format = Format::S24Packed.into();

        let inner = self.new_inner()?;

        Ok(Noise {
            inner,
            channels: self.channels,
            alloc_cb: None,
            _sample_format: PhantomData,
        })
    }

    /// Builds a noise generator that outputs `f32` PCM samples.
    pub fn build_f32(&mut self) -> MaResult<Noise<f32>> {
        self.inner.format = Format::F32.into();

        let inner = self.new_inner()?;

        Ok(Noise {
            inner,
            channels: self.channels,
            alloc_cb: None,
            _sample_format: PhantomData,
        })
    }

    fn new_inner(&self) -> MaResult<*mut sys::ma_noise> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_noise>> = Box::new(MaybeUninit::uninit());

        noise_ffi::ma_noise_init(self, None, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_noise = Box::into_raw(mem) as *mut sys::ma_noise;

        Ok(inner)
    }
}

/// Type of noise generated by [`Noise`] / [`NoiseBuilder`].
///
/// - [`White`](NoiseType::White): spectrally flat random noise
/// - [`Pink`](NoiseType::Pink): energy decreases with frequency
/// - [`Brown`](NoiseType::Brown): stronger low-frequency emphasis than pink
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum NoiseType {
    White,
    Brown,
    Pink,
}

impl From<NoiseType> for sys::ma_noise_type {
    fn from(value: NoiseType) -> Self {
        match value {
            NoiseType::White => sys::ma_noise_type_ma_noise_type_white,
            NoiseType::Brown => sys::ma_noise_type_ma_noise_type_brownian,
            NoiseType::Pink => sys::ma_noise_type_ma_noise_type_pink,
        }
    }
}

impl TryFrom<sys::ma_noise_type> for NoiseType {
    type Error = MaudioError;

    fn try_from(value: sys::ma_noise_type) -> Result<Self, Self::Error> {
        match value {
            sys::ma_noise_type_ma_noise_type_white => Ok(NoiseType::White),
            sys::ma_noise_type_ma_noise_type_brownian => Ok(NoiseType::Brown),
            sys::ma_noise_type_ma_noise_type_pink => Ok(NoiseType::Pink),
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                NoiseType,
            >(other as i64))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::formats::SampleBuffer;

    fn assert_nonzero_frames<F: PcmFormat>(result: MaResult<SampleBuffer<F>>) {
        let buf = result.expect("expected noise read to succeed");
        assert!(buf.frames() > 0);
    }

    #[test]
    fn test_noise_type_into_raw_white() {
        let raw: sys::ma_noise_type = NoiseType::White.into();
        assert_eq!(raw, sys::ma_noise_type_ma_noise_type_white);
    }

    #[test]
    fn test_noise_type_into_raw_brown() {
        let raw: sys::ma_noise_type = NoiseType::Brown.into();
        assert_eq!(raw, sys::ma_noise_type_ma_noise_type_brownian);
    }

    #[test]
    fn test_noise_type_into_raw_pink() {
        let raw: sys::ma_noise_type = NoiseType::Pink.into();
        assert_eq!(raw, sys::ma_noise_type_ma_noise_type_pink);
    }

    #[test]
    fn test_noise_type_try_from_raw_white() {
        let ty = NoiseType::try_from(sys::ma_noise_type_ma_noise_type_white).unwrap();
        assert_eq!(ty, NoiseType::White);
    }

    #[test]
    fn test_noise_type_try_from_raw_brown() {
        let ty = NoiseType::try_from(sys::ma_noise_type_ma_noise_type_brownian).unwrap();
        assert_eq!(ty, NoiseType::Brown);
    }

    #[test]
    fn test_noise_type_try_from_raw_pink() {
        let ty = NoiseType::try_from(sys::ma_noise_type_ma_noise_type_pink).unwrap();
        assert_eq!(ty, NoiseType::Pink);
    }

    #[test]
    fn test_noise_type_try_from_raw_invalid() {
        let invalid = -12345i32 as sys::ma_noise_type;
        let err = NoiseType::try_from(invalid).unwrap_err();
        let _ = err;
    }

    #[test]
    fn test_noise_builder_new_stores_basic_config() {
        let builder = NoiseBuilder::new(2, NoiseType::White, 0.25);

        assert_eq!(builder.channels, 2);
        assert_eq!(builder.noise_type, NoiseType::White);
        assert_eq!(builder.seed, 0);
        assert_eq!(builder.amplitude, 0.25);
    }

    #[test]
    fn test_noise_builder_seed_sets_seed() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.25);
        let returned = builder.seed(1234) as *mut _;
        let builder_ptr = &mut builder as *mut _;

        assert_eq!(returned, builder_ptr);
        assert_eq!(builder.seed, 1234);
        assert_eq!(builder.inner.seed, 1234);
    }

    #[test]
    fn test_noise_builder_duplicate_channels_true() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.25);
        let returned = builder.duplicate_channels(true) as *mut _;
        let builder_ptr = &mut builder as *mut _;

        assert_eq!(returned, builder_ptr);
        assert_eq!(builder.inner.duplicateChannels, 1);
    }

    #[test]
    fn test_noise_builder_duplicate_channels_false() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.25);
        builder.duplicate_channels(false);

        assert_eq!(builder.inner.duplicateChannels, 0);
    }

    #[test]
    fn test_noise_build_u8_and_read() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_u8().expect("build_u8 should succeed");

        assert_eq!(noise.channels, 2);
        assert_nonzero_frames(noise.read_pcm_frames(64));
    }

    #[test]
    fn test_noise_build_i16_and_read() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_i16().expect("build_i16 should succeed");

        assert_eq!(noise.channels, 2);
        assert_nonzero_frames(noise.read_pcm_frames(64));
    }

    #[test]
    fn test_noise_build_i32_and_read() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_i32().expect("build_i32 should succeed");

        assert_eq!(noise.channels, 2);
        assert_nonzero_frames(noise.read_pcm_frames(64));
    }

    #[test]
    fn test_noise_build_s24_and_read() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_s24().expect("build_s24 should succeed");

        assert_eq!(noise.channels, 2);
        assert_nonzero_frames(noise.read_pcm_frames(64));
    }

    #[test]
    fn test_noise_build_s24_packed_and_read() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder
            .build_s24_packed()
            .expect("build_s24_packed should succeed");

        assert_eq!(noise.channels, 2);
        assert_nonzero_frames(noise.read_pcm_frames(64));
    }

    #[test]
    fn test_noise_build_f32_and_read() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_f32().expect("build_f32 should succeed");

        assert_eq!(noise.channels, 2);
        assert_nonzero_frames(noise.read_pcm_frames(64));
    }

    #[test]
    fn test_noise_read_pcm_frames_into_u8() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_u8().unwrap();

        let mut dst = vec![0u8; 64 * 2];
        let frames = noise
            .read_pcm_frames_into(&mut dst)
            .expect("read_pcm_frames_into should succeed");

        assert_eq!(frames, 64);
    }

    #[test]
    fn test_noise_read_pcm_frames_into_i16() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_i16().unwrap();

        let mut dst = vec![0i16; 64 * 2];
        let frames = noise
            .read_pcm_frames_into(&mut dst)
            .expect("read_pcm_frames_into should succeed");

        assert_eq!(frames, 64);
    }

    #[test]
    fn test_noise_read_pcm_frames_into_i32() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_i32().unwrap();

        let mut dst = vec![0i32; 64 * 2];
        let frames = noise
            .read_pcm_frames_into(&mut dst)
            .expect("read_pcm_frames_into should succeed");

        assert_eq!(frames, 64);
    }

    #[test]
    fn test_noise_read_pcm_frames_into_s24() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_s24().unwrap();

        let mut dst = vec![i32::default(); 64 * 2];
        let frames = noise
            .read_pcm_frames_into(&mut dst)
            .expect("read_pcm_frames_into should succeed");

        assert_eq!(frames, 64);
    }

    #[test]
    fn test_noise_read_pcm_frames_into_s24_packed() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_s24_packed().unwrap();

        let mut dst = vec![S24Packed::SILENCE; 64 * 3 * 2];
        let frames = noise
            .read_pcm_frames_into(&mut dst)
            .expect("read_pcm_frames_into should succeed");

        assert_eq!(frames, 64);
    }

    #[test]
    fn test_noise_read_pcm_frames_into_f32() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_f32().unwrap();

        let mut dst = vec![0.0f32; 64 * 2];
        let frames = noise
            .read_pcm_frames_into(&mut dst)
            .expect("read_pcm_frames_into should succeed");

        assert_eq!(frames, 64);
    }

    #[test]
    fn test_noise_read_pcm_frames_zero_frames() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_f32().unwrap();

        let buf = noise
            .read_pcm_frames(0);
        assert!(buf.is_err());
    }

    #[test]
    fn test_noise_read_pcm_frames_into_empty_slice_returns_error() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_i16().unwrap();

        let mut dst: Vec<i16> = Vec::new();

        let err = noise
            .read_pcm_frames_into(&mut dst)
            .expect_err("empty slice should be invalid");

        let _ = err; // optional: assert exact error kind
    }

    #[test]
    fn test_noise_set_amplitude_after_build() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.1);
        let mut noise = builder.build_f32().unwrap();

        noise
            .set_amplitude(0.75)
            .expect("set_amplitude should succeed");

        let mut dst = vec![0.0f32; 32 * 2];
        let frames = noise.read_pcm_frames_into(&mut dst).unwrap();
        assert_eq!(frames, 32);
    }

    #[test]
    fn test_noise_set_seed_after_build() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.1);
        let mut noise = builder.build_f32().unwrap();

        noise.set_seed(42).expect("set_seed should succeed");

        let mut dst = vec![0.0f32; 32 * 2];
        let frames = noise.read_pcm_frames_into(&mut dst).unwrap();
        assert_eq!(frames, 32);
    }

    #[test]
    fn test_noise_seed_reproducible_for_f32() {
        let mut builder_a = NoiseBuilder::new(2, NoiseType::White, 0.5);
        builder_a.seed(12345);
        let mut noise_a = builder_a.build_f32().unwrap();

        let mut builder_b = NoiseBuilder::new(2, NoiseType::White, 0.5);
        builder_b.seed(12345);
        let mut noise_b = builder_b.build_f32().unwrap();

        let a = noise_a.read_pcm_frames(128).unwrap();
        let b = noise_b.read_pcm_frames(128).unwrap();

        assert_eq!(a.as_ref(), b.as_ref());
    }

    #[test]
    fn test_noise_duplicate_channels_true_produces_equal_channels_for_f32() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        builder.seed(999).duplicate_channels(true);
        let mut noise = builder.build_f32().unwrap();

        let buf = noise.read_pcm_frames(128).unwrap();

        for frame in buf.as_ref().chunks_exact(2) {
            assert_eq!(frame[0], frame[1]);
        }
    }

    #[test]
    fn test_noise_white_build_read_drop() {
        let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
        let mut noise = builder.build_i16().unwrap();

        let _ = noise.read_pcm_frames(256).unwrap();
    }

    #[test]
    fn test_noise_pink_build_read_drop() {
        let mut builder = NoiseBuilder::new(2, NoiseType::Pink, 0.5);
        let mut noise = builder.build_i16().unwrap();

        let _ = noise.read_pcm_frames(256).unwrap();
    }

    #[test]
    fn test_noise_brown_build_read_drop() {
        let mut builder = NoiseBuilder::new(2, NoiseType::Brown, 0.5);
        let mut noise = builder.build_i16().unwrap();

        let _ = noise.read_pcm_frames(256).unwrap();
    }

    #[test]
    fn test_noise_repeated_create_read_drop_loop() {
        for _ in 0..100 {
            let mut builder = NoiseBuilder::new(2, NoiseType::White, 0.5);
            builder.seed(7);

            let mut noise = builder.build_f32().unwrap();
            let _ = noise.read_pcm_frames(128).unwrap();

            let mut dst = vec![0.0f32; 128 * 2];
            let _ = noise.read_pcm_frames_into(&mut dst).unwrap();

            noise.set_amplitude(0.25).unwrap();
            noise.set_seed(99).unwrap();

            let _ = noise.read_pcm_frames(32).unwrap();
        }
    }
}
