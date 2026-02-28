use maudio_sys::ffi as sys;

type RmSoourceFlagsRaw = sys::ma_resource_manager_data_source_flags;

#[repr(transparent)]
#[derive(Debug, PartialEq, Clone, Copy, Hash, Eq)]
pub struct RmSourceFlags(RmSoourceFlagsRaw);

impl RmSourceFlags {
    pub const NONE: Self = Self(0);

    pub const STREAM: Self = Self(
        sys::ma_resource_manager_data_source_flags_MA_RESOURCE_MANAGER_DATA_SOURCE_FLAG_STREAM,
    );

    pub const DECODE: Self = Self(
        sys::ma_resource_manager_data_source_flags_MA_RESOURCE_MANAGER_DATA_SOURCE_FLAG_DECODE,
    );

    pub const ASYNC: Self =
        Self(sys::ma_resource_manager_data_source_flags_MA_RESOURCE_MANAGER_DATA_SOURCE_FLAG_ASYNC);

    pub const WAIT_INIT: Self = Self(
        sys::ma_resource_manager_data_source_flags_MA_RESOURCE_MANAGER_DATA_SOURCE_FLAG_WAIT_INIT,
    );

    pub const UNKOWN_LENGTH: Self = Self(sys::ma_resource_manager_data_source_flags_MA_RESOURCE_MANAGER_DATA_SOURCE_FLAG_UNKNOWN_LENGTH);

    pub const LOOPING: Self = Self(
        sys::ma_resource_manager_data_source_flags_MA_RESOURCE_MANAGER_DATA_SOURCE_FLAG_LOOPING,
    );

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

    /// Create RmSoourceFlags from a u32 bitmask
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits as RmSoourceFlagsRaw)
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

impl core::ops::BitOr for RmSourceFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for RmSourceFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for RmSourceFlags {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl core::ops::BitAndAssign for RmSourceFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl core::ops::BitXor for RmSourceFlags {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl core::ops::Not for RmSourceFlags {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl From<RmSourceFlags> for u32 {
    #[inline]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::useless_conversion)]
    #[allow(clippy::unnecessary_cast)]
    fn from(v: RmSourceFlags) -> u32 {
        v.0 as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_resouce_man_data_source_flags() {
        let mut flag = RmSourceFlags::NONE;
        flag.insert(RmSourceFlags::ASYNC);
        assert!(flag == RmSourceFlags::ASYNC);
        flag.remove(RmSourceFlags::ASYNC);
        assert!(flag == RmSourceFlags::NONE);

        flag.insert(RmSourceFlags::ASYNC);
        flag.insert(RmSourceFlags::LOOPING);
        assert!(flag == (RmSourceFlags::ASYNC | RmSourceFlags::LOOPING));
        assert!(flag.contains(RmSourceFlags::ASYNC));
        assert!(flag.contains(RmSourceFlags::LOOPING));
        assert!(!flag.contains(RmSourceFlags::DECODE));

        flag.remove(RmSourceFlags::ASYNC);
        assert!(flag == RmSourceFlags::LOOPING);
    }
}
