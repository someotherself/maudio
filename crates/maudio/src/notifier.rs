use std::sync::{Arc, atomic::AtomicBool};

use maudio_sys::ffi as sys;

pub struct EndNotifier {
    flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
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

    pub fn is_notified(&self) -> bool {
        self.flag.load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn reset(&self) {
        self.flag.store(false, std::sync::atomic::Ordering::Release);
    }

    pub fn call_if_notified<F: FnOnce()>(&self, f: F) {
        if self.is_notified() {
            self.flag.store(false, std::sync::atomic::Ordering::Release);
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
    flag.store(true, std::sync::atomic::Ordering::Release);
}
