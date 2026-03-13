//! Shared notifier for device state-change notifications.
//!
//! # Example
//!
//! ```ignore
//!
//! let mut device = DeviceBuilder::playback()
//!     .f32()
//!     .playback_channels(2)
//!     .state_notifier()
//!     .with_callback(|_a, _b, _c| {})
//!     .unwrap();
//!
//! let notifier = device.get_state_notifier().unwrap();
//!
//! device.device_start().unwrap();
//!
//! loop {
//!     // Retrieve and clear all pending notifications.
//!     let notif = notifier.take_notifications();
//!
//!     if notif.contains(DeviceNotificationType::Started.into()) {
//!         println!("device started");
//!     }
//!
//!     if notif.contains(DeviceNotificationType::Stopped.into()) {
//!         println!("device stopped");
//!     }
//!
//!     if notif.contains_any(
//!         DeviceNotificationType::InterruptionBegan
//!             | DeviceNotificationType::InterruptionEnded,
//!     ) {
//!         println!("interruption state changed");
//!     }
//!
//!     std::thread::sleep(std::time::Duration::from_millis(10));
//! }
//! ```
use std::sync::{atomic::AtomicU32, Arc};

use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

/// `DeviceStateNotifier` is a lightweight polling helper that records device
/// notification events emitted by miniaudio, such as start/stop, rerouting,
/// or interruption state changes.
///
/// The notifier is cheap to clone and all clones share the same internal bitmask.
///
/// # Notification behavior
///
/// Notifications are stored as a bitmask of [`DeviceNotificationType`] values.
///
/// This means notifications are **accumulated**, not queued:
///
/// - if the same notification occurs multiple times before being consumed, it is
///   only represented once in the mask;
/// - different notification kinds can be accumulated together;
/// - consuming notifications from one clone affects all other clones because the
///   underlying state is shared.
///
/// # Asynchronous delivery
///
/// Device notifications are delivered asynchronously by the underlying audio
/// backend. The callback that records these events may run on a driver or
/// backend-managed thread.
///
/// Because of this, a notification may not be visible immediately after the
/// operation that triggered it. For example, calling `device.start()` and then
/// immediately polling the notifier may not yet show a
/// [`DeviceNotificationType::Started`] event.
///
/// Applications that rely on these notifications should allow a short delay or
/// poll periodically rather than expecting them to appear synchronously.
///
/// # Backend behavior
///
/// Notification delivery depends on the active audio backend and platform.
/// Not all backends report all notification types.
///
/// In practice, [`Started`](DeviceNotificationType::Started),
/// [`Stopped`](DeviceNotificationType::Stopped), and some interruption events
/// are the most commonly observed notifications. Other notifications, such as
/// [`Rerouted`](DeviceNotificationType::Rerouted), may not be reported even if
/// rerouting happens, because some backends handle stream/device rerouting
/// internally without notifying miniaudio.
///
///
/// Code using this type should treat notifications as best-effort signals rather
/// than guaranteed lifecycle events.
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

    /// Atomically returns all currently pending notifications and clears them.
    ///
    /// This is the main consuming API for the notifier.
    ///
    /// Because the internal state is shared, calling this method on one clone
    /// clears the pending notification set for all clones.
    pub fn take_notifications(&self) -> DeviceNotificationSet {
        let bits = self.mask.swap(0, std::sync::atomic::Ordering::Relaxed);
        DeviceNotificationSet(bits)
    }

    /// Returns the currently pending notifications without clearing them.
    ///
    /// This is a non-consuming snapshot of the shared notification mask.
    pub fn notifications(&self) -> DeviceNotificationSet {
        let bits = self.mask.load(std::sync::atomic::Ordering::Relaxed);
        DeviceNotificationSet(bits)
    }

    /// Clears all pending notifications.
    ///
    /// Because the internal state is shared, this affects all clones of the notifier.
    pub fn clear(&self) {
        self.mask.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Returns `true` if all notification flags in `flags` are currently present.
    pub fn contains(&self, flags: impl Into<DeviceNotificationSet>) -> bool {
        self.notifications().contains(flags.into())
    }

    /// Returns `true` if any notification flag in `flags` is currently present.
    pub fn contains_any(&self, flags: impl Into<DeviceNotificationSet>) -> bool {
        self.notifications().contains_any(flags.into())
    }

    /// Returns `true` if a pending [`DeviceNotificationType::Started`] notification exists.
    pub fn started(&self) -> bool {
        self.contains(DeviceNotificationType::Started)
    }

    /// Returns `true` if a pending [`DeviceNotificationType::Stopped`] notification exists.
    pub fn stopped(&self) -> bool {
        self.contains(DeviceNotificationType::Stopped)
    }

    /// Returns `true` if a pending [`DeviceNotificationType::Rerouted`] notification exists.
    ///
    /// Note that many backends do not reliably report rerouting events.
    pub fn rerouted(&self) -> bool {
        self.contains(DeviceNotificationType::Rerouted)
    }

    /// Returns `true` if a pending [`DeviceNotificationType::InterruptionBegan`]
    /// notification exists.
    pub fn interruption_began(&self) -> bool {
        self.contains(DeviceNotificationType::InterruptionBegan)
    }

    /// Returns `true` if a pending [`DeviceNotificationType::InterruptionEnded`]
    /// notification exists.
    pub fn interruption_ended(&self) -> bool {
        self.contains(DeviceNotificationType::InterruptionEnded)
    }

    /// Returns `true` if a pending [`DeviceNotificationType::Unlocked`] notification exists.
    ///
    /// This notification is backend-dependent and may not be observed on all platforms.
    pub fn unlocked(&self) -> bool {
        self.contains(DeviceNotificationType::Unlocked)
    }

    /// Clears a single notification flag and returns whether it had been set.
    ///
    /// This can be useful when you want to consume one flag independently without
    /// clearing the entire notification set.
    ///
    /// `flag` should be a single bit corresponding to a [`DeviceNotificationType`].
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

/// A set of accumulated device notification flags.
///
/// This is a small bitflag-like wrapper used by [`DeviceStateNotifier`] to represent
/// pending device notifications.
///
/// Unlike an event queue, a `DeviceNotificationSet` only records whether a given
/// notification kind is present, not how many times it occurred or in what order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DeviceNotificationSet(u32);

impl DeviceNotificationSet {
    /// Returns an empty notification set.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Creates a notification set containing a single notification type.
    pub const fn from_type(ty: DeviceNotificationType) -> Self {
        Self(ty.bit())
    }

    /// Returns the raw bit representation of this set.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns `true` if all flags in `other` are present in this set.
    pub const fn contains(self, other: DeviceNotificationSet) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns `true` if any flag in `other` is present in this set.
    pub const fn contains_any(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Returns `true` if this set contains no flags.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// A device notification kind emitted by miniaudio.
///
/// Not all backends report all notification types. The availability and timing of
/// these notifications is platform-dependent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceNotificationType {
    /// The device has started.
    Started,
    /// The device has stopped.
    Stopped,
    /// The device has been rerouted, such as to a different output/input path.
    ///
    /// This is not guaranteed to be reported by every backend.
    Rerouted,
    /// An interruption has begun.
    ///
    /// This is commonly associated with mobile/platform audio interruptions,
    /// but support is backend-dependent.
    InterruptionBegan,
    /// A previously reported interruption has ended.
    InterruptionEnded,
    /// The device has been unlocked.
    ///
    /// This is a backend-specific notification and may be rare in practice.
    Unlocked,
}

impl DeviceNotificationType {
    /// Returns the internal bit corresponding to this notification type.
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
