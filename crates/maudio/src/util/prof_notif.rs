//! General purpose notifier that keeps track of the number of frames processed
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// Shared frame-progress notifier used by audio processing components.
///
/// `ProcFramesNotif` tracks how many PCM frames have been processed by a producer,
/// such as an [`Engine`](crate::engine::Engine) or [`Device`](crate::device::Device), and allows another part of the program to
/// observe that progress.
///
/// The notifier is cheap to clone and all clones refer to the same shared state.
/// This makes it useful for polling progress from another thread without needing
/// to receive a callback directly.
///
/// # Notes
///
/// - The unit is **frames**, not samples.
/// - This is intended as a lightweight polling helper, not a precise
///   synchronization primitive.
/// - Notification state is edge-like: once progress is consumed with
///   [`take_delta`](ProcFramesNotif::take_delta), it is no longer considered
///   pending.
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
    /// This is a non-consuming check. It does not update the internal cursor used by
    /// [`take_delta`](ProcFramesNotif::take_delta).
    ///
    /// This is useful when you only need to know whether progress has happened, without
    /// yet retrieving the exact frame count.    
    #[inline]
    pub fn peek(&self) -> bool {
        self.inner.old_frames.load(Ordering::Relaxed)
            < self.inner.frames_processed.load(Ordering::Relaxed)
    }

    /// Clears all tracked progress.
    ///
    /// Both the total processed-frame counter and the observation cursor are reset to `0`.
    ///
    /// Because the underlying state is shared, this affects all clones of this notifier.
    #[inline]
    pub fn clear(&self) {
        self.inner.frames_processed.store(0, Ordering::Relaxed);
        self.inner.old_frames.store(0, Ordering::Relaxed);
    }

    /// Returns the number of frames processed since the last call to `take_delta()`.
    ///
    /// After reading the current delta, this advances the internal cursor to the current
    /// processed-frame count, consuming the pending progress.
    ///
    /// Returns `0` if no new frames have been processed.
    ///
    /// Because the cursor is shared, calling this on one clone also consumes the pending
    /// delta for all other clones.
    #[inline]
    pub fn take_delta(&self) -> u64 {
        let cur = self.inner.frames_processed.load(Ordering::Relaxed);
        let delta = cur.saturating_sub(self.inner.old_frames.load(Ordering::Relaxed));
        self.inner.old_frames.store(cur, Ordering::Relaxed);
        delta
    }

    /// Calls `f` with the newly processed frame count, if any progress is pending.
    ///
    /// This is a convenience wrapper around [`take_delta`](ProcFramesNotif::take_delta).
    /// The closure is only invoked when the returned delta would be non-zero.
    ///
    /// Equivalent to:
    /// ```ignore
    /// let delta = notifier.take_delta();
    /// if delta != 0 {
    ///     f(delta);
    /// }
    /// ```
    #[inline]
    pub fn call_if_triggered<F: FnOnce(u64)>(&self, f: F) {
        let delta = self.take_delta();
        if delta != 0 {
            f(delta);
        }
    }
}
