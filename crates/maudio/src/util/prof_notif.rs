use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// General purpose notifier that keeps track of the number of frames processed
#[derive(Default, Clone)]
pub struct ProcFramesNotif {
    inner: Arc<ProcNotifInner>,
}

#[derive(Default)]
struct ProcNotifInner {
    frames_processed: AtomicU64,
    old_frames: AtomicU64,
}

impl ProcFramesNotif {
    #[inline]
    pub(crate) fn add_frames(&self, frames: u64) {
        self.inner
            .frames_processed
            .fetch_add(frames, Ordering::Relaxed);
    }

    /// Returns `true` if the process notification has been triggered (edge-triggered).
    ///
    /// This method does not update internal state. Use [`ProcFramesNotif::take_delta()`] to consume the progress
    #[inline]
    pub fn peek(&self) -> bool {
        self.inner.old_frames.load(Ordering::Relaxed)
            < self.inner.frames_processed.load(Ordering::Relaxed)
    }

    /// Resets the shared processed-frame counter back to 0.
    ///
    /// Note: this affects all clones of this notifier because the counter is shared.
    #[inline]
    pub fn clear(&mut self) {
        self.inner.frames_processed.store(0, Ordering::Relaxed);
        self.inner.old_frames.store(0, Ordering::Relaxed);
    }

    /// Returns the number of frames processed since the last call to `take_delta()`,
    /// and updates the internal cursor.
    ///
    /// Returns 0 if no progress has been made.
    #[inline]
    pub fn take_delta(&mut self) -> u64 {
        let cur = self.inner.frames_processed.load(Ordering::Relaxed);
        let delta = cur.saturating_sub(self.inner.old_frames.load(Ordering::Relaxed));
        self.inner.old_frames.store(cur, Ordering::Relaxed);
        delta
    }

    /// Executes `f(delta_frames)` if progress has been made since the last call to
    /// [`ProcFramesNotif::take_delta()`], and consumes that progress.
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
}
