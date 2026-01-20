use maudio_sys::ffi as sys;

use crate::{
    Binding, MaError, MaResult, MaudioError,
    audio::{formats::Format, sample_rate::SampleRate},
};

pub trait AsPulseWavePtr {
    fn as_pulsewave_ptr(&self) -> *mut sys::ma_pulsewave;
    fn channels(&self) -> u32;
}

pub(crate) struct PulseWaveInner {
    ptr: *mut sys::ma_pulsewave,
}

#[derive(Debug)]
pub(crate) struct PulseWaveState {
    channels: u32,
    sample_rate: SampleRate,
    amplitude: f64,
    frequency: f64,
    duty_cycle: f64,
}

pub struct PulseWaveU8 {
    inner: PulseWaveInner,
    format: Format,
    state: PulseWaveState,
}

impl AsPulseWavePtr for PulseWaveU8 {
    fn as_pulsewave_ptr(&self) -> *mut sys::ma_pulsewave {
        self.inner.ptr
    }
    fn channels(&self) -> u32 {
        self.state.channels
    }
}

impl PulseWaveU8 {
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<u8>, u64)> {
        pulsewave_ffi::ma_pulsewave_read_pcm_frames_u8(self, frame_count)
    }
}

pub struct PulseWaveI16 {
    inner: PulseWaveInner,
    format: Format,
    state: PulseWaveState,
}
impl AsPulseWavePtr for PulseWaveI16 {
    fn as_pulsewave_ptr(&self) -> *mut sys::ma_pulsewave {
        self.inner.ptr
    }
    fn channels(&self) -> u32 {
        self.state.channels
    }
}

impl PulseWaveI16 {
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<i16>, u64)> {
        pulsewave_ffi::ma_pulsewave_read_pcm_frames_i16(self, frame_count)
    }
}

pub struct PulseWaveI32 {
    inner: PulseWaveInner,
    format: Format,
    state: PulseWaveState,
}
impl AsPulseWavePtr for PulseWaveI32 {
    fn as_pulsewave_ptr(&self) -> *mut sys::ma_pulsewave {
        self.inner.ptr
    }
    fn channels(&self) -> u32 {
        self.state.channels
    }
}

impl PulseWaveI32 {
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<i32>, u64)> {
        pulsewave_ffi::ma_pulsewave_read_pcm_frames_i32(self, frame_count)
    }
}

pub struct PulseWaveS24 {
    inner: PulseWaveInner,
    format: Format,
    state: PulseWaveState,
}

impl AsPulseWavePtr for PulseWaveS24 {
    fn as_pulsewave_ptr(&self) -> *mut sys::ma_pulsewave {
        self.inner.ptr
    }
    fn channels(&self) -> u32 {
        self.state.channels
    }
}

impl PulseWaveS24 {
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<u8>, u64)> {
        pulsewave_ffi::ma_pulsewave_read_pcm_frames_s24(self, frame_count)
    }
}

pub struct PulseWaveF32 {
    inner: PulseWaveInner,
    format: Format,
    state: PulseWaveState,
}

impl AsPulseWavePtr for PulseWaveF32 {
    fn as_pulsewave_ptr(&self) -> *mut sys::ma_pulsewave {
        self.inner.ptr
    }
    fn channels(&self) -> u32 {
        self.state.channels
    }
}

impl PulseWaveF32 {
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<(Vec<f32>, u64)> {
        pulsewave_ffi::ma_pulsewave_read_pcm_frames_f32(self, frame_count)
    }
}

impl<T: AsPulseWavePtr + ?Sized> PulseWaveOps for T {}

pub trait PulseWaveOps: AsPulseWavePtr {
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
}

pub(crate) mod pulsewave_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        Binding, MaRawResult, MaResult,
        audio::sample_rate::SampleRate,
        data_source::sources::pulsewave::{AsPulseWavePtr, PulseWaveBuilder, PulseWaveInner},
    };

    #[inline]
    pub fn ma_pulsewave_init(
        config: &PulseWaveBuilder,
        pulsewave: *mut sys::ma_pulsewave,
    ) -> MaResult<()> {
        let raw = config.to_raw();
        let res = unsafe { sys::ma_pulsewave_init(&raw as *const _, pulsewave) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_pulsewave_uninit(pulsewave: &mut PulseWaveInner) {
        unsafe { sys::ma_pulsewave_uninit(pulsewave.ptr) }
    }

    pub fn ma_pulsewave_read_pcm_frames_u8<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frame_count: u64,
    ) -> MaResult<(Vec<u8>, u64)> {
        let mut buffer = vec![0u8; (frame_count * pw.channels() as u64) as usize];
        let frames_read = ma_pulsewave_read_pcm_frames_internal(
            pw,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;
        buffer.truncate((frames_read * pw.channels() as u64) as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_pulsewave_read_pcm_frames_i16<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frame_count: u64,
    ) -> MaResult<(Vec<i16>, u64)> {
        let mut buffer = vec![0i16; (frame_count * pw.channels() as u64) as usize];
        let frames_read = ma_pulsewave_read_pcm_frames_internal(
            pw,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;
        buffer.truncate((frames_read * pw.channels() as u64) as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_pulsewave_read_pcm_frames_i32<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frame_count: u64,
    ) -> MaResult<(Vec<i32>, u64)> {
        let mut buffer = vec![0i32; (frame_count * pw.channels() as u64) as usize];
        let frames_read = ma_pulsewave_read_pcm_frames_internal(
            pw,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;
        buffer.truncate((frames_read * pw.channels() as u64) as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_pulsewave_read_pcm_frames_s24<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frame_count: u64,
    ) -> MaResult<(Vec<u8>, u64)> {
        let mut buffer = vec![0u8; (frame_count * pw.channels() as u64 * 3) as usize];
        let frames_read = ma_pulsewave_read_pcm_frames_internal(
            pw,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;
        buffer.truncate((frames_read * pw.channels() as u64 * 3) as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_pulsewave_read_pcm_frames_f32<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frame_count: u64,
    ) -> MaResult<(Vec<f32>, u64)> {
        let mut buffer = vec![0.0f32; (frame_count * pw.channels() as u64) as usize];
        let frames_read = ma_pulsewave_read_pcm_frames_internal(
            pw,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;
        buffer.truncate((frames_read * pw.channels() as u64) as usize);
        Ok((buffer, frames_read))
    }

    pub fn ma_pulsewave_read_pcm_frames_internal<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_pulsewave_read_pcm_frames(
                pw.as_pulsewave_ptr(),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaRawResult::check(res)?;
        Ok(frames_read)
    }

    #[inline]
    pub fn ma_pulsewave_seek_to_pcm_frame<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frame_index: u64,
    ) -> MaResult<()> {
        let res =
            unsafe { sys::ma_pulsewave_seek_to_pcm_frame(pw.as_pulsewave_ptr(), frame_index) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_pulsewave_set_amplitude<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        amplitude: f64,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_pulsewave_set_amplitude(pw.as_pulsewave_ptr(), amplitude) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_pulsewave_set_frequency<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        frequency: f64,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_pulsewave_set_frequency(pw.as_pulsewave_ptr(), frequency) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_pulsewave_set_duty_cycle<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        duty_cycle: f64,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_pulsewave_set_duty_cycle(pw.as_pulsewave_ptr(), duty_cycle) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_pulsewave_set_sample_rate<W: AsPulseWavePtr + ?Sized>(
        pw: &mut W,
        sample_rate: SampleRate,
    ) -> MaResult<()> {
        let res =
            unsafe { sys::ma_pulsewave_set_sample_rate(pw.as_pulsewave_ptr(), sample_rate.into()) };
        MaRawResult::check(res)
    }
}

impl Drop for PulseWaveInner {
    fn drop(&mut self) {
        pulsewave_ffi::ma_pulsewave_uninit(self);
        drop(unsafe { Box::from_raw(self.ptr) })
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
        format: Format,
        channels: u32,
        sample_rate: SampleRate,
        amplitude: f64,
        frequency: f64,
        duty_cycle: f64,
    ) -> Self {
        let cfg = unsafe {
            sys::ma_pulsewave_config_init(
                format.into(),
                channels,
                sample_rate.into(),
                amplitude,
                frequency,
                duty_cycle,
            )
        };
        Self {
            inner: cfg,
            format,
            channels,
            sample_rate,
            amplitude,
            frequency,
            duty_cycle,
        }
    }

    pub fn amplitude(mut self, a: f64) -> Self {
        self.inner.amplitude = a;
        self.amplitude = a;
        self
    }

    pub fn frequency(mut self, f: f64) -> Self {
        self.inner.frequency = f;
        self.frequency = f;
        self
    }

    pub fn duty_cycle(mut self, d: f64) -> Self {
        self.inner.dutyCycle = d;
        self.duty_cycle = d;
        self
    }

    pub fn channels(mut self, c: u32) -> Self {
        self.inner.channels = c;
        self.channels = c;
        self
    }

    pub fn build_u8(self) -> MaResult<PulseWaveU8> {
        debug_assert!(
            matches!(self.format, Format::U8),
            "Cannot build PulseWaveU8 from {:?}",
            self.format
        );
        if !matches!(self.format, Format::U8) {
            return Err(MaudioError::from_ma_result(MaError(
                sys::ma_result_MA_INVALID_ARGS,
            )));
        }
        let inner = self.new_inner()?;
        let state = PulseWaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            amplitude: self.amplitude,
            frequency: self.frequency,
            duty_cycle: self.duty_cycle,
        };

        Ok(PulseWaveU8 {
            inner,
            format: Format::U8,
            state,
        })
    }

    pub fn build_i16(self) -> MaResult<PulseWaveI16> {
        debug_assert!(
            matches!(self.format, Format::S16),
            "Cannot build PulseWaveI16 from {:?}",
            self.format
        );
        if !matches!(self.format, Format::S16) {
            return Err(MaudioError::from_ma_result(MaError(
                sys::ma_result_MA_INVALID_ARGS,
            )));
        }
        let inner = self.new_inner()?;
        let state = PulseWaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            amplitude: self.amplitude,
            frequency: self.frequency,
            duty_cycle: self.duty_cycle,
        };

        Ok(PulseWaveI16 {
            inner,
            format: Format::S16,
            state,
        })
    }

    pub fn build_i32(self) -> MaResult<PulseWaveI32> {
        debug_assert!(
            matches!(self.format, Format::S32),
            "Cannot build PulseWaveI32 from {:?}",
            self.format
        );
        if !matches!(self.format, Format::S32) {
            return Err(MaudioError::from_ma_result(MaError(
                sys::ma_result_MA_INVALID_ARGS,
            )));
        }
        let inner = self.new_inner()?;
        let state = PulseWaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            amplitude: self.amplitude,
            frequency: self.frequency,
            duty_cycle: self.duty_cycle,
        };

        Ok(PulseWaveI32 {
            inner,
            format: Format::S32,
            state,
        })
    }

    pub fn build_s24(self) -> MaResult<PulseWaveS24> {
        debug_assert!(
            matches!(self.format, Format::S24),
            "Cannot build PulseWaveS24 from {:?}",
            self.format
        );
        if !matches!(self.format, Format::S24) {
            return Err(MaudioError::from_ma_result(MaError(
                sys::ma_result_MA_INVALID_ARGS,
            )));
        }
        let inner = self.new_inner()?;
        let state = PulseWaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            amplitude: self.amplitude,
            frequency: self.frequency,
            duty_cycle: self.duty_cycle,
        };

        Ok(PulseWaveS24 {
            inner,
            format: Format::S24,
            state,
        })
    }

    pub fn build_f32(self) -> MaResult<PulseWaveF32> {
        debug_assert!(
            matches!(self.format, Format::F32),
            "Cannot build PulseWaveF32 from {:?}",
            self.format
        );
        if !matches!(self.format, Format::F32) {
            return Err(MaudioError::from_ma_result(MaError(
                sys::ma_result_MA_INVALID_ARGS,
            )));
        }
        let inner = self.new_inner()?;
        let state = PulseWaveState {
            channels: self.channels,
            sample_rate: self.sample_rate,
            amplitude: self.amplitude,
            frequency: self.frequency,
            duty_cycle: self.duty_cycle,
        };

        Ok(PulseWaveF32 {
            inner,
            format: Format::F32,
            state,
        })
    }

    fn new_inner(&self) -> MaResult<PulseWaveInner> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_pulsewave>> = Box::new_uninit();

        pulsewave_ffi::ma_pulsewave_init(self, mem.as_mut_ptr())?;

        let ptr: Box<sys::ma_pulsewave> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_pulsewave = Box::into_raw(ptr);
        Ok(PulseWaveInner { ptr: inner })
    }
}

#[cfg(test)]
mod test {
    use crate::{
        audio::{formats::Format, sample_rate::SampleRate},
        data_source::sources::pulsewave::{PulseWaveBuilder, PulseWaveOps},
    };

    #[test]
    fn test_pulsewave_basic_init() {
        let _pw = PulseWaveBuilder::new(Format::F32, 2, SampleRate::Sr48000, 1.0, 440.0, 1.0)
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

    fn assert_frames_and_len_s24(buf: &[u8], frames_read: u64, channels: u32) {
        assert_eq!(frames_read, FRAMES);
        assert_eq!(buf.len(), (frames_read * channels as u64 * 3) as usize);
    }

    fn assert_frames_and_len_f32(buf: &[f32], frames_read: u64, channels: u32) {
        assert_eq!(frames_read, FRAMES);
        assert_eq!(buf.len(), (frames_read * channels as u64) as usize);
    }

    #[test]
    fn test_pulsewave_basic_init_f32() {
        let _pw = PulseWaveBuilder::new(Format::F32, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_f32()
            .unwrap();
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn test_pulsewave_build_mismatch_errors() {
        // Each build_* should reject mismatched Format with MA_INVALID_ARGS mapped into Err.
        assert!(
            PulseWaveBuilder::new(Format::F32, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
                .build_u8()
                .is_err()
        );

        assert!(
            PulseWaveBuilder::new(Format::U8, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
                .build_i16()
                .is_err()
        );

        assert!(
            PulseWaveBuilder::new(Format::S16, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
                .build_i32()
                .is_err()
        );

        assert!(
            PulseWaveBuilder::new(Format::S32, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
                .build_s24()
                .is_err()
        );

        assert!(
            PulseWaveBuilder::new(Format::S24, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
                .build_f32()
                .is_err()
        );
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_u8_sizing() {
        let mut pw = PulseWaveBuilder::new(Format::U8, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_u8()
            .unwrap();

        let (buf, frames_read) = pw.read_pcm_frames(FRAMES).unwrap();
        assert_frames_and_len_u8(&buf, frames_read, CH);
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_i16_sizing() {
        let mut pw = PulseWaveBuilder::new(Format::S16, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_i16()
            .unwrap();

        let (buf, frames_read) = pw.read_pcm_frames(FRAMES).unwrap();
        assert_frames_and_len_i16(&buf, frames_read, CH);
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_i32_sizing() {
        let mut pw = PulseWaveBuilder::new(Format::S32, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_i32()
            .unwrap();

        let (buf, frames_read) = pw.read_pcm_frames(FRAMES).unwrap();
        assert_frames_and_len_i32(&buf, frames_read, CH);
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_s24_sizing() {
        let mut pw = PulseWaveBuilder::new(Format::S24, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_s24()
            .unwrap();

        let (buf, frames_read) = pw.read_pcm_frames(FRAMES).unwrap();
        assert_frames_and_len_s24(&buf, frames_read, CH);
    }

    #[test]
    fn test_pulsewave_read_pcm_frames_f32_sizing() {
        let mut pw = PulseWaveBuilder::new(Format::F32, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_f32()
            .unwrap();

        let (buf, frames_read) = pw.read_pcm_frames(FRAMES).unwrap();
        assert_frames_and_len_f32(&buf, frames_read, CH);
    }

    #[test]
    fn test_pulsewave_seek_is_deterministic_f32() {
        let mut pw = PulseWaveBuilder::new(Format::F32, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_f32()
            .unwrap();

        pw.seek_to_pcm_frame(0).unwrap();
        let (a, fa) = pw.read_pcm_frames(FRAMES).unwrap();
        assert_frames_and_len_f32(&a, fa, CH);

        pw.seek_to_pcm_frame(0).unwrap();
        let (b, fb) = pw.read_pcm_frames(FRAMES).unwrap();
        assert_frames_and_len_f32(&b, fb, CH);

        assert_eq!(a, b, "Expected identical output after seek_to_pcm_frame(0)");
    }

    #[test]
    fn test_pulsewave_set_frequency_changes_output_f32() {
        let mut pw = PulseWaveBuilder::new(Format::F32, CH, SampleRate::Sr48000, 0.8, 400.0, 0.5)
            .build_f32()
            .unwrap();

        pw.seek_to_pcm_frame(0).unwrap();
        let (a, _) = pw.read_pcm_frames(FRAMES).unwrap();

        pw.set_frequency(440.0).unwrap();
        pw.seek_to_pcm_frame(0).unwrap();
        let (b, _) = pw.read_pcm_frames(FRAMES).unwrap();

        assert_ne!(a, b, "Changing frequency should change generated samples");
    }

    #[test]
    fn test_pulsewave_set_amplitude_zero_silences_signed_and_float() {
        // F32 -> expect all zeros
        let mut pw_f32 =
            PulseWaveBuilder::new(Format::F32, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
                .build_f32()
                .unwrap();
        pw_f32.set_amplitude(0.0).unwrap();
        pw_f32.seek_to_pcm_frame(0).unwrap();
        let (buf_f32, _) = pw_f32.read_pcm_frames(FRAMES).unwrap();
        assert!(buf_f32.iter().all(|&s| s == 0.0));

        // S16 -> expect all zeros
        let mut pw_i16 =
            PulseWaveBuilder::new(Format::S16, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
                .build_i16()
                .unwrap();
        pw_i16.set_amplitude(0.0).unwrap();
        pw_i16.seek_to_pcm_frame(0).unwrap();
        let (buf_i16, _) = pw_i16.read_pcm_frames(FRAMES).unwrap();
        assert!(buf_i16.iter().all(|&s| s == 0));

        // S32 -> expect all zeros
        let mut pw_i32 =
            PulseWaveBuilder::new(Format::S32, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
                .build_i32()
                .unwrap();
        pw_i32.set_amplitude(0.0).unwrap();
        pw_i32.seek_to_pcm_frame(0).unwrap();
        let (buf_i32, _) = pw_i32.read_pcm_frames(FRAMES).unwrap();
        assert!(buf_i32.iter().all(|&s| s == 0));

        // S24 -> expect all bytes zero
        let mut pw_s24 =
            PulseWaveBuilder::new(Format::S24, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
                .build_s24()
                .unwrap();
        pw_s24.set_amplitude(0.0).unwrap();
        pw_s24.seek_to_pcm_frame(0).unwrap();
        let (buf_s24, _) = pw_s24.read_pcm_frames(FRAMES).unwrap();
        assert!(buf_s24.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_pulsewave_set_amplitude_zero_u8_is_constant_midpointish() {
        // For unsigned 8-bit PCM, "silence" is typically centered around 128.
        // We don't assume the exact value here; we only assert it becomes constant.
        let mut pw = PulseWaveBuilder::new(Format::U8, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_u8()
            .unwrap();

        pw.set_amplitude(0.0).unwrap();
        pw.seek_to_pcm_frame(0).unwrap();
        let (buf, _) = pw.read_pcm_frames(FRAMES).unwrap();

        assert!(!buf.is_empty());
        let first = buf[0];
        assert!(
            buf.iter().all(|&b| b == first),
            "U8 amplitude=0 should produce a constant signal (silence level)"
        );
    }

    #[test]
    fn test_pulsewave_setters_return_ok_f32() {
        let mut pw = PulseWaveBuilder::new(Format::F32, CH, SampleRate::Sr48000, 1.0, 440.0, 0.5)
            .build_f32()
            .unwrap();

        pw.set_amplitude(0.25).unwrap();
        pw.set_frequency(220.0).unwrap();
        pw.set_duty_cycle(0.1).unwrap();
        pw.set_sample_rate(SampleRate::Sr44100).unwrap();
        pw.seek_to_pcm_frame(0).unwrap();

        let (_buf, frames_read) = pw.read_pcm_frames(FRAMES).unwrap();
        assert_eq!(frames_read, FRAMES);
    }

    #[test]
    fn test_pulsewave_init_all_sample_rates_f32() {
        for sr in all_sample_rates() {
            let mut pw = PulseWaveBuilder::new(Format::F32, 1, sr, 1.0, 440.0, 0.5)
                .build_f32()
                .unwrap();

            let (buf, frames_read) = pw.read_pcm_frames(32).unwrap();
            assert_eq!(frames_read, 32);
            assert_eq!(buf.len(), 32);
        }
    }
}
