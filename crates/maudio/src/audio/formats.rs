use std::marker::PhantomData;

use maudio_sys::ffi as sys;

use crate::{
    pcm_frames::{PcmFormat, PcmFormatInternal},
    ErrorKinds, MaResult, MaudioError,
};

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

// Always holds the sample format used by the user
/// An owned, interleaved audio sample buffer.
///
/// Stores raw PCM samples for one or more channels, typically returned by
/// audio read and decode operations.
pub struct SampleBuffer<F: PcmFormat> {
    pub data: Vec<F::PcmUnit>,
    channels: u32,
    frames: usize,
    _pcm_format: PhantomData<F>,
}

impl<F: PcmFormat> AsRef<[F::PcmUnit]> for SampleBuffer<F> {
    fn as_ref(&self) -> &[F::PcmUnit] {
        self.as_slice()
    }
}

impl<F: PcmFormat> AsMut<[F::PcmUnit]> for SampleBuffer<F> {
    fn as_mut(&mut self) -> &mut [F::PcmUnit] {
        self.as_mut_slice()
    }
}

impl<F: PcmFormat> SampleBuffer<F> {
    pub(crate) fn new(data: Vec<F::PcmUnit>, channels: u32, frames: usize) -> SampleBuffer<F> {
        Self {
            data,
            channels,
            frames,
            _pcm_format: PhantomData,
        }
    }

    pub(crate) fn required_len(frames: usize, channels: u32, vec_unit: usize) -> MaResult<usize> {
        let ch = channels as usize;
        let samples = frames.checked_mul(ch).ok_or(MaudioError::new_ma_error(
            ErrorKinds::IntegerOverflow {
                op: "frames * channels",
                lhs: frames as u64,
                rhs: channels as u64,
            },
        ))?;
        let len = samples
            .checked_mul(vec_unit)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                op: "frames to storage",
                lhs: samples as u64,
                rhs: channels as u64,
            }))?;
        Ok(len)
    }

    pub(crate) fn new_zeroed(frames: usize, channels: u32) -> MaResult<Vec<F::StorageUnit>> {
        let len = Self::required_len(frames, channels, F::VEC_PCM_UNITS_PER_FRAME)?;
        Ok(vec![F::StorageUnit::default(); len])
    }

    /// Takes a `Vec<F::StorageUnit>` and returns a SampleBuffer (with PcmUnit)
    ///
    /// Performs any conversion necessary and truncates to frames read
    pub(crate) fn from_storage(
        mut storage: Vec<F::StorageUnit>,
        frames_read: usize,
        channels: u32,
    ) -> MaResult<SampleBuffer<F>> {
        let len = frames_read
            .checked_mul(channels as usize)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                op: "truncate: frames * channels",
                lhs: frames_read as u64,
                rhs: channels as u64,
            }))?;

        let vec_el =
            len.checked_mul(F::VEC_STORE_UNITS_PER_FRAME)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "truncate: frames to buf units",
                    lhs: len as u64,
                    rhs: F::VEC_STORE_UNITS_PER_FRAME as u64,
                }))?;

        // Convert from Storage to Pcm
        storage.truncate(vec_el);

        let data = <F as PcmFormatInternal>::storage_to_pcm_internal(storage)?;

        Ok(SampleBuffer {
            data,
            channels,
            frames: frames_read,
            _pcm_format: PhantomData,
        })
    }

    pub fn channels(&self) -> u32 {
        self.channels
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the len of the underlying Vec[T]. `Not` the frames count.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    fn as_slice(&self) -> &[F::PcmUnit] {
        &self.data
    }

    fn as_mut_slice(&mut self) -> &mut [F::PcmUnit] {
        &mut self.data
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut core::ffi::c_void {
        self.data.as_mut_ptr() as *mut core::ffi::c_void
    }

    /// items_per_frame is either F::VEC_STORE_UNITS_PER_FRAME or F::VEC_PCM_UNITS_PER_FRAME
    pub(crate) fn truncate_to_frames(&mut self, frames: usize) -> MaResult<()> {
        let len = frames
            .checked_mul(self.channels as usize)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                op: "truncate: frames * channels",
                lhs: frames as u64,
                rhs: self.channels as u64,
            }))?;

        let vec_el =
            len.checked_mul(F::VEC_PCM_UNITS_PER_FRAME)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "truncate: frames * channels",
                    lhs: len as u64,
                    rhs: F::VEC_PCM_UNITS_PER_FRAME as u64,
                }))?;

        self.data.truncate(vec_el);
        Ok(())
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
