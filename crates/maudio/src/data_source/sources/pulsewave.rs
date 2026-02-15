//! Pulse wave signal generator.
//!
//! Produces a procedural pulse (square) wave and exposes it as a [`DataSource`](crate::data_source::DataSource).
//! This is useful for synthesis, testing, and generating example audio without
//! loading files.
//!
//! The generator can be controlled at runtime via [`PulseWaveOps`] (amplitude,
//! frequency, duty cycle, and sample rate) and can be seeked like any other
//! source.
use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{
        formats::{Format, SampleBuffer},
        sample_rate::SampleRate,
    },
    data_source::{private_data_source, AsSourcePtr, DataSourceRef},
    pcm_frames::{PcmFormat, S24Packed, S24},
    Binding, MaResult,
};

#[derive(Debug)]
pub(crate) struct PulseWaveState {
    channels: u32,
    sample_rate: SampleRate,
    amplitude: f64,
    frequency: f64,
    duty_cycle: f64,
}

pub struct PulseWave<F: PcmFormat> {
    inner: *mut sys::ma_pulsewave,
    format: Format,
    state: PulseWaveState,
    _sample_format: PhantomData<F>,
}

#[doc(hidden)]
impl<F: PcmFormat> AsSourcePtr for PulseWave<F> {
    type __PtrProvider = private_data_source::PulseWaveProvider;
}

pub trait AsPulseWavePtr {
    #[doc(hidden)]
    type __PtrProvider: private_pulsew::PulseWavePtrProvider<Self>;
    fn channels(&self) -> u32;
}

impl<F: PcmFormat> AsPulseWavePtr for PulseWave<F> {
    type __PtrProvider = private_pulsew::PulseWaveProvider;

    fn channels(&self) -> u32 {
        self.state.channels
    }
}

mod private_pulsew {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait PulseWavePtrProvider<T: ?Sized> {
        fn as_pulsewave_ptr(t: &T) -> *mut sys::ma_pulsewave;
    }

    pub struct PulseWaveProvider;

    impl<F: PcmFormat> PulseWavePtrProvider<PulseWave<F>> for PulseWaveProvider {
        fn as_pulsewave_ptr(t: &PulseWave<F>) -> *mut sys::ma_pulsewave {
            t.inner
        }
    }

    pub fn pulsewave_ptr<T: AsPulseWavePtr + ?Sized>(t: &T) -> *mut sys::ma_pulsewave {
        <T as AsPulseWavePtr>::__PtrProvider::as_pulsewave_ptr(t)
    }
}

impl<F: PcmFormat> PulseWaveOps for PulseWave<F> {
    type Format = F;
}

/// The PulseWaveOps trait contains shared methods for all PulseWave types for each data format.
pub trait PulseWaveOps: AsPulseWavePtr + AsSourcePtr {
    type Format: PcmFormat;

    fn read_pcm_frames_into(
        &mut self,
        dst: &mut [<Self::Format as PcmFormat>::PcmUnit],
    ) -> MaResult<usize> {
        pulsewave_ffi::ma_pulsewave_read_pcm_frames_into::<Self::Format, Self>(self, dst)
    }

    fn read_pcm_frames(&mut self, frames: u64) -> MaResult<SampleBuffer<Self::Format>> {
        pulsewave_ffi::ma_pulsewave_read_pcm_frames(self, frames)
    }

    fn seek_to_pcm_frame(&mut self, frame_index: u64) -> MaResult<()> {
        pulsewave_ffi::ma_pulsewave_seek_to_pcm_frame(self, frame_index)
    }

    fn set_amplitude(&mut self, amplitude: f64) -> MaResult<()> {
        pulsewave_ffi::ma_pulsewave_set_amplitude(self, amplitude)
    }

    fn set_frequency(&mut self, frequency: f64) -> MaResult<()> {
        pulsewave_ffi::ma_pulsewave_set_frequency(self, frequency)
    }

    fn set_duty_cycle(&mut self, duty_cycle: f64) -> MaResult<()> {
        pulsewave_ffi::ma_pulsewave_set_duty_cycle(self, duty_cycle)
    }

    fn set_sample_rate(&mut self, sample_rate: SampleRate) -> MaResult<()> {
        pulsewave_ffi::ma_pulsewave_set_sample_rate(self, sample_rate)
    }

    fn as_source(&self) -> DataSourceRef<'_> {
        debug_assert!(!private_pulsew::pulsewave_ptr(self).is_null());
        let ptr = private_pulsew::pulsewave_ptr(self).cast::<sys::ma_data_source>();
        DataSourceRef::from_ptr(ptr)
    }
}

pub(crate) mod pulsewave_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        audio::{formats::SampleBuffer, sample_rate::SampleRate},
        data_source::sources::pulsewave::{private_pulsew, AsPulseWavePtr, PulseWaveBuilder},
        pcm_frames::{PcmFormat, PcmFormatInternal},
        Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_pulsewave_init(
        config: &PulseWaveBuilder,
        pulsewave: *mut sys::ma_pulsewave,
    ) -> MaResult<()> {
        let raw = config.to_raw();
        let res = unsafe { sys::ma_pulsewave_init(&raw as *const _, pulsewave) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_pulsewave_uninit<W: AsPulseWavePtr + ?Sized>(pulsewave: &mut W) {
        unsafe { sys::ma_pulsewave_uninit(private_pulsew::pulsewave_ptr(pulsewave)) }
    }

    pub fn ma_pulsewave_read_pcm_frames_into<F: PcmFormat, W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        dst: &mut [F::PcmUnit],
    ) -> MaResult<usize> {
        let channels = pw.channels();
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        let frame_count = (dst.len() / channels as usize / F::VEC_PCM_UNITS_PER_FRAME) as u64;

        match F::DIRECT_READ {
            true => {
                let frames_read = ma_pulsewave_read_pcm_frames_internal(
                    pw,
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
                let frames_read = ma_pulsewave_read_pcm_frames_internal(
                    pw,
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

    pub fn ma_pulsewave_read_pcm_frames<F: PcmFormat, W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<F>> {
        let mut buffer = SampleBuffer::<F>::new_zeroed(frame_count as usize, pw.channels())?;

        let frames_read = ma_pulsewave_read_pcm_frames_internal(
            pw,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<F>::from_storage(buffer, frames_read as usize, pw.channels())
    }

    fn ma_pulsewave_read_pcm_frames_internal<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_pulsewave_read_pcm_frames(
                private_pulsew::pulsewave_ptr(pw),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_read)
    }

    #[inline]
    pub fn ma_pulsewave_seek_to_pcm_frame<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frame_index: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_pulsewave_seek_to_pcm_frame(private_pulsew::pulsewave_ptr(pw), frame_index)
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_pulsewave_set_amplitude<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        amplitude: f64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_pulsewave_set_amplitude(private_pulsew::pulsewave_ptr(pw), amplitude)
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_pulsewave_set_frequency<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frequency: f64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_pulsewave_set_frequency(private_pulsew::pulsewave_ptr(pw), frequency)
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_pulsewave_set_duty_cycle<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        duty_cycle: f64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_pulsewave_set_duty_cycle(private_pulsew::pulsewave_ptr(pw), duty_cycle)
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_pulsewave_set_sample_rate<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        sample_rate: SampleRate,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_pulsewave_set_sample_rate(private_pulsew::pulsewave_ptr(pw), sample_rate.into())
        };
        MaudioError::check(res)
    }
}

impl<F: PcmFormat> Drop for PulseWave<F> {
    fn drop(&mut self) {
        pulsewave_ffi::ma_pulsewave_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) })
    }
}

pub struct PulseWaveBuilder {
    inner: sys::ma_pulsewave_config,
    format: Format,
    channels: u32,
    sample_rate: SampleRate,
    amplitude: f64,
    frequency: f64,
    duty_cycle: f64,
}

impl Binding for PulseWaveBuilder {
    type Raw = sys::ma_pulsewave_config;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl PulseWaveBuilder {
    pub fn new(
        channels: u32,
        sample_rate: SampleRate,
        amplitude: f64,
        frequency: f64,
        duty_cycle: f64,
    ) -> Self {
        let cfg = unsafe {
            sys::ma_pulsewave_config_init(
                Format::F32.into(),
                channels,
                sample_rate.into(),
                duty_cycle,
                amplitude,
                frequency,
            )
        };
        Self {
            inner: cfg,
            format: Format::F32,
            channels,
            sample_rate,
            amplitude,
            frequency,
            duty_cycle,
        }
    }

    pub fn amplitude(&mut self, a: f64) -> &mut Self {
        self.inner.amplitude = a;
        self.amplitude = a;
        self
    }

    pub fn frequency(&mut self, f: f64) -> &mut Self {
        self.inner.frequency = f;
        self.frequency = f;
        self
    }

    pub fn duty_cycle(&mut self, d: f64) -> &mut Self {
        self.inner.dutyCycle = d;
        self.duty_cycle = d;
        self
    }

    pub fn channels(&mut self, c: u32) -> &mut Self {
        self.inner.channels = c;
        self.channels = c;
        self
    }

    pub fn build_u8(&mut self) -> MaResult<PulseWave<u8>> {
        self.inner.format = Format::U8.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(PulseWave {
            inner,
            format: Format::U8,
            state,
            _sample_format: PhantomData,
        })
    }

    pub fn build_i16(&mut self) -> MaResult<PulseWave<i16>> {
        self.inner.format = Format::S16.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(PulseWave {
            inner,
            format: Format::S16,
            state,
            _sample_format: PhantomData,
        })
    }

    pub fn build_i32(&mut self) -> MaResult<PulseWave<i32>> {
        self.inner.format = Format::S32.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(PulseWave {
            inner,
            format: Format::S32,
            state,
            _sample_format: PhantomData,
        })
    }

    pub fn build_s24(&mut self) -> MaResult<PulseWave<S24>> {
        self.inner.format = Format::S24.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(PulseWave {
            inner,
            format: Format::S24,
            state,
            _sample_format: PhantomData,
        })
    }

    pub fn build_s24_packed(&mut self) -> MaResult<PulseWave<S24Packed>> {
        self.inner.format = Format::S24.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(PulseWave {
            inner,
            format: Format::S24,
            state,
            _sample_format: PhantomData,
        })
    }

    pub fn build_f32(&mut self) -> MaResult<PulseWave<f32>> {
        self.inner.format = Format::F32.into();

        let inner = self.new_inner()?;
        let state = self.new_state();

        Ok(PulseWave {
            inner,
            format: Format::F32,
            state,
            _sample_format: PhantomData,
        })
    }

    fn new_state(&self) -> PulseWaveState {
        PulseWaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            amplitude: self.amplitude,
            frequency: self.frequency,
            duty_cycle: self.duty_cycle,
        }
    }

    fn new_inner(&self) -> MaResult<*mut sys::ma_pulsewave> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_pulsewave>> =
            Box::new(MaybeUninit::uninit());

        pulsewave_ffi::ma_pulsewave_init(self, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_pulsewave = Box::into_raw(mem) as *mut sys::ma_pulsewave;

        Ok(inner)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        audio::sample_rate::SampleRate,
        data_source::sources::pulsewave::{PulseWaveBuilder, PulseWaveOps},
    };

    #[test]
    fn test_pulsewave_basic_init() {
        let _pw = PulseWaveBuilder::new(2, SampleRate::Sr48000, 1.0, 440.0, 1.0)
            .build_f32()
            .unwrap();
    }

    const CH: u32 = 2;
    const FRAMES: u64 = 128;

    fn all_sample_rates() -> [SampleRate; 14] {
        [
            SampleRate::Sr48000,
            SampleRate::Sr44100,
            SampleRate::Sr32000,
            SampleRate::Sr24000,
            SampleRate::Sr22050,
            SampleRate::Sr88200,
            SampleRate::Sr96000,
            SampleRate::Sr176400,
            SampleRate::Sr192000,
            SampleRate::Sr16000,
            SampleRate::Sr11025,
            SampleRate::Sr8000,
            SampleRate::Sr352800,
            SampleRate::Sr384000,
        ]
    }

    fn assert_frames_and_len_u8(buf: &[u8], frames_read: u64, channels: u32) {
        assert_eq!(frames_read, FRAMES);
        assert_eq!(buf.len(), (frames_read * channels as u64) as usize);
    }

    fn assert_frames_and_len_i16(buf: &[i16], frames_read: u64, channels: u32) {
        assert_eq!(frames_read, FRAMES);
        assert_eq!(buf.len(), (frames_read * channels as u64) as usize);
    }

    fn assert_frames_and_len_i32(buf: &[i32], frames_read: u64, channels: u32) {
        assert_eq!(frames_read, FRAMES);
        assert_eq!(buf.len(), (frames_read * channels as u64) as usize);
    }

    fn assert_frames_and_len_s24(buf: &[i32], frames_read: u64, channels: u32) {
        assert_eq!(frames_read, FRAMES);
        assert_eq!(buf.len(), (frames_read * channels as u64) as usize);
    }

    fn assert_frames_and_len_s24_packed(buf: &[u8], frames_read: u64, channels: u32) {
        assert_eq!(frames_read, FRAMES);
        assert_eq!(buf.len(), (frames_read * channels as u64 * 3) as usize);
    }

    fn assert_frames_and_len_f32(buf: &[f32], frames_read: u64, channels: u32) {
        assert_eq!(frames_read, FRAMES);
        assert_eq!(buf.len(), (frames_read * channels as u64) as usize);
    }

    #[test]
    fn test_pulsewave_basic_init_f32() {
        let _pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_f32()
            .unwrap();
    }

    #[test]
    fn test_pulsewave_generates_nonzero_f32() {
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 0.2, 440.0, 0.5)
            .build_f32()
            .unwrap();

        let buf = pw.read_pcm_frames(FRAMES).unwrap();
        let frames_read = buf.frames() as u64;
        assert_frames_and_len_f32(buf.as_ref(), frames_read, CH);

        assert!(
            buf.as_ref().iter().all(|&s| s.is_finite()),
            "PulseWave produced NaN/Inf samples"
        );

        let max_abs = buf.as_ref().iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        assert!(
            max_abs > 1.0e-6,
            "PulseWave output looks silent (max_abs={max_abs})"
        );

        let mut max_delta = 0.0f32;
        for w in buf.as_ref().windows(2) {
            let d = (w[1] - w[0]).abs();
            if d > max_delta {
                max_delta = d;
            }
        }
        assert!(
            max_delta > 1.0e-6,
            "PulseWave output looks constant/DC (max_delta={max_delta})"
        );
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_u8_sizing() {
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_u8()
            .unwrap();

        let buf = pw.read_pcm_frames(FRAMES).unwrap();
        let frames_read = buf.frames() as u64;
        assert_frames_and_len_u8(buf.as_ref(), frames_read, CH);
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_i16_sizing() {
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_i16()
            .unwrap();

        let buf = pw.read_pcm_frames(FRAMES).unwrap();
        let frames_read = buf.frames() as u64;
        assert_frames_and_len_i16(buf.as_ref(), frames_read, CH);
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_i32_sizing() {
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_i32()
            .unwrap();

        let buf = pw.read_pcm_frames(FRAMES).unwrap();
        let frames_read = buf.frames() as u64;
        assert_frames_and_len_i32(buf.as_ref(), frames_read, CH);
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_s24_sizing() {
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_s24()
            .unwrap();

        let buf = pw.read_pcm_frames(FRAMES).unwrap();
        let frames_read = buf.frames() as u64;
        assert_frames_and_len_s24(buf.as_ref(), frames_read, CH);
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_s24_packed_sizing() {
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_s24_packed()
            .unwrap();

        let buf = pw.read_pcm_frames(FRAMES).unwrap();
        let frames_read = buf.frames() as u64;
        assert_frames_and_len_s24_packed(buf.as_ref(), frames_read, CH);
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_f32_sizing() {
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_f32()
            .unwrap();

        let buf = pw.read_pcm_frames(FRAMES).unwrap();
        let frames_read = buf.frames() as u64;
        assert_frames_and_len_f32(buf.as_ref(), frames_read, CH);
    }

    #[test]
    fn test_pulsewave_seek_is_deterministic_f32() {
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_f32()
            .unwrap();

        pw.seek_to_pcm_frame(0).unwrap();
        let a = pw.read_pcm_frames(FRAMES).unwrap();
        let fa = a.frames() as u64;
        assert_frames_and_len_f32(a.as_ref(), fa, CH);

        pw.seek_to_pcm_frame(0).unwrap();
        let b = pw.read_pcm_frames(FRAMES).unwrap();
        let fb = b.frames() as u64;
        assert_frames_and_len_f32(b.as_ref(), fb, CH);

        assert_eq!(
            a.as_ref(),
            b.as_ref(),
            "Expected identical output after seek_to_pcm_frame(0)"
        );
    }

    #[test]
    fn test_pulsewave_set_frequency_changes_output_f32() {
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 0.8, 400.0, 0.5)
            .build_f32()
            .unwrap();

        pw.seek_to_pcm_frame(0).unwrap();
        let a = pw.read_pcm_frames(FRAMES).unwrap();

        pw.set_frequency(440.0).unwrap();
        pw.seek_to_pcm_frame(0).unwrap();
        let b = pw.read_pcm_frames(FRAMES).unwrap();

        assert_ne!(
            a.as_ref(),
            b.as_ref(),
            "Changing frequency should change generated samples"
        );
    }

    #[test]
    fn test_pulsewave_set_amplitude_zero_silences_signed_and_float() {
        // F32 -> expect all zeros
        let mut pw_f32 = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_f32()
            .unwrap();
        pw_f32.set_amplitude(0.0).unwrap();
        pw_f32.seek_to_pcm_frame(0).unwrap();
        let buf_f32 = pw_f32.read_pcm_frames(FRAMES).unwrap();
        assert!(buf_f32.as_ref().iter().all(|&s| s == 0.0));

        // S16 -> expect all zeros
        let mut pw_i16 = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_i16()
            .unwrap();
        pw_i16.set_amplitude(0.0).unwrap();
        pw_i16.seek_to_pcm_frame(0).unwrap();
        let buf_i16 = pw_i16.read_pcm_frames(FRAMES).unwrap();
        assert!(buf_i16.as_ref().iter().all(|&s| s == 0));

        // S32 -> expect all zeros
        let mut pw_i32 = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_i32()
            .unwrap();
        pw_i32.set_amplitude(0.0).unwrap();
        pw_i32.seek_to_pcm_frame(0).unwrap();
        let buf_i32 = pw_i32.read_pcm_frames(FRAMES).unwrap();
        assert!(buf_i32.as_ref().iter().all(|&s| s == 0));

        // S24 -> expect all bytes zero
        let mut pw_s24 = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_s24()
            .unwrap();
        pw_s24.set_amplitude(0.0).unwrap();
        pw_s24.seek_to_pcm_frame(0).unwrap();
        let buf_s24 = pw_s24.read_pcm_frames(FRAMES).unwrap();
        assert!(buf_s24.as_ref().iter().all(|&b| b == 0));

        // S24 -> expect all bytes zero
        let mut pw_s24_p = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_s24_packed()
            .unwrap();
        pw_s24_p.set_amplitude(0.0).unwrap();
        pw_s24_p.seek_to_pcm_frame(0).unwrap();
        let buf_s24_p = pw_s24_p.read_pcm_frames(FRAMES).unwrap();
        assert!(buf_s24_p.as_ref().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_pulsewave_set_amplitude_zero_u8_is_constant_midpointish() {
        // For unsigned 8-bit PCM, "silence" is typically centered around 128.
        // We don't assume the exact value here; we only assert it becomes constant.
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_u8()
            .unwrap();

        pw.set_amplitude(0.0).unwrap();
        pw.seek_to_pcm_frame(0).unwrap();
        let buf = pw.read_pcm_frames(FRAMES).unwrap();

        assert!(!buf.as_ref().is_empty());
        let first = buf.as_ref()[0];
        assert!(
            buf.as_ref().iter().all(|&b| b == first),
            "U8 amplitude=0 should produce a constant signal (silence level)"
        );
    }

    #[test]
    fn test_pulsewave_setters_return_ok_f32() {
        let mut pw = PulseWaveBuilder::new(CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_f32()
            .unwrap();

        pw.set_amplitude(0.25).unwrap();
        pw.set_frequency(220.0).unwrap();
        pw.set_duty_cycle(0.1).unwrap();
        pw.set_sample_rate(SampleRate::Sr44100).unwrap();
        pw.seek_to_pcm_frame(0).unwrap();

        let buf = pw.read_pcm_frames(FRAMES).unwrap();
        let frames_read = buf.frames() as u64;
        assert_eq!(frames_read, FRAMES);
    }

    #[test]
    fn test_pulsewave_init_all_sample_rates_f32() {
        for sr in all_sample_rates() {
            let mut pw = PulseWaveBuilder::new(1, sr, 1.0, 440.0, 0.5)
                .build_f32()
                .unwrap();

            let buf = pw.read_pcm_frames(32).unwrap();
            let frames_read = buf.frames() as u64;
            assert_eq!(frames_read, 32);
            assert_eq!(buf.len(), 32);
        }
    }
}
