use std::sync::Mutex;

use maudio_sys::ffi as sys;

use crate::{util::fence::Fence, Binding, MaResult};

pub struct NotificationPipeline {
    inner: sys::ma_resource_manager_pipeline_notifications,
}

impl NotificationPipeline {
    fn init() -> Self {
        let inner = unsafe { sys::ma_resource_manager_pipeline_notifications_init() };
        Self { inner }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyAt {
    Init,
    Done,
}

pub enum NotificationPipelineBuilder<'a> {
    Fence {
        at: NotifyAt,
        fence: &'a Fence,
    },
    Callback {
        at: NotifyAt,
        cb: Box<dyn FnOnce(MaResult<()>) + Send + 'static>,
    },
}

impl<'a> NotificationPipelineBuilder<'a> {
    fn init_with_fence(fence: &'a Fence) -> NotificationPipeline {
        let mut notif = NotificationPipeline::init();
        notif.inner.init.pFence = fence.to_raw();
        notif
    }

    fn done_with_fence(fence: &'a Fence) -> NotificationPipeline {
        let mut notif = NotificationPipeline::init();
        notif.inner.done.pFence = fence.to_raw();
        notif
    }

    fn init_with_cb() -> NotificationPipeline {
        todo!()
    }

    fn done_with_cb() {}
}

#[repr(C)]
struct CustomNotif {
    cb: sys::ma_async_notification,
    state: *mut core::ffi::c_void,
}

struct State {
    cb: Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>,
}
