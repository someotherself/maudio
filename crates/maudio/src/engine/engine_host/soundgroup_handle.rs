use std::sync::{atomic::AtomicBool, mpsc::Sender, Arc};

use crate::engine::engine_host::{GroupId, Job};

pub struct SoundGroupHandle {
    pub(crate) inner: Arc<SoundGroupHandleInner>,
}

pub(crate) struct SoundGroupHandleInner {
    sender: Sender<Job>,
    pub(crate) group: GroupId,
    is_shutdown: AtomicBool,
}
