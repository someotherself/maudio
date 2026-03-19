use std::{
    path::Path,
    sync::{atomic::AtomicBool, mpsc::Sender, Arc},
};

use crate::{
    engine::engine_host::{
        soundgroup_handle::SoundGroupHandle, BuilderId, HostDispatcher, HostStore, HostedBuilders,
        Job, SoundId,
    },
    sound::sound_builder::SoundBuilder,
    ErrorKinds, MaResult,
};

pub struct SoundHandle {
    inner: Arc<SoundHandleInner>,
}

struct SoundHandleInner {
    sender: Sender<Job>,
    sound: SoundId,
    is_shutdown: AtomicBool,
}

unsafe impl Send for SoundHandleInner {}
unsafe impl Sync for SoundHandleInner {}

impl SoundHandle {
    pub(crate) fn new(id: SoundId, sender: Sender<Job>) -> Self {
        SoundHandle {
            inner: Arc::new(SoundHandleInner {
                sender,
                sound: id,
                is_shutdown: AtomicBool::new(false),
            }),
        }
    }

    pub fn play_sound(&mut self) -> MaResult<()> {
        let id = self.inner.sound;
        self.call(move |_, hosts| hosts.get_sound(id).play_sound())?
    }
}

impl HostDispatcher for SoundHandle {
    fn post<F>(&self, f: F) -> MaResult<()>
    where
        F: FnOnce(&crate::engine::Engine, &mut HostStore<'_, '_>) + Send + 'static,
    {
        self.inner
            .sender
            .send(Box::new(f))
            .map_err(|_| crate::MaudioError::new_ma_error(ErrorKinds::ChannelSendError))
    }
}

pub struct SoundHostBuilder {
    builder: BuilderId,
    sender: Sender<Job>,
}

impl HostDispatcher for SoundHostBuilder {
    fn post<F>(&self, f: F) -> MaResult<()>
    where
        F: FnOnce(&crate::engine::Engine, &mut HostStore<'_, '_>) + Send + 'static,
    {
        self.sender
            .send(Box::new(f))
            .map_err(|_| crate::MaudioError::new_ma_error(ErrorKinds::ChannelSendError))
    }
}

impl SoundHostBuilder {
    pub(crate) fn new(hosts: &mut HostStore<'_, '_>, sender: Sender<Job>) -> Self {
        let builder = SoundBuilder::new(hosts.engine);
        let id = hosts.insert_builder(builder);
        Self {
            builder: id,
            sender,
        }
    }

    pub fn no_source(&mut self) -> MaResult<&mut Self> {
        let id = self.builder;
        self.post(move |_, h| match h.builders.values.get_mut(&id).unwrap() {
            HostedBuilders::Sounds { value } => {
                value.no_source();
            }
        })?;
        Ok(self)
    }

    pub fn file_path(&mut self, path: &Path) -> MaResult<&mut Self> {
        let id = self.builder;
        let path_copy = path.to_path_buf();
        self.post(move |_, h| match h.builders.values.get_mut(&id).unwrap() {
            HostedBuilders::Sounds { value } => {
                value.file_path(&path_copy);
            }
        })?;
        Ok(self)
    }

    pub fn sound_group(&mut self, _group: &SoundGroupHandle) -> MaResult<&mut Self> {
        // let id = self.builder;
        // let group_id = group.inner.group;
        self.post(move |_, _h| todo!())?;
        Ok(self)
    }
}
