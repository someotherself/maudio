//! Event-based alternative to polling resource loading

use std::sync::{Arc, Mutex};

use maudio_sys::ffi as sys;

use crate::{util::fence::Fence, AsRawRef, Binding};

/// Asynchronous notification pipeline for resource-manager data sources.
///
/// A `NotificationPipeline` allows you to receive signals from the resource
/// manager as a resource progresses through its loading pipeline.
///
/// It is typically used together with [`PendingResource`](crate::engine::resource::PendingResource) to avoid polling.
/// Instead of repeatedly calling `poll_ready()`, you can attach a pipeline
/// and wait for a notification (via a [`Fence`]).
///
/// # Stages
///
/// The resource manager exposes two main notification points:
///
/// - `Init` — The resource has been initialized (may still be loading).
/// - `Done` — The resource is fully ready for use.
///
/// In most cases, you should attach a notification to the `Done` stage.
///
/// # Example
///
/// ```ignore
/// # let rm = todo!();
/// # let path = todo!();
/// use crate::util::fence::Fence;
///
/// let fence = Fence::new();
///
/// let notif = NotificationPipelineBuilder::new()
///     .done_with_fence(&fence)
///     .build();
///
/// let pending = ResourceManagerBufferBuilder::new(&rm)
///     .file_path(path)
///     .notification(notif)
///     .build()?;
///
/// // Wait until loading is complete
/// fence.wait()?;
///
/// let buffer = pending.into_ready().unwrap();
/// ```
///
/// # When to use
///
/// Use a `NotificationPipeline` when:
/// - You are loading resources asynchronously
/// - You want to avoid polling [`PendingResource`](crate::engine::resource::PendingResource)
/// - You need to integrate with your own synchronization primitives
///
/// # Notes
/// - Attaching a pipeline is optional.
/// - The `NotificationPipeline` object is Send + Sync and cheap to clone and re-use.
/// - Internally, this wraps `ma_resource_manager_pipeline_notifications`.
#[derive(Clone)]
pub struct NotificationPipeline {
    inner: Arc<NotifPipeInner>,
}

struct NotifPipeInner {
    inner: sys::ma_resource_manager_pipeline_notifications,
    init: Option<Fence>, // ref count
    done: Option<Fence>, // ref count
}

unsafe impl Send for NotifPipeInner {}
unsafe impl Sync for NotifPipeInner {}

impl AsRawRef for NotificationPipeline {
    type Raw = sys::ma_resource_manager_pipeline_notifications;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner.inner
    }
}

/// Builder for [`NotificationPipeline`].
///
/// Allows attaching notifications (e.g. [`Fence`]) to specific stages
/// of the resource loading pipeline.
///
/// In most cases, only the `done` stage needs to be configured.
pub struct NotificationPipelineBuilder {
    inner: sys::ma_resource_manager_pipeline_notifications,
    init_fence: Option<Fence>,
    done_fence: Option<Fence>,
}

impl NotificationPipelineBuilder {
    #[allow(clippy::new_without_default)]
    pub fn new() -> NotificationPipelineBuilder {
        let inner = unsafe { sys::ma_resource_manager_pipeline_notifications_init() };
        Self {
            inner,
            init_fence: None,
            done_fence: None,
        }
    }

    /// Attach a [`Fence`] to be signaled when initialization completes.
    ///
    /// This stage occurs early and does not guarantee the resource is fully ready.
    pub fn init_with_fence(&mut self, fence: Fence) -> &mut Self {
        self.inner.init.pFence = fence.to_raw();
        self.init_fence = Some(fence);
        self
    }

    /// Attach a [`Fence`] to be signaled when the resource is fully ready.
    ///
    /// This is the most commonly used notification point.
    pub fn done_with_fence(&mut self, fence: &Fence) -> &mut Self {
        self.inner.done.pFence = fence.to_raw();
        self.done_fence = Some(fence.clone());
        self
    }

    pub fn build(self) -> NotificationPipeline {
        NotificationPipeline {
            inner: Arc::new(NotifPipeInner {
                inner: self.inner,
                init: self.init_fence,
                done: self.done_fence,
            }),
        }
    }
}

// Not implemented
#[repr(C)]
struct CustomNotif {
    cb: sys::ma_async_notification,
    state: *mut core::ffi::c_void,
}

// Not implemented
struct State {
    cb: Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>,
}
