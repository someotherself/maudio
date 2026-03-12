//! Callback fired whenever the engine processes and outputs audio frames.
use std::{
    cell::UnsafeCell,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use maudio_sys::ffi as sys;

use crate::util::{device_notif::DeviceStateNotifier, prof_notif::ProcFramesNotif};

#[derive(Default)]
pub struct ProcessState {
    frames_processed: ProcFramesNotif,
    channels: u32,
    cb: UnsafeCell<Option<Box<EngineProcessCallback>>>,
    pub(crate) state_notif: DeviceStateNotifier,
    panic_flag: Arc<AtomicBool>,
    in_cb: AtomicBool,
}

impl ProcessState {
    pub(crate) fn new(channels: u32, cb: Option<Box<EngineProcessCallback>>) -> Self {
        ProcessState {
            frames_processed: ProcFramesNotif::default(),
            channels,
            cb: UnsafeCell::new(cb),
            state_notif: DeviceStateNotifier::default(),
            panic_flag: Arc::new(AtomicBool::new(false)),
            in_cb: AtomicBool::new(false),
        }
    }

    pub(crate) fn clone_proc_notif(&self) -> ProcFramesNotif {
        self.frames_processed.clone()
    }

    pub(crate) fn data_callback_panicked(&self) -> bool {
        self.panic_flag.load(Ordering::Relaxed)
    }

    pub(crate) fn clone_panic_flag(&self) -> Arc<AtomicBool> {
        self.panic_flag.clone()
    }
}

// TODO: Maybe convert it to a generic as in the Device callback?
pub type EngineProcessCallback = dyn FnMut(&mut [f32], u32) + Send + 'static;

pub(crate) unsafe extern "C" fn on_process_callback(
    user_data: *mut core::ffi::c_void,
    frames_out: *mut f32,
    frame_count: sys::ma_uint64,
) {
    // `ma_engine_uninit()` guarantees the engine's audio thread is stopped before returning,
    // so this callback cannot run after the `ProcessState` userdata has been freed.
    if user_data.is_null() {
        return;
    }

    let ctx = unsafe { &*(user_data as *const ProcessState) };

    if ctx.panic_flag.load(Ordering::Relaxed) {
        // The callback is poisoned
        return;
    }

    if frames_out.is_null() || frame_count == 0 {
        return;
    }

    ctx.frames_processed.add_frames(frame_count);

    if ctx
        .in_cb
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        //Another thread is already running this callback
        return;
    }

    let channels = ctx.channels as usize;
    // Engine is alwaus f32, no need to adjust to vec storage units
    let slice_len = (frame_count as usize).saturating_mul(channels);

    // Out is only valid for the duration of the callback
    let out = core::slice::from_raw_parts_mut(frames_out, slice_len);

    let cb_slot = &mut *ctx.cb.get();
    if let Some(cb) = cb_slot.as_mut() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            cb(out, ctx.channels);
        }));

        if result.is_err() {
            // Disable callback permanently after panic.
            ctx.panic_flag.store(true, Ordering::Release);
            *cb_slot = None;
        }
    }

    ctx.in_cb.store(false, Ordering::Release);
}
