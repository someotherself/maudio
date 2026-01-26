//! Notification for when a sound reaches the end.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use maudio_sys::ffi as sys;

/// A lightweight notification handle that becomes `true` when a sound finishes playback.
///
/// The audio thread sets the flag when playback ends. You can then:
/// - check it without clearing via [`peek()`](EndNotifier::peek())
/// - consume it exactly once via [`take()`](EndNotifier::take()) (recommended)
/// - run a closure once via [`call_if_notified()`](EndNotifier::call_if_notified())
///
/// The `EndNotifier` is not triggered by scheduled events like [`Sound::set_stop_time_pcm()`](crate::sound::Sound::set_stop_time_pcm())
///
/// Cloning an `EndNotifier` creates another handle to the same underlying notification flag.
#[derive(Debug, Clone)]
pub struct EndNotifier {
    flag: Arc<AtomicBool>,
}

impl EndNotifier {
    pub(crate) fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn clone_flag(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }

    /// Returns `true` if the end notification has been triggered.
    ///
    /// This does **not** clear the notification. Use [`EndNotifier::take()`] if you want “fire once”
    /// behavior.
    #[inline]
    pub fn peek(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    /// Consumes the notification and returns whether it was set.
    ///
    /// Returns `true` exactly once per playback end (until the sound ends again and
    /// triggers another notification).
    #[inline]
    pub fn take(&self) -> bool {
        self.flag.swap(false, Ordering::Relaxed)
    }

    /// Clears the notification flag.
    ///
    /// This is useful if you want to ignore a pending notification (for example after
    /// seeking, restarting, or reusing a sound).
    #[inline]
    pub fn clear(&self) {
        self.flag.store(false, Ordering::Relaxed);
    }

    /// Executes `f` if the end notification has been triggered, consuming it.
    ///
    /// Equivalent to:
    /// `if notifier.take() { f(); }`
    pub fn call_if_notified<F: FnOnce()>(&self, f: F) {
        if self.take() {
            f();
        }
    }

    pub(crate) fn as_user_data_ptr(&self) -> *mut core::ffi::c_void {
        std::sync::Arc::as_ptr(&self.flag) as *mut core::ffi::c_void
    }
}

pub(crate) unsafe extern "C" fn on_end_callback(
    user_data: *mut core::ffi::c_void,
    _sound: *mut sys::ma_sound,
) {
    if user_data.is_null() {
        return;
    }
    let flag = unsafe { &*(user_data as *const std::sync::atomic::AtomicBool) };
    flag.store(true, Ordering::Relaxed);
}
