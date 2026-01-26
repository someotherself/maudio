use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use maudio_sys::ffi as sys;

#[derive(Default)]
pub struct ProcessState {
    frames_read: AtomicU64,
    cb: Option<Box<EngineProcessCallback>>,
}

#[derive(Clone)]
pub struct ProcessNotifier {
    channels: u32,
    old_frames: u64,
    state: Arc<ProcessState>,
}

impl ProcessNotifier {
    pub(crate) fn new(channels: u32, cb: Option<Box<EngineProcessCallback>>) -> Self {
        let state: ProcessState = ProcessState {
            frames_read: AtomicU64::new(0),
            cb,
        };
        Self {
            channels,
            old_frames: 0,
            #[allow(clippy::arc_with_non_send_sync)]
            state: Arc::new(state),
        }
    }

    /// Returns `true` if the process notification has been triggered (edge-triggered).
    ///
    /// This method does not update internal state. Use [`ProcessNotifier::take_delta()`] to consume the progress
    #[inline]
    pub fn peek(&self) -> bool {
        self.old_frames < self.state.frames_read.load(Ordering::Relaxed)
    }

    /// Resets the shared processed-frame counter back to 0.
    ///
    /// Note: this affects all clones of this notifier because the counter is shared.
    #[inline]
    pub fn clear(&mut self) {
        self.state.frames_read.store(0, Ordering::Relaxed);
        self.old_frames = 0;
    }

    /// Returns the number of frames processed since the last call to `take_delta()`,
    /// and updates the internal cursor.
    ///
    /// Returns 0 if no progress has been made.
    #[inline]
    pub fn take_delta(&mut self) -> u64 {
        let cur = self.state.frames_read.load(Ordering::Relaxed);
        let delta = cur.saturating_sub(self.old_frames);
        self.old_frames = cur;
        delta
    }

    /// Executes `f(delta_frames)` if progress has been made since the last call to
    /// [`ProcessNotifier::take_delta()`], and consumes that progress.
    ///
    /// Equivalent to:
    /// `let d = notifier.take_delta(); if d != 0 { f(d) }`
    #[inline]
    pub fn call_if_triggered<F: FnOnce(u64)>(&mut self, f: F) {
        let delta = self.take_delta();
        if delta != 0 {
            f(delta);
        }
    }

    pub(crate) fn clone_flag(&self) -> Arc<ProcessState> {
        self.state.clone()
    }

    pub(crate) fn as_user_data_ptr(&self) -> *mut core::ffi::c_void {
        std::sync::Arc::as_ptr(&self.state) as *mut core::ffi::c_void
    }
}

#[derive(Debug, Copy, Clone)]
pub struct EngineProcessProc {
    old_frame_count: u64,
    // Number of frames in this processing block. Returned by the callback.
    frame_count: u64,
    // Channels in the processing block.
    pub channels: u32,
}

pub type EngineProcessCallback = dyn FnMut(&mut [f32]) + Send + 'static;

pub(crate) unsafe extern "C" fn on_process_callback(
    user_data: *mut core::ffi::c_void,
    _frames_out: *mut f32,
    frame_count: sys::ma_uint64,
) {
    if user_data.is_null() {
        return;
    }

    let ctx = unsafe { &*(user_data as *const ProcessState) };
    ctx.frames_read.fetch_add(frame_count, Ordering::Relaxed);

    // if frames_out.is_null() {
    //     return;
    // }

    // let Some(process) = ctx.cb.as_ref() else {
    //     return;
    // };

    // let out = unsafe { core::slice::from_raw_parts_mut(frames_out, frame_count as usize) };
    // unsafe { process.process(ctx, out) }
}
