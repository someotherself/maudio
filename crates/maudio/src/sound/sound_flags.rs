//! Flags that control how a sound is initialized
use maudio_sys::ffi as sys;

pub type SoundFlagsRaw = sys::ma_sound_flags;

/// Bitflags controlling how a sound is loaded, initialized, and processed.
///
/// `SoundFlags` is a typed wrapper around miniaudio’s `ma_sound_flags` and is
/// used to configure sound behavior such as streaming, decoding, asynchronous
/// loading, looping, and spatialization.
///
/// Flags can be combined, then passed to sound initialization
/// functions where they are forwarded directly to
/// miniaudio without modification.
#[repr(transparent)]
#[derive(Debug, PartialEq, Clone, Copy, Hash, Eq)]
pub struct SoundFlags(SoundFlagsRaw);

impl SoundFlags {
    /// Default value. No flags.
    ///
    /// When no flags are specified, the sound file is fully loaded into memory as
    /// raw encoded data (for example WAV or MP3 bytes) and is decoded on demand
    /// during playback, rather than being pre-decoded or streamed.
    ///
    /// To instead decode the audio data before storing it in memory, use the `DECODE` flag.
    ///
    /// By default, the sound file is loaded synchronously, meaning playback can only
    /// begin after the entire file has finished loading, which may be slow for large assets.
    ///
    /// Enabling `ASYNC` allows loading and decoding to happen in the background so
    /// playback can start sooner, with audio data becoming available progressively.
    pub const NONE: Self = Self(0);
    /// Resource Manager flag
    ///
    /// Streams the sound from the resource manager instead of fully loading it into memory.
    ///
    /// For large sounds, it's often prohibitive to store the entire file in memory.
    /// To mitigate this, you can instead stream audio data which you can do by specifying this flag.
    /// When streaming, data will be decoded in 1 second pages.
    pub const STREAM: Self = Self(sys::ma_sound_flags_MA_SOUND_FLAG_STREAM);
    /// Resource Manager flag
    ///
    /// Decodes the entire sound into memory up front rather than streaming.
    pub const DECODE: Self = Self(sys::ma_sound_flags_MA_SOUND_FLAG_DECODE);
    /// Resource Manager flag
    ///
    /// Loads the sound asynchronously on a background thread. Will start playing after the sound has had some audio decoded.
    pub const ASYNC: Self = Self(sys::ma_sound_flags_MA_SOUND_FLAG_ASYNC);
    /// Resource Manager flag
    ///
    /// Blocks until asynchronous initialization has completed before returning.
    pub const WAIT_INIT: Self = Self(sys::ma_sound_flags_MA_SOUND_FLAG_WAIT_INIT);
    /// Resource Manager flag
    ///
    /// Indicates that the sound length is unknown (for example, streamed sources).
    pub const UNKNOWN_LENGTH: Self = Self(sys::ma_sound_flags_MA_SOUND_FLAG_UNKNOWN_LENGTH);
    /// Resource Manager flag
    ///
    /// Loops the audio. Same as `Sound::set_looping()`
    pub const LOOPING: Self = Self(sys::ma_sound_flags_MA_SOUND_FLAG_LOOPING);
    /// Resource Manager flag
    ///
    /// Prevents the sound from being automatically attached to the engine’s default sound group.
    ///
    /// This is useful when manually constructing a custom node graph before
    /// attaching the sound to a specific group or endpoint.
    pub const NO_DEFAULT_ATTACHMENT: Self =
        Self(sys::ma_sound_flags_MA_SOUND_FLAG_NO_DEFAULT_ATTACHMENT);
    /// Sound specific flag.
    ///
    /// Disables ma_sound_set_pitch and ma_sound_get_pitch as an optimization.
    pub const NO_PITCH: Self = Self(sys::ma_sound_flags_MA_SOUND_FLAG_NO_PITCH);
    /// Sound specific flag.
    ///
    /// Disables spatialization so the sound is always treated as non-positional.
    pub const NO_SPATIALIZATION: Self = Self(sys::ma_sound_flags_MA_SOUND_FLAG_NO_SPATIALIZATION);

    #[inline]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::useless_conversion)]
    #[allow(clippy::unnecessary_cast)]
    pub fn bits(self) -> u32 {
        self.0 as u32
    }

    /// Set or clear bits
    #[inline]
    pub const fn set(&mut self, other: Self, enabled: bool) {
        if enabled {
            self.0 |= other.0;
        } else {
            self.0 &= !other.0;
        }
    }

    /// Create SoundFlags from a u32 bitmask
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits as SoundFlagsRaw)
    }

    /// Check if all the bits in other are set
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Check if any of the bits in other are set
    #[inline]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    #[inline]
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn insert(&mut self, other: Self) {
        self.0 |= other.0
    }

    #[inline]
    pub(crate) const fn insert_bits(&mut self, other: &Self) {
        self.0 |= other.0
    }

    #[inline]
    pub const fn remove(&mut self, other: Self) {
        self.0 &= !other.0
    }
}

impl core::ops::BitOr for SoundFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl core::ops::BitOrAssign for SoundFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
impl core::ops::BitAnd for SoundFlags {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}
impl core::ops::BitAndAssign for SoundFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}
impl core::ops::BitXor for SoundFlags {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}
impl core::ops::Not for SoundFlags {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

#[cfg(unix)]
impl From<SoundFlags> for u32 {
    #[inline]
    fn from(v: SoundFlags) -> u32 {
        v.0
    }
}

#[cfg(windows)]
impl From<SoundFlags> for i32 {
    #[inline]
    fn from(v: SoundFlags) -> i32 {
        v.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_sound_flags() {
        let mut flag = SoundFlags::NONE;
        flag.insert(SoundFlags::ASYNC);
        assert!(flag == SoundFlags::ASYNC);
        flag.remove(SoundFlags::ASYNC);
        assert!(flag == SoundFlags::NONE);

        flag.insert(SoundFlags::ASYNC);
        flag.insert(SoundFlags::LOOPING);
        assert!(flag == (SoundFlags::ASYNC | SoundFlags::LOOPING));
        assert!(flag.contains(SoundFlags::ASYNC));
        assert!(flag.contains(SoundFlags::LOOPING));
        assert!(!flag.contains(SoundFlags::DECODE));

        flag.remove(SoundFlags::ASYNC);
        assert!(flag == SoundFlags::LOOPING);
    }
}
