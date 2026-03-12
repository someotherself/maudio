use std::sync::{atomic::AtomicU32, Arc};

use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

#[derive(Default, Clone)]
pub struct DeviceStateNotifier {
    mask: Arc<AtomicU32>,
}

impl DeviceStateNotifier {
    pub(crate) fn debug_ptr(&self) -> *const AtomicU32 {
        Arc::as_ptr(&self.mask)
    }

    pub(crate) fn store_notifications(&self, flags: u32) {
        self.mask
            .fetch_or(flags, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn take_notifications(&self) -> DeviceNotificationSet {
        let bits = self.mask.swap(0, std::sync::atomic::Ordering::Relaxed);
        DeviceNotificationSet(bits)
    }

    pub fn notifications(&self) -> DeviceNotificationSet {
        let bits = self.mask.load(std::sync::atomic::Ordering::Relaxed);
        DeviceNotificationSet(bits)
    }

    pub fn clear(&self) {
        self.mask.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn contains(&self, flags: impl Into<DeviceNotificationSet>) -> bool {
        self.notifications().contains(flags.into())
    }

    pub fn contains_any(&self, flags: impl Into<DeviceNotificationSet>) -> bool {
        self.notifications().contains_any(flags.into())
    }

    pub fn started(&self) -> bool {
        self.contains(DeviceNotificationType::Started)
    }

    pub fn stopped(&self) -> bool {
        self.contains(DeviceNotificationType::Stopped)
    }

    pub fn rerouted(&self) -> bool {
        self.contains(DeviceNotificationType::Rerouted)
    }

    pub fn interruption_began(&self) -> bool {
        self.contains(DeviceNotificationType::InterruptionBegan)
    }

    pub fn interruption_ended(&self) -> bool {
        self.contains(DeviceNotificationType::InterruptionEnded)
    }

    pub fn unlocked(&self) -> bool {
        self.contains(DeviceNotificationType::Unlocked)
    }

    pub fn take_flag(&self, flag: u32) -> bool {
        let old = self
            .mask
            .fetch_and(!flag, std::sync::atomic::Ordering::AcqRel);
        old & flag != 0
    }
}

impl From<DeviceNotificationType> for DeviceNotificationSet {
    fn from(value: DeviceNotificationType) -> Self {
        Self::from_type(value)
    }
}

impl std::ops::BitOr for DeviceNotificationSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOr<DeviceNotificationType> for DeviceNotificationSet {
    type Output = Self;

    fn bitor(self, rhs: DeviceNotificationType) -> Self::Output {
        Self(self.0 | rhs.bit())
    }
}

impl std::ops::BitOr for DeviceNotificationType {
    type Output = DeviceNotificationSet;

    fn bitor(self, rhs: Self) -> Self::Output {
        DeviceNotificationSet(self.bit() | rhs.bit())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DeviceNotificationSet(u32);

impl DeviceNotificationSet {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn from_type(ty: DeviceNotificationType) -> Self {
        Self(ty.bit())
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: DeviceNotificationSet) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn contains_any(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceNotificationType {
    Started,
    Stopped,
    Rerouted,
    InterruptionBegan,
    InterruptionEnded,
    Unlocked,
}

impl DeviceNotificationType {
    pub const fn bit(self) -> u32 {
        match self {
            DeviceNotificationType::Started => 1 << 0,
            DeviceNotificationType::Stopped => 1 << 1,
            DeviceNotificationType::Rerouted => 1 << 2,
            DeviceNotificationType::InterruptionBegan => 1 << 3,
            DeviceNotificationType::InterruptionEnded => 1 << 4,
            DeviceNotificationType::Unlocked => 1 << 5,
        }
    }
}

impl From<DeviceNotificationType> for sys::ma_device_notification_type {
    fn from(value: DeviceNotificationType) -> Self {
        match value {
            DeviceNotificationType::Started => {
                sys::ma_device_notification_type_ma_device_notification_type_started
            }
            DeviceNotificationType::Stopped => {
                sys::ma_device_notification_type_ma_device_notification_type_stopped
            }
            DeviceNotificationType::Rerouted => {
                sys::ma_device_notification_type_ma_device_notification_type_rerouted
            }
            DeviceNotificationType::InterruptionBegan => {
                sys::ma_device_notification_type_ma_device_notification_type_interruption_began
            }
            DeviceNotificationType::InterruptionEnded => {
                sys::ma_device_notification_type_ma_device_notification_type_interruption_ended
            }
            DeviceNotificationType::Unlocked => {
                sys::ma_device_notification_type_ma_device_notification_type_unlocked
            }
        }
    }
}

impl TryFrom<sys::ma_device_notification_type> for DeviceNotificationType {
    type Error = MaudioError;

    fn try_from(value: sys::ma_device_notification_type) -> Result<Self, Self::Error> {
        match value {
            sys::ma_device_notification_type_ma_device_notification_type_started => {
                Ok(DeviceNotificationType::Started)
            }
            sys::ma_device_notification_type_ma_device_notification_type_stopped => {
                Ok(DeviceNotificationType::Stopped)
            }
            sys::ma_device_notification_type_ma_device_notification_type_rerouted => {
                Ok(DeviceNotificationType::Rerouted)
            }
            sys::ma_device_notification_type_ma_device_notification_type_interruption_began => {
                Ok(DeviceNotificationType::InterruptionBegan)
            }
            sys::ma_device_notification_type_ma_device_notification_type_interruption_ended => {
                Ok(DeviceNotificationType::InterruptionEnded)
            }
            sys::ma_device_notification_type_ma_device_notification_type_unlocked => {
                Ok(DeviceNotificationType::Unlocked)
            }
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                DeviceNotificationType,
            >(other as i64))),
        }
    }
}
