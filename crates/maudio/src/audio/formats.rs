use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaResult, MaudioError};

/// Sample format (numeric representation of audio samples).
///
/// Each format uses the full dynamic range of its underlying type:
///
/// - **`Format::U8`**  — 8-bit unsigned integer, range `[0, 255]`
/// - **`Format::S16`** — 16-bit signed integer, range `[-32768, 32767]`
/// - **`Format::S24`** — 24-bit signed integer (tightly packed), range `[-8_388_608, 8_388_607]`
/// - **`Format::S32`** — 32-bit signed integer, range `[-2_147_483_648, 2_147_483_647]`
/// - **`Format::F32`** — 32-bit floating point, typically normalized to `[-1.0, 1.0]`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum Format {
    U8,
    S16,
    S24,
    S32,
    F32,
}

/// An owned, interleaved audio sample buffer.
///
/// Stores raw PCM samples for one or more channels, typically returned by
/// audio read and decode operations.
pub struct SampleBuffer<T> {
    data: Vec<T>,
    channels: u32,
    // sample_rate: SampleRate,
}

impl<T> AsRef<[T]> for SampleBuffer<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> AsMut<[T]> for SampleBuffer<T> {
    fn as_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T> SampleBuffer<T> {
    pub(crate) fn new(data: Vec<T>, channels: u32) -> Self {
        SampleBuffer { data, channels }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn len_samples(&self) -> usize {
        self.data.len()
    }

    pub fn channels(&self) -> u32 {
        self.channels
    }

    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            return 0;
        }
        self.data.len() / self.channels as usize
    }

    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut core::ffi::c_void {
        self.data.as_mut_ptr() as *mut core::ffi::c_void
    }

    pub(crate) fn truncate_to_frames(&mut self, frames_read: usize) {
        self.data.truncate(frames_read * self.channels as usize)
    }
}

/// An owned interleaved audio buffer containing 24-bit PCM samples.
///
/// Samples are stored as tightly packed 3-byte values in a `u8` buffer.
pub struct SampleBufferS24 {
    data: Vec<u8>, // len == frames * channels * 3
    channels: u32,
}

impl SampleBufferS24 {
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn len_samples(&self) -> usize {
        self.data.len()
    }

    pub fn channels(&self) -> u32 {
        self.channels
    }

    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            return 0;
        }
        self.data.len() / self.channels as usize
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut core::ffi::c_void {
        self.data.as_mut_ptr() as *mut core::ffi::c_void
    }

    pub(crate) fn truncate_to_frames(&mut self, frames_read: usize) {
        self.data.truncate(frames_read * self.channels as usize * 3)
    }
}

impl AsRef<[u8]> for SampleBufferS24 {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl AsMut<[u8]> for SampleBufferS24 {
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

impl Format {
    // pub(crate) fn new_generic<F: PcmFormat>(
    //     &self,
    //     channels: u32,
    //     frames: u64,
    // ) -> MaResult<SampleBuffer<<F as PcmFormat>::StorageUnit>> {
    //     // let len = frames
    //     //     .checked_mul(channels as u64)
    //     //     .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
    //     //         op: "frames * channels",
    //     //         lhs: frames,
    //     //         rhs: channels as u64,
    //     //     }))?;

    // // if matches!(F, S24Packed) {
    // //     let _ = frames
    // //         .checked_mul(3)
    // //         .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
    // //             op: "S24 samples",
    // //             lhs: len,
    // //             rhs: 3,
    // //         }))?;
    // // }

    //     Ok(F::new_zeroed(frames, channels))
    // }

    pub(crate) fn new_u8(&self, channels: u32, frames: u64) -> MaResult<SampleBuffer<u8>> {
        debug_assert!(
            matches!(self, Format::U8),
            "Format::new_u8 called on {self:?}"
        );
        if !matches!(self, Format::U8) {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidFormat));
        }

        let len = frames
            .checked_mul(channels as u64)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::InvalidFormat))?;

        Ok(SampleBuffer {
            data: vec![0u8; len as usize],
            channels,
        })
    }

    pub(crate) fn new_s16(&self, channels: u32, frames: u64) -> MaResult<SampleBuffer<i16>> {
        debug_assert!(
            matches!(self, Format::S16),
            "Format::new_s16 called on {self:?}"
        );
        if !matches!(self, Format::S16) {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidFormat));
        }

        let len = frames
            .checked_mul(channels as u64)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::InvalidFormat))?;

        Ok(SampleBuffer {
            data: vec![0i16; len as usize],
            channels,
        })
    }

    pub(crate) fn new_s32(&self, channels: u32, frames: u64) -> MaResult<SampleBuffer<i32>> {
        debug_assert!(
            matches!(self, Format::S32),
            "Format::new_s32 called on {self:?}"
        );
        if !matches!(self, Format::S32) {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidFormat));
        }

        let len = frames
            .checked_mul(channels as u64)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::InvalidFormat))?;

        Ok(SampleBuffer {
            data: vec![0i32; len as usize],
            channels,
        })
    }

    pub(crate) fn new_f32(&self, channels: u32, frames: u64) -> MaResult<SampleBuffer<f32>> {
        debug_assert!(
            matches!(self, Format::F32),
            "Format::new_f32 called on {self:?}"
        );
        if !matches!(self, Format::F32) {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidFormat));
        }

        let len = frames
            .checked_mul(channels as u64)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::InvalidFormat))?;

        Ok(SampleBuffer {
            data: vec![0.0f32; len as usize],
            channels,
        })
    }

    pub(crate) fn new_s24(&self, channels: u32, frames: u64) -> MaResult<SampleBufferS24> {
        debug_assert!(
            matches!(self, Format::S24),
            "Format::new_s24 called on {self:?}"
        );
        if !matches!(self, Format::S24) {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidFormat));
        }

        let len = frames
            .checked_mul(channels as u64)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::InvalidFormat))?
            .checked_mul(3)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::InvalidFormat))?;

        Ok(SampleBufferS24 {
            data: vec![0u8; len as usize],
            channels,
        })
    }
}

impl From<Format> for sys::ma_format {
    fn from(value: Format) -> Self {
        match value {
            Format::U8 => sys::ma_format_ma_format_u8,
            Format::S16 => sys::ma_format_ma_format_s16,
            Format::S24 => sys::ma_format_ma_format_s24,
            Format::S32 => sys::ma_format_ma_format_s32,
            Format::F32 => sys::ma_format_ma_format_f32,
        }
    }
}

impl TryFrom<sys::ma_format> for Format {
    type Error = MaudioError;
    fn try_from(value: sys::ma_format) -> Result<Self, Self::Error> {
        match value {
            sys::ma_format_ma_format_u8 => Ok(Format::U8),
            sys::ma_format_ma_format_s16 => Ok(Format::S16),
            sys::ma_format_ma_format_s24 => Ok(Format::S24),
            sys::ma_format_ma_format_s32 => Ok(Format::S32),
            sys::ma_format_ma_format_f32 => Ok(Format::F32),
            _ => Err(MaudioError::new_ma_error(ErrorKinds::InvalidFormat)),
        }
    }
}

/// Controls dithering applied during sample format conversion.
///
/// Dithering is used to reduce quantization distortion when converting
/// from a higher-precision format to a lower-precision one. The selected
/// mode is a **hint** — dithering is only applied when it is meaningful
/// for the conversion.
///
/// ### Modes (ordered by efficiency)
///
/// - **`Dither::None`**  
///   No dithering.
///
/// - **`Dither::Rectangle`**  
///   Rectangular probability distribution function (RPDF).
///
/// - **`Dither::Triangle`**  
///   Triangular probability distribution function (TPDF).
///
/// ### When dithering is applied
///
/// Dithering is currently used for the following format conversions:
///
/// - `S16 → U8`
/// - `S24 → U8`
/// - `S32 → U8`
/// - `F32 → U8`
/// - `S24 → S16`
/// - `S32 → S16`
/// - `F32 → S16`
///
/// For conversions where dithering is unnecessary, the selected mode is
/// silently ignored. Passing a dithering mode other than `None` in these
/// cases is **not** an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum Dither {
    None,
    Rectangle,
    Triangle,
}

impl From<Dither> for sys::ma_dither_mode {
    fn from(value: Dither) -> Self {
        match value {
            Dither::None => sys::ma_dither_mode_ma_dither_mode_none,
            Dither::Rectangle => sys::ma_dither_mode_ma_dither_mode_rectangle,
            Dither::Triangle => sys::ma_dither_mode_ma_dither_mode_triangle,
        }
    }
}

impl TryFrom<sys::ma_dither_mode> for Dither {
    type Error = MaudioError;

    fn try_from(value: sys::ma_dither_mode) -> Result<Self, Self::Error> {
        match value {
            sys::ma_dither_mode_ma_dither_mode_none => Ok(Dither::None),
            sys::ma_dither_mode_ma_dither_mode_rectangle => Ok(Dither::Rectangle),
            sys::ma_dither_mode_ma_dither_mode_triangle => Ok(Dither::Triangle),
            other => Err(MaudioError::new_ma_error(
                ErrorKinds::unknown_enum::<Dither>(other as i64),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::MaError;

    use super::*;
    use maudio_sys::ffi as sys;

    fn invalid_args() -> MaError {
        MaError(sys::ma_result_MA_ERROR)
    }

    #[test]
    fn test_formats_format_into_sys_matches_expected_constants() {
        assert_eq!(
            sys::ma_format::from(Format::U8),
            sys::ma_format_ma_format_u8
        );
        assert_eq!(
            sys::ma_format::from(Format::S16),
            sys::ma_format_ma_format_s16
        );
        assert_eq!(
            sys::ma_format::from(Format::S24),
            sys::ma_format_ma_format_s24
        );
        assert_eq!(
            sys::ma_format::from(Format::S32),
            sys::ma_format_ma_format_s32
        );
        assert_eq!(
            sys::ma_format::from(Format::F32),
            sys::ma_format_ma_format_f32
        );
    }

    #[test]
    fn test_formats_format_try_from_sys_accepts_known_values() {
        assert_eq!(
            Format::try_from(sys::ma_format_ma_format_u8).unwrap(),
            Format::U8
        );
        assert_eq!(
            Format::try_from(sys::ma_format_ma_format_s16).unwrap(),
            Format::S16
        );
        assert_eq!(
            Format::try_from(sys::ma_format_ma_format_s24).unwrap(),
            Format::S24
        );
        assert_eq!(
            Format::try_from(sys::ma_format_ma_format_s32).unwrap(),
            Format::S32
        );
        assert_eq!(
            Format::try_from(sys::ma_format_ma_format_f32).unwrap(),
            Format::F32
        );
    }

    #[test]
    fn test_formats_format_try_from_sys_rejects_unknown_values() {
        // Pick a value that is not mapped by TryFrom (Count is intentionally rejected too).
        let err = Format::try_from(sys::ma_format_ma_format_count).unwrap_err();
        assert_eq!(err, invalid_args());

        // Also test a totally bogus value.
        let bogus = sys::ma_format_ma_format_f32 + 999;
        let err = Format::try_from(bogus).unwrap_err();
        assert_eq!(err, invalid_args());
    }

    #[test]
    fn test_formats_format_roundtrip_sys_to_rust_to_sys_for_supported_variants() {
        let cases = [
            sys::ma_format_ma_format_u8,
            sys::ma_format_ma_format_s16,
            sys::ma_format_ma_format_s24,
            sys::ma_format_ma_format_s32,
            sys::ma_format_ma_format_f32,
        ];

        for &v in &cases {
            let rust = Format::try_from(v).unwrap();
            let back = sys::ma_format::from(rust);
            assert_eq!(back, v);
        }
    }

    #[test]
    fn test_formats_dither_into_sys_matches_expected_constants() {
        assert_eq!(
            <sys::ma_dither_mode as From<Dither>>::from(Dither::None),
            sys::ma_dither_mode_ma_dither_mode_none
        );
        assert_eq!(
            <sys::ma_dither_mode as From<Dither>>::from(Dither::Rectangle),
            sys::ma_dither_mode_ma_dither_mode_rectangle
        );
        assert_eq!(
            <sys::ma_dither_mode as From<Dither>>::from(Dither::Triangle),
            sys::ma_dither_mode_ma_dither_mode_triangle
        );
    }

    #[test]
    fn test_formats_dither_try_from_sys_accepts_known_values() {
        assert_eq!(
            Dither::try_from(sys::ma_dither_mode_ma_dither_mode_none).unwrap(),
            Dither::None
        );
        assert_eq!(
            Dither::try_from(sys::ma_dither_mode_ma_dither_mode_rectangle).unwrap(),
            Dither::Rectangle
        );
        assert_eq!(
            Dither::try_from(sys::ma_dither_mode_ma_dither_mode_triangle).unwrap(),
            Dither::Triangle
        );
    }

    #[test]
    fn test_formats_dither_try_from_sys_rejects_unknown_values() {
        let bogus = sys::ma_dither_mode_ma_dither_mode_triangle + 123;
        let err = Dither::try_from(bogus).unwrap_err();
        assert_eq!(err, invalid_args());
    }

    #[test]
    fn test_formats_dither_roundtrip_sys_to_rust_to_sys_for_supported_variants() {
        let cases = [
            sys::ma_dither_mode_ma_dither_mode_none,
            sys::ma_dither_mode_ma_dither_mode_rectangle,
            sys::ma_dither_mode_ma_dither_mode_triangle,
        ];

        for &v in &cases {
            let rust = Dither::try_from(v).unwrap();

            let back = <sys::ma_dither_mode as From<Dither>>::from(rust);
            assert_eq!(back, v);
        }
    }
}
