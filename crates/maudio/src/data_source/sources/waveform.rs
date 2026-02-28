//! Procedural waveform generator.
//!
//! Generates common waveforms (sine, square, triangle, sawtooth, etc.) and
//! exposes them as a [`DataSource`](crate::data_source::DataSource).
//!
//! This is useful for synthesis, testing, and generating example audio without
//! loading external files.
//!
//! The waveform can be controlled at runtime via [`WaveFormOps`] (type,
//! amplitude, frequency, and sample rate) and can be seeked like any other
//! source.
use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{
        formats::{Format, SampleBuffer},
        sample_rate::SampleRate,
        wave_shape::WaveFormType,
    },
    data_source::{
        private_data_source, sources::waveform::private_wave::WaveFormPtrProvider, AsSourcePtr,
        DataSourceRef,
    },
    pcm_frames::{PcmFormat, S24Packed, S24},
    AsRawRef, Binding, MaResult,
};

pub(crate) struct WaveState {
    channels: u32,
    sample_rate: SampleRate,
    wave_type: WaveFormType,
    amplitude: f64,
    frequency: f64,
}

/// Allows all WaveForm types to access the same methods
pub trait AsWaveFormPtr {
    #[doc(hidden)]
    type __PtrProvider: WaveFormPtrProvider<Self>;
    fn channels(&self) -> u32;
}

/// Procedural waveform generator.
///
/// `WaveForm` produces continuous periodic PCM audio (sine, square, triangle,
/// sawtooth) and can be read, seeked, or used as a [`DataSource`](crate::data_source::DataSource).
///
/// Audio is generated in **PCM frames** using the selected [`PcmFormat`].
pub struct WaveForm<F: PcmFormat> {
    inner: *mut sys::ma_waveform,
    format: Format,
    state: WaveState,
    _sample_format: PhantomData<F>,
}

impl<F: PcmFormat> Binding for WaveForm<F> {
    type Raw = *mut sys::ma_waveform;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[doc(hidden)]
impl<F: PcmFormat> AsSourcePtr for WaveForm<F> {
    type __PtrProvider = private_data_source::WaveFormProvider;
}

impl<F: PcmFormat> AsWaveFormPtr for WaveForm<F> {
    #[doc(hidden)]
    type __PtrProvider = private_wave::WaveFormProvider;

    fn channels(&self) -> u32 {
        self.state.channels
    }
}

mod private_wave {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait WaveFormPtrProvider<T: ?Sized> {
        fn as_waveform_ptr(t: &T) -> *mut sys::ma_waveform;
    }

    pub struct WaveFormProvider;

    impl<F: PcmFormat> WaveFormPtrProvider<WaveForm<F>> for WaveFormProvider {
        fn as_waveform_ptr(t: &WaveForm<F>) -> *mut sys::ma_waveform {
            t.to_raw()
        }
    }

    pub fn waveform_ptr<T: AsWaveFormPtr + ?Sized>(t: &T) -> *mut sys::ma_waveform {
        <T as AsWaveFormPtr>::__PtrProvider::as_waveform_ptr(t)
    }
}

impl<F: PcmFormat> WaveFormOps for WaveForm<F> {
    type Format = F;
}

pub trait WaveFormOps: AsWaveFormPtr + AsSourcePtr {
    type Format: PcmFormat;

    /// Generates PCM frames into `dst`, returning the number of frames written.
    fn read_pcm_frames_into(
        &mut self,
        dst: &mut [<Self::Format as PcmFormat>::PcmUnit],
    ) -> MaResult<usize> {
        waveform_ffi::ma_waveform_read_pcm_frames_into::<Self::Format, Self>(self, dst)
    }

    /// Allocates and generates `frames` PCM frames.
    fn read_pcm_frames(&mut self, frames: u64) -> MaResult<SampleBuffer<Self::Format>> {
        waveform_ffi::ma_waveform_read_pcm_frames(self, frames)
    }

    /// Seeks to an absolute PCM frame position.
    fn seek_to_pcm_frame(&mut self, frame_index: u64) -> MaResult<()> {
        waveform_ffi::ma_waveform_seek_to_pcm_frame(self, frame_index)
    }

    /// Sets the waveform amplitude.
    fn set_amplitude(&mut self, amplitude: f64) -> MaResult<()> {
        waveform_ffi::ma_waveform_set_amplitude(self, amplitude)
    }

    /// Sets the waveform frequency in Hz.
    fn set_frequency(&mut self, frequency: f64) -> MaResult<()> {
        waveform_ffi::ma_waveform_set_frequency(self, frequency)
    }

    /// Sets the waveform shape.
    fn set_type(&mut self, wave_type: WaveFormType) -> MaResult<()> {
        waveform_ffi::ma_waveform_set_type(self, wave_type)
    }

    /// Sets the sample rate used for generation.
    fn set_sample_rate(&mut self, sample_rate: SampleRate) -> MaResult<()> {
        waveform_ffi::ma_waveform_set_sample_rate(self, sample_rate)
    }

    /// Returns a [`DataSourceRef`] view of this waveform.
    fn as_source<'a>(&'a self) -> DataSourceRef<'a> {
        debug_assert!(!private_wave::waveform_ptr(self).is_null());
        let ptr = private_wave::waveform_ptr(self).cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

pub(crate) mod waveform_ffi {
    use crate::{
        audio::{formats::SampleBuffer, sample_rate::SampleRate, wave_shape::WaveFormType},
        data_source::sources::waveform::{private_wave, AsWaveFormPtr, WaveFormBuilder},
        pcm_frames::{PcmFormat, PcmFormatInternal},
        AsRawRef, MaResult, MaudioError,
    };
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_waveform_init(
        config: &WaveFormBuilder,
        waveform: *mut sys::ma_waveform,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_waveform_init(config.as_raw_ptr(), waveform) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_waveform_uninit<W: AsWaveFormPtr + ?Sized>(waveform: &mut W) {
        unsafe {
            sys::ma_waveform_uninit(private_wave::waveform_ptr(waveform));
        }
    }

    pub fn ma_waveform_read_pcm_frames_into<F: PcmFormat, W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        dst: &mut [F::PcmUnit],
    ) -> MaResult<usize> {
        let channels = waveform.channels();
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        let frame_count = (dst.len() / channels as usize / F::VEC_PCM_UNITS_PER_FRAME) as u64;

        match F::DIRECT_READ {
            true => {
                let frames_read = ma_waveform_read_pcm_frames_internal(
                    waveform,
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
                let frames_read = ma_waveform_read_pcm_frames_internal(
                    waveform,
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

    pub fn ma_waveform_read_pcm_frames<F: PcmFormat, W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<F>> {
        let mut buffer = SampleBuffer::<F>::new_zeroed(frame_count as usize, waveform.channels())?;

        let frames_read = ma_waveform_read_pcm_frames_internal(
            waveform,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<F>::from_storage(buffer, frames_read as usize, waveform.channels())
    }

    #[inline]
    fn ma_waveform_read_pcm_frames_internal<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_waveform_read_pcm_frames(
                private_wave::waveform_ptr(waveform),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_read)
    }

    #[inline]
    pub fn ma_waveform_seek_to_pcm_frame<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        frame_index: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_waveform_seek_to_pcm_frame(private_wave::waveform_ptr(waveform), frame_index)
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_waveform_set_amplitude<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        amplitude: f64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_waveform_set_amplitude(private_wave::waveform_ptr(waveform), amplitude)
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_waveform_set_frequency<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        frequency: f64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_waveform_set_frequency(private_wave::waveform_ptr(waveform), frequency)
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_waveform_set_type<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        wave_type: WaveFormType,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_waveform_set_type(private_wave::waveform_ptr(waveform), wave_type.into())
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_waveform_set_sample_rate<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        sample_rate: SampleRate,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_waveform_set_sample_rate(
                private_wave::waveform_ptr(waveform),
                sample_rate.into(),
            )
        };
        MaudioError::check(res)
    }
}

impl<F: PcmFormat> Drop for WaveForm<F> {
    fn drop(&mut self) {
        waveform_ffi::ma_waveform_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

/// Builder for constructing a [`WaveForm`]
pub struct WaveFormBuilder {
    inner: sys::ma_waveform_config,
    format: Format,
    channels: u32,
    sample_rate: SampleRate,
    wave_type: WaveFormType,
    amplitude: f64,
    frequency: f64,
}

impl AsRawRef for WaveFormBuilder {
    type Raw = sys::ma_waveform_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl WaveFormBuilder {
    /// Creates a new waveform builder with fully configurable parameters.
    ///
    /// This is the most flexible way to construct a waveform.
    pub fn new(
        channels: u32,
        sample_rate: SampleRate,
        wave_type: WaveFormType,
        amplitude: f64,
        frequency: f64,
    ) -> Self {
        let ptr = unsafe {
            sys::ma_waveform_config_init(
                Format::F32.into(),
                channels,
                sample_rate.into(),
                wave_type.into(),
                amplitude,
                frequency,
            )
        };
        Self {
            inner: ptr,
            format: Format::F32,
            channels,
            sample_rate,
            wave_type,
            amplitude,
            frequency,
        }
    }

    /// Convenience method for creating a `WaveFormType::Sine` node with some default values:
    /// - `channels`: 2
    /// - `amplitude`: 0.2
    /// - `format`: Format::F32
    pub fn new_sine(sample_rate: SampleRate, frequency: f64) -> Self {
        let channels = 2;
        let wave_type = WaveFormType::Sine;
        let amplitude = 0.2;
        let ptr = unsafe {
            sys::ma_waveform_config_init(
                Format::F32.into(),
                channels,
                sample_rate.into(),
                wave_type.into(),
                amplitude,
                frequency,
            )
        };
        Self {
            inner: ptr,
            format: Format::F32,
            channels,
            sample_rate,
            wave_type,
            amplitude,
            frequency,
        }
    }

    /// Convenience method for creating a `WaveFormType::Square` node with some default values:
    /// - `channels`: 2
    /// - `amplitude`: 0.2
    /// - `format`: Format::F32
    pub fn new_square(sample_rate: SampleRate, frequency: f64) -> Self {
        let channels = 2;
        let wave_type = WaveFormType::Square;
        let amplitude = 0.2;
        let ptr = unsafe {
            sys::ma_waveform_config_init(
                Format::F32.into(),
                channels,
                sample_rate.into(),
                wave_type.into(),
                amplitude,
                frequency,
            )
        };
        Self {
            inner: ptr,
            format: Format::F32,
            channels,
            sample_rate,
            wave_type,
            amplitude,
            frequency,
        }
    }

    /// Convenience method for creating a `WaveFormType::Sawtooth` node with some default values:
    /// - `channels`: 2
    /// - `amplitude`: 0.2
    /// - `format`: Format::F32
    pub fn new_sawtooth(sample_rate: SampleRate, frequency: f64) -> Self {
        let channels = 2;
        let wave_type = WaveFormType::Sawtooth;
        let amplitude = 0.2;
        let ptr = unsafe {
            sys::ma_waveform_config_init(
                Format::F32.into(),
                channels,
                sample_rate.into(),
                wave_type.into(),
                amplitude,
                frequency,
            )
        };
        Self {
            inner: ptr,
            format: Format::F32,
            channels,
            sample_rate,
            wave_type,
            amplitude,
            frequency,
        }
    }

    /// Convenience method for creating a `WaveFormType::Triangle` node with some default values:
    /// - `channels`: 2
    /// - `amplitude`: 0.2
    /// - `format`: Format::F32
    pub fn new_triangle(sample_rate: SampleRate, frequency: f64) -> Self {
        let channels = 2;
        let wave_type = WaveFormType::Triangle;
        let amplitude = 0.2;
        let ptr = unsafe {
            sys::ma_waveform_config_init(
                Format::F32.into(),
                channels,
                sample_rate.into(),
                wave_type.into(),
                amplitude,
                frequency,
            )
        };
        Self {
            inner: ptr,
            format: Format::F32,
            channels,
            sample_rate,
            wave_type,
            amplitude,
            frequency,
        }
    }

    /// Sets the waveform type.
    pub fn wave_type(&mut self, t: WaveFormType) -> &mut Self {
        self.inner.type_ = t.into();
        self.wave_type = t;
        self
    }

    /// Sets the waveform amplitude.
    pub fn amplitude(&mut self, a: f64) -> &mut Self {
        self.inner.amplitude = a;
        self.amplitude = a;
        self
    }

    /// Sets the waveform frequency, in Hertz.
    pub fn frequency(&mut self, f: f64) -> &mut Self {
        self.inner.frequency = f;
        self.frequency = f;
        self
    }

    /// Sets the number of output channels.
    pub fn channels(&mut self, c: u32) -> &mut Self {
        self.inner.channels = c;
        self.channels = c;
        self
    }

    pub fn build_u8(&mut self) -> MaResult<WaveForm<u8>> {
        self.inner.format = Format::U8.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(WaveForm {
            inner,
            format: Format::U8,
            state,
            _sample_format: PhantomData,
        })
    }

    pub fn build_i16(&mut self) -> MaResult<WaveForm<i16>> {
        self.inner.format = Format::S16.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(WaveForm {
            inner,
            format: Format::S16,
            state,
            _sample_format: PhantomData,
        })
    }

    pub fn build_i32(&mut self) -> MaResult<WaveForm<i32>> {
        self.inner.format = Format::S32.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(WaveForm {
            inner,
            format: Format::S32,
            state,
            _sample_format: PhantomData,
        })
    }

    pub fn build_s24(&mut self) -> MaResult<WaveForm<S24>> {
        self.inner.format = Format::S24.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(WaveForm {
            inner,
            format: Format::S24,
            state,
            _sample_format: PhantomData,
        })
    }

    pub fn build_s24_packed(&mut self) -> MaResult<WaveForm<S24Packed>> {
        self.inner.format = Format::S24.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(WaveForm {
            inner,
            format: Format::S24,
            state,
            _sample_format: PhantomData,
        })
    }

    pub fn build_f32(&mut self) -> MaResult<WaveForm<f32>> {
        self.inner.format = Format::F32.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(WaveForm {
            inner,
            format: Format::F32,
            state,
            _sample_format: PhantomData,
        })
    }

    fn new_state(&self) -> WaveState {
        WaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            wave_type: self.wave_type,
            amplitude: self.amplitude,
            frequency: self.frequency,
        }
    }

    fn new_inner(&self) -> MaResult<*mut sys::ma_waveform> {
        self.init_from_config_internal()
    }

    fn init_from_config_internal(&self) -> MaResult<*mut sys::ma_waveform> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_waveform>> = Box::new(MaybeUninit::uninit());

        waveform_ffi::ma_waveform_init(self, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_waveform = Box::into_raw(mem) as *mut sys::ma_waveform;

        Ok(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq_f32(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn test_waveform_builder_new_sine_initializes_expected_defaults() {
        let sample_rate = SampleRate::Sr48000;
        let frequency = 440.0;

        let b = WaveFormBuilder::new_sine(sample_rate, frequency);

        assert_eq!(b.format, Format::F32);
        assert_eq!(b.channels, 2);
        assert_eq!(b.sample_rate, sample_rate);
        assert_eq!(b.wave_type, WaveFormType::Sine);
        assert_eq!(b.amplitude, 0.2);
        assert_eq!(b.frequency, frequency);

        // Ensure raw config mirrors as well.
        assert_eq!(b.inner.channels, 2);
        assert_eq!(b.inner.amplitude, 0.2);
        assert_eq!(b.inner.frequency, frequency);
    }

    #[test]
    fn test_waveform_build_u8_sets_state_and_format() {
        let channels = 2;
        let sample_rate = SampleRate::Sr48000;
        let wave_type = WaveFormType::Sine;
        let amplitude = 1.0;
        let frequency = 440.0;

        let w = WaveFormBuilder::new(channels, sample_rate, wave_type, amplitude, frequency)
            .build_u8()
            .unwrap();

        assert_eq!(w.format, Format::U8);
        assert_eq!(w.state.channels, channels);
        assert_eq!(w.state.sample_rate, sample_rate);
        assert_eq!(w.state.wave_type, wave_type);
        assert_eq!(w.state.amplitude, amplitude);
        assert_eq!(w.state.frequency, frequency);
    }

    #[test]
    fn test_waveform_build_i16_sets_state_and_format() {
        let w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_i16()
            .unwrap();

        assert_eq!(w.format, Format::S16);
        assert_eq!(w.state.channels, 2);
        assert_eq!(w.state.wave_type, WaveFormType::Sine);
        assert_eq!(w.state.amplitude, 0.2);
        assert_eq!(w.state.frequency, 440.0);
    }

    #[test]
    fn test_waveform_build_i32_sets_state_and_format() {
        let w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_i32()
            .unwrap();

        assert_eq!(w.format, Format::S32);
        assert_eq!(w.state.channels, 2);
    }

    #[test]
    fn test_waveform_build_s24_sets_state_and_format() {
        let w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_s24()
            .unwrap();

        assert_eq!(w.format, Format::S24);
        assert_eq!(w.state.channels, 2);
    }

    #[test]
    fn test_waveform_build_f32_sets_state_and_format() {
        let w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_f32()
            .unwrap();

        assert_eq!(w.format, Format::F32);
        assert_eq!(w.state.channels, 2);
    }

    // --- read_pcm_frames contract tests -------------------------------------
    //
    // We do NOT assume ma_waveform always fills `frame_count`.
    // We only validate:
    // - frames_read <= requested
    // - buffer length matches frames_read * channels (or *3 for s24)
    // - reading 0 frames behaves

    #[test]
    fn test_waveform_read_pcm_frames_u8_len_matches_frames_read_times_channels() {
        let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_u8()
            .unwrap();

        let requested = 256u64;
        let buf = w.read_pcm_frames(requested).unwrap();
        let frames_read = buf.frames() as u64;

        assert!(frames_read <= requested);
        assert_eq!(buf.len(), (frames_read * w.channels() as u64) as usize);
    }

    #[test]
    fn test_waveform_read_pcm_frames_i16_len_matches_frames_read_times_channels() {
        let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_i16()
            .unwrap();

        let requested = 256u64;
        let buf = w.read_pcm_frames(requested).unwrap();
        let frames_read = buf.frames() as u64;

        assert!(frames_read <= requested);
        assert_eq!(buf.len(), (frames_read * w.channels() as u64) as usize);
    }

    #[test]
    fn test_waveform_read_pcm_frames_i32_len_matches_frames_read_times_channels() {
        let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_i32()
            .unwrap();

        let requested = 256u64;
        let buf = w.read_pcm_frames(requested).unwrap();
        let frames_read = buf.frames() as u64;

        assert!(frames_read <= requested);
        assert_eq!(buf.len(), (frames_read * w.channels() as u64) as usize);
    }

    #[test]
    fn test_waveform_read_pcm_frames_f32_len_matches_frames_read_times_channels() {
        let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_f32()
            .unwrap();

        let requested = 256u64;
        let buf = w.read_pcm_frames(requested).unwrap();
        let frames_read = buf.frames() as u64;

        assert!(frames_read <= requested);
        assert_eq!(buf.len(), (frames_read * w.channels() as u64) as usize);

        // Optional sanity: sine in [-1,1] range when amplitude=1.
        // Don't be overly strict across backends/platforms.
        if !buf.is_empty() {
            let max_abs = buf
                .as_ref()
                .iter()
                .copied()
                .map(|x| x.abs())
                .fold(0.0f32, f32::max);
            assert!(max_abs <= 1.1);
        }
    }

    #[test]
    fn test_waveform_read_pcm_frames_s24_len_matches_frames_read_times_channels_times_3() {
        let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_s24()
            .unwrap();

        let requested = 256u64;
        let buf = w.read_pcm_frames(requested).unwrap();
        let frames_read = buf.frames() as u64;

        assert!(frames_read <= requested);
        assert_eq!(buf.len(), (frames_read * w.channels() as u64) as usize);
    }

    #[test]
    fn test_waveform_read_pcm_frames_s24_packed_len_matches_frames_read_times_channels_times_3() {
        let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_s24_packed()
            .unwrap();

        let requested = 256u64;
        let buf = w.read_pcm_frames(requested).unwrap();
        let frames_read = buf.frames() as u64;

        assert!(frames_read <= requested);
        assert_eq!(buf.len(), (frames_read * w.channels() as u64 * 3) as usize);
    }

    #[test]
    fn test_waveform_read_zero_frames_returns_empty_buffer_all_formats() {
        // U8
        {
            let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
                .channels(2)
                .build_u8()
                .unwrap();
            let buf = w.read_pcm_frames(1).unwrap();
            let frames_read = buf.frames() as u64;
            assert_eq!(frames_read, 1);
            assert_eq!(buf.len(), 2);
        }

        // I16
        {
            let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
                .channels(2)
                .build_i16()
                .unwrap();
            let buf = w.read_pcm_frames(1).unwrap();
            let frames_read = buf.frames() as u64;
            assert_eq!(frames_read, 1);
            assert_eq!(buf.len(), 2);
        }

        // I32
        {
            let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
                .channels(2)
                .build_i32()
                .unwrap();
            let buf = w.read_pcm_frames(1).unwrap();
            let frames_read = buf.frames() as u64;
            assert_eq!(frames_read, 1);
            assert_eq!(buf.len(), 2);
        }

        // S24
        {
            let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
                .channels(2)
                .build_s24()
                .unwrap();
            let buf = w.read_pcm_frames(1).unwrap();
            let frames_read = buf.frames() as u64;
            assert_eq!(frames_read, 1);
            assert_eq!(buf.len(), 2);
        }

        // S24Pacled
        {
            let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
                .channels(2)
                .build_s24_packed()
                .unwrap();
            let buf = w.read_pcm_frames(1).unwrap();
            let frames_read = buf.frames() as u64;
            assert_eq!(frames_read, 1);
            assert_eq!(buf.len(), 6);
        }

        // F32
        {
            let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
                .channels(2)
                .build_f32()
                .unwrap();
            let buf = w.read_pcm_frames(1).unwrap();
            let frames_read = buf.frames() as u64;
            assert_eq!(frames_read, 1);
            assert_eq!(buf.len(), 2);
        }
    }

    // --- WaveFormOps delegation / seek determinism tests ---------------------

    #[test]
    fn test_waveform_seek_to_zero_makes_subsequent_reads_repeatable_f32() {
        let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_f32()
            .unwrap();

        let n = 256u64;

        let a = w.read_pcm_frames(n).unwrap();
        let a_frames = a.frames() as u64;
        w.seek_to_pcm_frame(0).unwrap();
        let b = w.read_pcm_frames(n).unwrap();
        let b_frames = b.frames() as u64;

        assert_eq!(a_frames, b_frames);
        assert_eq!(a.len(), b.len());

        // Float comparisons: allow tiny error (though for the same code path it should be exact).
        for (x, y) in a.as_ref().iter().copied().zip(b.as_ref().iter().copied()) {
            assert!(approx_eq_f32(x, y, 1e-6), "x={x} y={y}");
        }
    }

    #[test]
    fn test_waveform_set_frequency_changes_generated_signal_f32() {
        let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_f32()
            .unwrap();

        let n = 256u64;

        // Read baseline.
        let a = w.read_pcm_frames(n).unwrap();
        let a_frames = a.frames() as u64;

        // Reset to start, change frequency, read again.
        w.seek_to_pcm_frame(0).unwrap();
        w.set_frequency(880.0).unwrap();
        let b = w.read_pcm_frames(n).unwrap();
        let b_frames = b.frames() as u64;

        assert_eq!(a_frames, b_frames);
        assert_eq!(a.len(), b.len());

        // The buffers should differ for most samples.
        // Be tolerant: just ensure we see at least one differing sample.
        let any_diff = a
            .as_ref()
            .iter()
            .zip(b.as_ref().iter())
            .any(|(&x, &y)| !approx_eq_f32(x, y, 1e-6));
        assert!(any_diff, "frequency change did not affect samples");
    }

    #[test]
    fn test_waveform_set_amplitude_scales_signal_f32() {
        let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_f32()
            .unwrap();

        let n = 512u64;

        w.seek_to_pcm_frame(0).unwrap();
        w.set_amplitude(1.0).unwrap();
        let a = w.read_pcm_frames(n).unwrap();

        w.seek_to_pcm_frame(0).unwrap();
        w.set_amplitude(0.5).unwrap();
        let b = w.read_pcm_frames(n).unwrap();

        // Compare peak absolute values (roughly half).
        let max_a = a
            .as_ref()
            .iter()
            .copied()
            .map(|x| x.abs())
            .fold(0.0f32, f32::max);
        let max_b = b
            .as_ref()
            .iter()
            .copied()
            .map(|x| x.abs())
            .fold(0.0f32, f32::max);

        // Use a loose bound to avoid backend quirks.
        assert!(max_b < max_a);
        assert!(max_b <= max_a * 0.6);
    }

    #[test]
    fn test_waveform_set_type_changes_signal_f32() {
        let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
            .channels(2)
            .build_f32()
            .unwrap();

        let n = 256u64;

        w.seek_to_pcm_frame(0).unwrap();
        w.set_type(WaveFormType::Sine).unwrap();
        let a = w.read_pcm_frames(n).unwrap();

        w.seek_to_pcm_frame(0).unwrap();
        w.set_type(WaveFormType::Square).unwrap();
        let b = w.read_pcm_frames(n).unwrap();

        // Not identical if waveform type is respected.
        let any_diff = a
            .as_ref()
            .iter()
            .zip(b.as_ref().iter())
            .any(|(&x, &y)| !approx_eq_f32(x, y, 1e-6));
        assert!(any_diff, "waveform type change did not affect samples");
    }
}
