use maudio_sys::ffi as sys;

type RmFlagsRaw = sys::ma_resource_manager_flags;

#[repr(transparent)]
#[derive(Debug, PartialEq, Clone, Copy, Hash, Eq)]
pub struct RmFlags(RmFlagsRaw);

impl RmFlags {
    pub const NONE: Self = Self(0);
    pub const NON_BLOCKING: Self =
        Self(sys::ma_resource_manager_flags_MA_RESOURCE_MANAGER_FLAG_NON_BLOCKING);
    pub const NO_THREADING: Self =
        Self(sys::ma_resource_manager_flags_MA_RESOURCE_MANAGER_FLAG_NO_THREADING);

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

    /// Create RmFlags from a u32 bitmask
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits as RmFlagsRaw)
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

impl core::ops::BitOr for RmFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl core::ops::BitOrAssign for RmFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
impl core::ops::BitAnd for RmFlags {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}
impl core::ops::BitAndAssign for RmFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}
impl core::ops::BitXor for RmFlags {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}
impl core::ops::Not for RmFlags {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl From<RmFlags> for u32 {
    #[inline]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::useless_conversion)]
    #[allow(clippy::unnecessary_cast)]
    fn from(v: RmFlags) -> u32 {
        v.0 as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_resource_manager_flags() {
        let mut flag = RmFlags::NONE;
        flag.insert(RmFlags::NON_BLOCKING);
        assert!(flag == RmFlags::NON_BLOCKING);
        flag.remove(RmFlags::NON_BLOCKING);
        assert!(flag == RmFlags::NONE);

        flag.insert(RmFlags::NON_BLOCKING);
        flag.insert(RmFlags::NO_THREADING);
        assert!(flag.contains(RmFlags::NO_THREADING));
        assert!(flag.contains(RmFlags::NON_BLOCKING));
        assert!(flag == (RmFlags::NO_THREADING | RmFlags::NON_BLOCKING));

        flag.remove(RmFlags::NO_THREADING);
        assert!(flag == RmFlags::NON_BLOCKING);
    }
}
