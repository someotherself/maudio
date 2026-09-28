//! Callback fired whenever the engine processes and outputs audio frames.
use std::{
    cell::UnsafeCell,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

use maudio_sys::ffi as sys;

use crate::{
    backend::{
        custom_backend::CustomBackend,
        custom_context::{CustomContext, CustomContextInner},
    },
    util::{device_notif::DeviceStateNotifier, proc_notif::ProcFramesNotif},
};

#[derive(Default)]
pub(crate) struct EngineUserData {
    frame_counter: FrameCounter,
    pub(crate) state_notif: DeviceStateNotifier, // device notificationCallback
    process_callback: ProcessCallbackState,
    pub(crate) backend_state: Mutex<Option<ErasedBackendState>>,
}

#[derive(Default)]
struct FrameCounter {
    frames_processed: ProcFramesNotif,
    channels: u32,
}

impl FrameCounter {
    fn new(channels: u32) -> Self {
        Self {
            frames_processed: ProcFramesNotif::default(),
            channels,
        }
    }
}

#[derive(Default)]
struct ProcessCallbackState {
    cb: UnsafeCell<Option<Box<EngineProcessCallback>>>,
    panic_flag: Arc<AtomicBool>,
    callback_lock: AtomicBool, // prevents concurrent access to the cb
}

impl ProcessCallbackState {
    fn new(cb: Option<Box<EngineProcessCallback>>) -> Self {
        Self {
            cb: UnsafeCell::new(cb),
            panic_flag: Arc::new(AtomicBool::new(false)),
            callback_lock: AtomicBool::new(false),
        }
    }
}

impl EngineUserData {
    pub(crate) fn new(
        channels: u32,
        cb: Option<Box<EngineProcessCallback>>,
        state: Option<ErasedBackendState>,
    ) -> Self {
        EngineUserData {
            frame_counter: FrameCounter::new(channels),
            state_notif: DeviceStateNotifier::default(),
            process_callback: ProcessCallbackState::new(cb),
            backend_state: Mutex::new(state),
        }
    }

    pub(crate) fn clone_proc_notif(&self) -> ProcFramesNotif {
        self.frame_counter.frames_processed.clone()
    }

    #[allow(unused)]
    pub(crate) fn data_callback_panicked(&self) -> bool {
        self.process_callback.panic_flag.load(Ordering::Relaxed)
    }

    pub(crate) fn clone_panic_flag(&self) -> Arc<AtomicBool> {
        self.process_callback.panic_flag.clone()
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

    let ctx = unsafe { &*(user_data as *const EngineUserData) };

    if ctx.process_callback.panic_flag.load(Ordering::Relaxed) {
        // The callback is poisoned
        return;
    }

    if frames_out.is_null() || frame_count == 0 {
        return;
    }

    ctx.frame_counter.frames_processed.add_frames(frame_count);

    if ctx
        .process_callback
        .callback_lock
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        //Another thread is already running this callback
        return;
    }

    let channels = ctx.frame_counter.channels as usize;
    // Engine is alwaus f32, no need to adjust to vec storage units
    let Some(slice_len) = (frame_count as usize).checked_mul(channels) else {
        return;
    };

    // Out is only valid for the duration of the callback
    let out = core::slice::from_raw_parts_mut(frames_out, slice_len);

    let cb_slot = &mut *ctx.process_callback.cb.get();
    if let Some(cb) = cb_slot.as_mut() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            cb(out, ctx.frame_counter.channels);
        }));

        if result.is_err() {
            // Disable callback permanently after panic.
            ctx.process_callback
                .panic_flag
                .store(true, Ordering::Release);
            *cb_slot = None;
        }
    }

    ctx.process_callback
        .callback_lock
        .store(false, Ordering::Release);
}

#[doc(hidden)]
pub struct ErasedBackendState {
    pub(crate) data: *mut core::ffi::c_void,
    vtable: BackendStateVTable,
}

impl ErasedBackendState {
    pub(crate) fn new<T>(data: T) -> Self {
        let boxed = Box::new(data);

        unsafe fn drop_impl<T>(ptr: *mut std::ffi::c_void) {
            drop(Box::from_raw(ptr.cast::<T>()));
        }

        let vtable: BackendStateVTable = BackendStateVTable {
            drop: drop_impl::<T>,
        };

        Self {
            data: Box::into_raw(boxed).cast(),
            vtable,
        }
    }
}

struct BackendStateVTable {
    drop: unsafe fn(*mut core::ffi::c_void),
}

impl Drop for ErasedBackendState {
    fn drop(&mut self) {
        unsafe { (self.vtable.drop)(self.data) }
    }
}

pub(crate) struct CustomBackendState<'device, B: CustomBackend> {
    pub(crate) _custom_context: Arc<CustomContextInner<B>>,
    pub(crate) backend_device: OnceLock<B::Device<'device>>,
}

impl<'device, B: CustomBackend> CustomBackendState<'device, B> {
    pub(crate) fn new_erased(ctx: &CustomContext<B>) -> ErasedBackendState {
        let state = CustomBackendState {
            _custom_context: ctx.0.clone(),
            backend_device: OnceLock::new(),
        };

        ErasedBackendState::new(state)
    }
}

// The device must be destroyed before the context
impl<'device, B: CustomBackend> Drop for CustomBackendState<'device, B> {
    fn drop(&mut self) {
        drop(self.backend_device.take());
    }
}
