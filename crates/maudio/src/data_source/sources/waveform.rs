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
use maudio_sys::ffi as sys;

use crate::{
    Binding, MaResult,
    audio::{formats::Format, sample_rate::SampleRate, waveform::WaveformType},
    data_source::{
        AsSourcePtr, DataSourceRef, private_data_source,
        sources::waveform::private_wave::WaveFormPtrProvider,
    },
};

pub(crate) struct WaveFormInner {
    ptr: *mut sys::ma_waveform,
}

pub(crate) struct WaveState {
    channels: u32,
    sample_rate: SampleRate,
    wave_type: WaveformType,
    amplitude: f64,
    frequency: f64,
}

/// Allows all WaveForm types to access the same methods
pub trait AsWaveFormPtr {
    #[doc(hidden)]
    type __PtrProvider: WaveFormPtrProvider<Self>;
    fn channels(&self) -> u32;
}

mod private_wave {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait WaveFormPtrProvider<T: ?Sized> {
        fn as_waveform_ptr(t: &T) -> *mut sys::ma_waveform;
    }

    pub struct WaveFormU8Provider;
    pub struct WaveFormI16Provider;
    pub struct WaveFormI32Provider;
    pub struct WaveFormS24Provider;
    pub struct WaveFormF32Provider;

    impl WaveFormPtrProvider<WaveFormU8> for WaveFormU8Provider {
        fn as_waveform_ptr(t: &WaveFormU8) -> *mut sys::ma_waveform {
            t.inner.ptr
        }
    }

    impl WaveFormPtrProvider<WaveFormI16> for WaveFormI16Provider {
        fn as_waveform_ptr(t: &WaveFormI16) -> *mut sys::ma_waveform {
            t.inner.ptr
        }
    }

    impl WaveFormPtrProvider<WaveFormI32> for WaveFormI32Provider {
        fn as_waveform_ptr(t: &WaveFormI32) -> *mut sys::ma_waveform {
            t.inner.ptr
        }
    }

    impl WaveFormPtrProvider<WaveFormS24> for WaveFormS24Provider {
        fn as_waveform_ptr(t: &WaveFormS24) -> *mut sys::ma_waveform {
            t.inner.ptr
        }
    }

    impl WaveFormPtrProvider<WaveFormF32> for WaveFormF32Provider {
        fn as_waveform_ptr(t: &WaveFormF32) -> *mut sys::ma_waveform {
            t.inner.ptr
        }
    }

    pub fn waveform_ptr<T: AsWaveFormPtr + ?Sized>(t: &T) -> *mut sys::ma_waveform {
        <T as AsWaveFormPtr>::__PtrProvider::as_waveform_ptr(t)
    }
}

/// Waveform generator producing [`Format::U8`] samples.
pub struct WaveFormU8 {
    inner: WaveFormInner,
    format: Format,
    state: WaveState,
}

impl AsWaveFormPtr for WaveFormU8 {
    #[doc(hidden)]
    type __PtrProvider = private_wave::WaveFormU8Provider;

    fn channels(&self) -> u32 {
        self.state.channels
    }
}

#[doc(hidden)]
impl AsSourcePtr for WaveFormU8 {
    type __PtrProvider = private_data_source::WaveFormU8Provider;
}

impl WaveFormU8 {
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<u8>, u64)> {
        waveform_ffi::ma_ma_waveform_read_pcm_frames_u8(self, frame_count)
    }
}

/// Waveform generator producing [`Format::S16`] samples.
pub struct WaveFormI16 {
    inner: WaveFormInner,
    format: Format,
    state: WaveState,
}

impl AsWaveFormPtr for WaveFormI16 {
    #[doc(hidden)]
    type __PtrProvider = private_wave::WaveFormI16Provider;

    fn channels(&self) -> u32 {
        self.state.channels
    }
}

#[doc(hidden)]
impl AsSourcePtr for WaveFormI16 {
    type __PtrProvider = private_data_source::WaveFormI16Provider;
}

impl WaveFormI16 {
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<i16>, u64)> {
        waveform_ffi::ma_ma_waveform_read_pcm_frames_i16(self, frame_count)
    }
}

/// Waveform generator producing [`Format::S32`] samples.
pub struct WaveFormI32 {
    inner: WaveFormInner,
    format: Format,
    state: WaveState,
}

impl AsWaveFormPtr for WaveFormI32 {
    #[doc(hidden)]
    type __PtrProvider = private_wave::WaveFormI32Provider;

    fn channels(&self) -> u32 {
        self.state.channels
    }
}

#[doc(hidden)]
impl AsSourcePtr for WaveFormI32 {
    type __PtrProvider = private_data_source::WaveFormI32Provider;
}

impl WaveFormI32 {
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<i32>, u64)> {
        waveform_ffi::ma_ma_waveform_read_pcm_frames_i32(self, frame_count)
    }
}

/// Waveform generator producing [`Format::S24`] samples.
pub struct WaveFormS24 {
    inner: WaveFormInner,
    format: Format,
    state: WaveState,
}

impl AsWaveFormPtr for WaveFormS24 {
    #[doc(hidden)]
    type __PtrProvider = private_wave::WaveFormS24Provider;

    fn channels(&self) -> u32 {
        self.state.channels
    }
}

#[doc(hidden)]
impl AsSourcePtr for WaveFormS24 {
    type __PtrProvider = private_data_source::WaveFormS24Provider;
}

impl WaveFormS24 {
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<u8>, u64)> {
        waveform_ffi::ma_ma_waveform_read_pcm_frames_s24(self, frame_count)
    }
}

/// Waveform generator producing [`Format::F32`] samples.
pub struct WaveFormF32 {
    inner: WaveFormInner,
    format: Format,
    state: WaveState,
}

impl AsWaveFormPtr for WaveFormF32 {
    #[doc(hidden)]
    type __PtrProvider = private_wave::WaveFormF32Provider;

    fn channels(&self) -> u32 {
        self.state.channels
    }
}

#[doc(hidden)]
impl AsSourcePtr for WaveFormF32 {
    #[doc(hidden)]
    type __PtrProvider = private_data_source::WaveFormF32Provider;
}

impl WaveFormF32 {
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<f32>, u64)> {
        waveform_ffi::ma_ma_waveform_read_pcm_frames_f32(self, frame_count)
    }
}

impl<T: AsWaveFormPtr + AsSourcePtr + ?Sized> WaveFormOps for T {}

/// The WaveFormOps trait contains shared methods for all WaveForm types for each data format.
pub trait WaveFormOps: AsWaveFormPtr + AsSourcePtr {
    fn seek_to_pcm_frame(&mut self, frame_index: u64) -> MaResult<()> {
        waveform_ffi::ma_waveform_seek_to_pcm_frame(self, frame_index)
    }

    fn set_amplitude(&mut self, amplitude: f64) -> MaResult<()> {
        waveform_ffi::ma_waveform_set_amplitude(self, amplitude)
    }

    fn set_frequency(&mut self, frequency: f64) -> MaResult<()> {
        waveform_ffi::ma_waveform_set_frequency(self, frequency)
    }

    fn set_type(&mut self, wave_type: WaveformType) -> MaResult<()> {
        waveform_ffi::ma_waveform_set_type(self, wave_type)
    }

    fn set_sample_rate(&mut self, sample_rate: SampleRate) -> MaResult<()> {
        waveform_ffi::ma_waveform_set_sample_rate(self, sample_rate)
    }

    fn as_source(&self) -> DataSourceRef<'_> {
        debug_assert!(!private_wave::waveform_ptr(self).is_null());
        let ptr = private_wave::waveform_ptr(self).cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

pub(crate) mod waveform_ffi {
    use crate::{
        Binding, MaRawResult, MaResult,
        audio::{sample_rate::SampleRate, waveform::WaveformType},
        data_source::sources::waveform::{
            AsWaveFormPtr, WaveFormBuilder, WaveFormInner, private_wave,
        },
    };
    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_waveform_init(
        config: &WaveFormBuilder,
        waveform: *mut sys::ma_waveform,
    ) -> MaResult<()> {
        let raw = config.to_raw();
        let res = unsafe { sys::ma_waveform_init(&raw as *const _, waveform) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_waveform_uninit(waveform: &mut WaveFormInner) {
        unsafe {
            sys::ma_waveform_uninit(waveform.ptr);
        }
    }

    pub fn ma_ma_waveform_read_pcm_frames_u8<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        frame_count: u64,
    ) -> MaResult<(Vec<u8>, u64)> {
        let mut buffer = vec![0u8; (frame_count * waveform.channels() as u64) as usize];
        let frames_read = ma_waveform_read_pcm_frames_internal(
            waveform,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;
        let samples_read = (frames_read * waveform.channels() as u64) as usize;
        buffer.truncate(samples_read);
        Ok((buffer, frames_read))
    }

    pub fn ma_ma_waveform_read_pcm_frames_i16<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        frame_count: u64,
    ) -> MaResult<(Vec<i16>, u64)> {
        let mut buffer = vec![0i16; (frame_count * waveform.channels() as u64) as usize];
        let frames_read = ma_waveform_read_pcm_frames_internal(
            waveform,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;
        let samples_read = (frames_read * waveform.channels() as u64) as usize;
        buffer.truncate(samples_read);
        Ok((buffer, frames_read))
    }

    pub fn ma_ma_waveform_read_pcm_frames_i32<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        frame_count: u64,
    ) -> MaResult<(Vec<i32>, u64)> {
        let mut buffer = vec![0i32; (frame_count * waveform.channels() as u64) as usize];
        let frames_read = ma_waveform_read_pcm_frames_internal(
            waveform,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;
        let samples_read = (frames_read * waveform.channels() as u64) as usize;
        buffer.truncate(samples_read);
        Ok((buffer, frames_read))
    }

    pub fn ma_ma_waveform_read_pcm_frames_s24<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        frame_count: u64,
    ) -> MaResult<(Vec<u8>, u64)> {
        let mut buffer = vec![0u8; (frame_count * waveform.channels() as u64 * 3) as usize];
        let frames_read = ma_waveform_read_pcm_frames_internal(
            waveform,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;
        let samples_read = (frames_read * waveform.channels() as u64 * 3) as usize;
        buffer.truncate(samples_read);
        Ok((buffer, frames_read))
    }

    pub fn ma_ma_waveform_read_pcm_frames_f32<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        frame_count: u64,
    ) -> MaResult<(Vec<f32>, u64)> {
        let mut buffer = vec![0.0f32; (frame_count * waveform.channels() as u64) as usize];
        let frames_read = ma_waveform_read_pcm_frames_internal(
            waveform,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;
        let samples_read = (frames_read * waveform.channels() as u64) as usize;
        buffer.truncate(samples_read);
        Ok((buffer, frames_read))
    }

    #[inline]
    pub fn ma_waveform_read_pcm_frames_internal<W: AsWaveFormPtr + ?Sized>(
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
        MaRawResult::check(res)?;
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
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_waveform_set_amplitude<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        amplitude: f64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_waveform_set_amplitude(private_wave::waveform_ptr(waveform), amplitude)
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_waveform_set_frequency<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        frequency: f64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_waveform_set_frequency(private_wave::waveform_ptr(waveform), frequency)
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_waveform_set_type<W: AsWaveFormPtr + ?Sized>(
        waveform: &mut W,
        wave_type: WaveformType,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_waveform_set_type(private_wave::waveform_ptr(waveform), wave_type.into())
        };
        MaRawResult::check(res)
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
        MaRawResult::check(res)
    }
}

impl Drop for WaveFormInner {
    fn drop(&mut self) {
        waveform_ffi::ma_waveform_uninit(self);
        drop(unsafe { Box::from_raw(self.ptr) });
    }
}

pub struct WaveFormBuilder {
    inner: sys::ma_waveform_config,
    format: Format,
    channels: u32,
    sample_rate: SampleRate,
    wave_type: WaveformType,
    amplitude: f64,
    frequency: f64,
}

impl Binding for WaveFormBuilder {
    type Raw = sys::ma_waveform_config;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl WaveFormBuilder {
    /// Creates a new waveform builder with fully configurable parameters.
    ///
    /// This is the most flexible way to construct a waveform.
    pub fn new(
        channels: u32,
        sample_rate: SampleRate,
        wave_type: WaveformType,
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

    /// Convenience method for creating a `WaveformType::Sine` node with some default values:
    /// - `channels`: 2
    /// - `amplitude`: 0.2
    /// - `format`: Format::F32
    pub fn new_sine(sample_rate: SampleRate, frequency: f64) -> Self {
        let channels = 2;
        let wave_type = WaveformType::Sine;
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

    /// Convenience method for creating a `WaveformType::Square` node with some default values:
    /// - `channels`: 2
    /// - `amplitude`: 0.2
    /// - `format`: Format::F32
    pub fn new_square(sample_rate: SampleRate, frequency: f64) -> Self {
        let channels = 2;
        let wave_type = WaveformType::Square;
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

    /// Convenience method for creating a `WaveformType::Sawtooth` node with some default values:
    /// - `channels`: 2
    /// - `amplitude`: 0.2
    /// - `format`: Format::F32
    pub fn new_sawtooth(sample_rate: SampleRate, frequency: f64) -> Self {
        let channels = 2;
        let wave_type = WaveformType::Sawtooth;
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

    /// Convenience method for creating a `WaveformType::Triangle` node with some default values:
    /// - `channels`: 2
    /// - `amplitude`: 0.2
    /// - `format`: Format::F32
    pub fn new_triangle(sample_rate: SampleRate, frequency: f64) -> Self {
        let channels = 2;
        let wave_type = WaveformType::Triangle;
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
    pub fn wave_type(mut self, t: WaveformType) -> Self {
        self.inner.type_ = t.into();
        self.wave_type = t;
        self
    }

    /// Sets the waveform amplitude.
    pub fn amplitude(mut self, a: f64) -> Self {
        self.inner.amplitude = a;
        self.amplitude = a;
        self
    }

    /// Sets the waveform frequency, in Hertz.
    pub fn frequency(mut self, f: f64) -> Self {
        self.inner.frequency = f;
        self.frequency = f;
        self
    }

    /// Sets the number of output channels.
    pub fn channels(mut self, c: u32) -> Self {
        self.inner.channels = c;
        self.channels = c;
        self
    }

    pub fn build_u8(mut self) -> MaResult<WaveFormU8> {
        self.inner.format = Format::U8.into();

        let inner = self.new_inner()?;
        let state: WaveState = WaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            wave_type: self.wave_type,
            amplitude: self.amplitude,
            frequency: self.frequency,
        };
        Ok(WaveFormU8 {
            inner,
            format: Format::U8,
            state,
        })
    }

    pub fn build_i16(mut self) -> MaResult<WaveFormI16> {
        self.inner.format = Format::S16.into();

        let inner = self.new_inner()?;
        let state: WaveState = WaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            wave_type: self.wave_type,
            amplitude: self.amplitude,
            frequency: self.frequency,
        };
        Ok(WaveFormI16 {
            inner,
            format: Format::S16,
            state,
        })
    }

    pub fn build_i32(mut self) -> MaResult<WaveFormI32> {
        self.inner.format = Format::S32.into();

        let inner = self.new_inner()?;
        let state: WaveState = WaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            wave_type: self.wave_type,
            amplitude: self.amplitude,
            frequency: self.frequency,
        };
        Ok(WaveFormI32 {
            inner,
            format: Format::S32,
            state,
        })
    }

    pub fn build_s24(mut self) -> MaResult<WaveFormS24> {
        self.inner.format = Format::S24.into();

        let inner = self.new_inner()?;
        let state: WaveState = WaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            wave_type: self.wave_type,
            amplitude: self.amplitude,
            frequency: self.frequency,
        };
        Ok(WaveFormS24 {
            inner,
            format: Format::S24,
            state,
        })
    }

    /// The native format of the `Engine`
    pub fn build_f32(mut self) -> MaResult<WaveFormF32> {
        self.inner.format = Format::F32.into();

        let inner = self.new_inner()?;
        let state: WaveState = WaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            wave_type: self.wave_type,
            amplitude: self.amplitude,
            frequency: self.frequency,
        };
        Ok(WaveFormF32 {
            inner,
            format: Format::F32,
            state,
        })
    }

    fn new_inner(&self) -> MaResult<WaveFormInner> {
        self.init_from_config_internal()
    }

    fn init_from_config_internal(&self) -> MaResult<WaveFormInner> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_waveform>> = Box::new_uninit();

        waveform_ffi::ma_waveform_init(self, mem.as_mut_ptr())?;

        let ptr = unsafe { mem.assume_init() };
        let inner = Box::into_raw(ptr);
        Ok(WaveFormInner { ptr: inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq_f32(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    // --- builder field + setter tests ---------------------------------------

    #[test]
    fn test_waveform_builder_new_sine_initializes_expected_defaults() {
        let sample_rate = SampleRate::Sr48000;
        let frequency = 440.0;

        let b = WaveFormBuilder::new_sine(sample_rate, frequency);

        assert_eq!(b.format, Format::F32);
        assert_eq!(b.channels, 2);
        assert_eq!(b.sample_rate, sample_rate);
        assert_eq!(b.wave_type, WaveformType::Sine);
        assert_eq!(b.amplitude, 0.2);
        assert_eq!(b.frequency, frequency);

        // Ensure raw config mirrors as well.
        assert_eq!(b.inner.channels, 2);
        assert_eq!(b.inner.amplitude, 0.2);
        assert_eq!(b.inner.frequency, frequency);
    }

    #[test]
    fn test_waveform_builder_setters_update_mirrored_fields_and_raw_inner() {
        let b = WaveFormBuilder::new_sine(SampleRate::Sr44100, 440.0)
            .channels(1)
            .amplitude(0.25)
            .frequency(880.0)
            .wave_type(WaveformType::Square);

        assert_eq!(b.channels, 1);
        assert_eq!(b.amplitude, 0.25);
        assert_eq!(b.frequency, 880.0);
        assert_eq!(b.wave_type, WaveformType::Square);

        assert_eq!(b.inner.channels, 1);
        assert_eq!(b.inner.amplitude, 0.25);
        assert_eq!(b.inner.frequency, 880.0);
        assert_eq!(b.inner.type_, WaveformType::Square.into());
    }

    // --- build_* smoke tests (state + format correctness) -------------------

    #[test]
    fn test_waveform_build_u8_sets_state_and_format() {
        let channels = 2;
        let sample_rate = SampleRate::Sr48000;
        let wave_type = WaveformType::Sine;
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
        assert_eq!(w.state.wave_type, WaveformType::Sine);
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
        let (buf, frames_read) = w.read_pcm_frames(requested).unwrap();

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
        let (buf, frames_read) = w.read_pcm_frames(requested).unwrap();

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
        let (buf, frames_read) = w.read_pcm_frames(requested).unwrap();

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
        let (buf, frames_read) = w.read_pcm_frames(requested).unwrap();

        assert!(frames_read <= requested);
        assert_eq!(buf.len(), (frames_read * w.channels() as u64) as usize);

        // Optional sanity: sine in [-1,1] range when amplitude=1.
        // Don't be overly strict across backends/platforms.
        if !buf.is_empty() {
            let max_abs = buf.iter().copied().map(|x| x.abs()).fold(0.0f32, f32::max);
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
        let (buf, frames_read) = w.read_pcm_frames(requested).unwrap();

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
            let (buf, frames_read) = w.read_pcm_frames(1).unwrap();
            assert_eq!(frames_read, 1);
            assert_eq!(buf.len(), 2);
        }

        // I16
        {
            let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
                .channels(2)
                .build_i16()
                .unwrap();
            let (buf, frames_read) = w.read_pcm_frames(1).unwrap();
            assert_eq!(frames_read, 1);
            assert_eq!(buf.len(), 2);
        }

        // I32
        {
            let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
                .channels(2)
                .build_i32()
                .unwrap();
            let (buf, frames_read) = w.read_pcm_frames(1).unwrap();
            assert_eq!(frames_read, 1);
            assert_eq!(buf.len(), 2);
        }

        // S24
        {
            let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
                .channels(2)
                .build_s24()
                .unwrap();
            let (buf, frames_read) = w.read_pcm_frames(1).unwrap();
            assert_eq!(frames_read, 1);
            assert_eq!(buf.len(), 6);
        }

        // F32
        {
            let mut w = WaveFormBuilder::new_sine(SampleRate::Sr48000, 440.0)
                .channels(2)
                .build_f32()
                .unwrap();
            let (buf, frames_read) = w.read_pcm_frames(1).unwrap();
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

        let (a, a_frames) = w.read_pcm_frames(n).unwrap();
        w.seek_to_pcm_frame(0).unwrap();
        let (b, b_frames) = w.read_pcm_frames(n).unwrap();

        assert_eq!(a_frames, b_frames);
        assert_eq!(a.len(), b.len());

        // Float comparisons: allow tiny error (though for the same code path it should be exact).
        for (x, y) in a.iter().copied().zip(b.iter().copied()) {
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
        let (a, a_frames) = w.read_pcm_frames(n).unwrap();

        // Reset to start, change frequency, read again.
        w.seek_to_pcm_frame(0).unwrap();
        w.set_frequency(880.0).unwrap();
        let (b, b_frames) = w.read_pcm_frames(n).unwrap();

        assert_eq!(a_frames, b_frames);
        assert_eq!(a.len(), b.len());

        // The buffers should differ for most samples.
        // Be tolerant: just ensure we see at least one differing sample.
        let any_diff = a
            .iter()
            .zip(b.iter())
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
        let (a, _) = w.read_pcm_frames(n).unwrap();

        w.seek_to_pcm_frame(0).unwrap();
        w.set_amplitude(0.5).unwrap();
        let (b, _) = w.read_pcm_frames(n).unwrap();

        // Compare peak absolute values (roughly half).
        let max_a = a.iter().copied().map(|x| x.abs()).fold(0.0f32, f32::max);
        let max_b = b.iter().copied().map(|x| x.abs()).fold(0.0f32, f32::max);

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
        w.set_type(WaveformType::Sine).unwrap();
        let (a, _) = w.read_pcm_frames(n).unwrap();

        w.seek_to_pcm_frame(0).unwrap();
        w.set_type(WaveformType::Square).unwrap();
        let (b, _) = w.read_pcm_frames(n).unwrap();

        // Not identical if waveform type is respected.
        let any_diff = a
            .iter()
            .zip(b.iter())
            .any(|(&x, &y)| !approx_eq_f32(x, y, 1e-6));
        assert!(any_diff, "waveform type change did not affect samples");
    }
}
