// Experimental feature

use std::{collections::HashMap, sync::mpsc::Sender, thread::JoinHandle};

use crate::{
    audio::{formats::SampleBuffer, math::vec3::Vec3, spatial::cone::Cone},
    engine::{
        node_graph::{nodes::NodeRef, NodeGraphRef},
        Engine, EngineOps,
    },
    sound::Sound,
    ErrorKinds, MaResult,
};

pub struct SoundId(u64);

#[derive(Default)]
struct Hosts<'a> {
    sounds: SoundHosts<'a>,
}

#[derive(Default)]
struct SoundHosts<'a> {
    next_sound_id: u64,
    sounds: HashMap<SoundId, Sound<'a>>,
}

type Job = Box<dyn FnOnce(&mut Engine, &mut SoundHosts) + Send + 'static>;

pub struct EngineHandle {
    handle: JoinHandle<MaResult<()>>,
    sender: Sender<Job>,
}

impl Engine {
    pub fn spawn() -> MaResult<EngineHandle> {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();

        let join = std::thread::spawn(move || -> MaResult<()> {
            let mut engine = Engine::new()?;
            let mut sound_hosts = SoundHosts::default();

            while let Ok(job) = rx.recv() {
                job(&mut engine, &mut sound_hosts)
            }
            Ok(())
        });
        Ok(EngineHandle {
            handle: join,
            sender: tx,
        })
    }
}

impl EngineHandle {
    pub fn shutdown(self) {
        drop(self.sender);
        let _ = self.handle.join().unwrap();
    }

    fn post<F>(&self, f: F) -> MaResult<()>
    where
        F: FnOnce(&mut Engine, &mut SoundHosts) + Send + 'static,
    {
        self.sender
            .send(Box::new(f))
            .map_err(|_| crate::MaudioError::new_ma_error(ErrorKinds::ChannelSendError))
    }

    fn call<F, R>(&self, f: F) -> MaResult<R>
    where
        F: FnOnce(&mut Engine, &mut SoundHosts) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (rtx, rrx) = std::sync::mpsc::channel::<R>();
        self.post(move |eng, sounds| {
            let r = f(eng, sounds);
            let _ = rtx.send(r);
        })?;
        rrx.recv()
            .map_err(|_| crate::MaudioError::new_ma_error(ErrorKinds::ChannelRecieveError))
    }
}

impl EngineHandle {
    pub fn set_volume(&self, volume: f32) -> MaResult<()> {
        self.call(move |e, _s| e.set_volume(volume))?
    }

    pub fn volume(&self) -> MaResult<f32> {
        self.call(move |e, _s| e.volume())
    }

    pub fn set_gain_db(&self, db_gain: f32) -> MaResult<()> {
        self.call(move |e, _s| e.set_gain_db(db_gain))?
    }

    pub fn gain_db(&self) -> MaResult<f32> {
        self.call(move |e, _s| e.gain_db())
    }

    pub fn listener_count(&self) -> MaResult<u32> {
        self.call(move |e, _s| e.listener_count())
    }

    pub fn closest_listener(&self, position: Vec3) -> MaResult<u32> {
        self.call(move |e, _s| e.closest_listener(position))
    }

    pub fn set_position(&self, listener: u32, position: Vec3) -> MaResult<()> {
        self.post(move |e, _s| e.set_position(listener, position))
    }

    pub fn position(&self, listener: u32) -> MaResult<Vec3> {
        self.call(move |e, _s| e.position(listener))
    }

    pub fn set_direction(&self, listener: u32, direction: Vec3) -> MaResult<()> {
        self.post(move |e, _s| e.set_direction(listener, direction))
    }

    pub fn direction(&self, listener: u32) -> MaResult<Vec3> {
        self.call(move |e, _s| e.direction(listener))
    }

    pub fn set_velocity(&self, listener: u32, position: Vec3) -> MaResult<()> {
        self.post(move |e, _s| e.set_velocity(listener, position))
    }

    pub fn velocity(&self, listener: u32) -> MaResult<Vec3> {
        self.call(move |e, _s| e.velocity(listener))
    }

    pub fn set_cone(&self, listener: u32, cone: Cone) -> MaResult<()> {
        self.post(move |e, _s| e.set_cone(listener, cone))
    }

    pub fn cone(&self, listener: u32) -> MaResult<Cone> {
        self.call(move |e, _s| e.cone(listener))
    }

    pub fn set_world_up(&self, listener: u32, up_direction: Vec3) -> MaResult<()> {
        self.post(move |e, _s| e.set_world_up(listener, up_direction))
    }

    pub fn get_world_up(&self, listener: u32) -> MaResult<Vec3> {
        self.call(move |e, _s| e.get_world_up(listener))
    }

    pub fn toggle_listener(&self, listener: u32, enabled: bool) -> MaResult<()> {
        self.post(move |e, _s| e.toggle_listener(listener, enabled))
    }

    pub fn listener_enabled(&self, listener: u32) -> MaResult<bool> {
        self.call(move |e, _s| e.listener_enabled(listener))
    }

    fn as_node_graph(&self) -> Option<NodeGraphRef<'_>> {
        // engine_ffi::ma_engine_get_node_graph(self)
        // TODO
        todo!()
    }

    /// Renders audio from the engine into a newly allocated buffer.
    ///
    /// This function pulls audio from the engine’s internal node graph and returns
    /// up to `frame_count` frames of interleaved PCM samples.
    ///
    /// ### Semantics
    ///
    /// - This is a **pull-based render operation**.
    /// - The engine will attempt to render `frame_count` frames, but it may return
    ///   **fewer frames**.
    /// - The number of frames actually rendered is returned alongside the samples.
    ///
    pub fn read_pcm_frames(&self, frame_count: u64) -> MaResult<SampleBuffer<f32>> {
        self.call(move |e, _s| e.read_pcm_frames(frame_count))?
    }

    /// Returns the engine’s **endpoint node**.
    ///
    /// The endpoint node is the final node in the engine’s internal node graph.
    /// All sounds ultimately connect to this node before audio is sent to the
    /// output device.
    ///
    /// This can be used to:
    /// - Inspect or modify the engine’s node graph
    /// - Attach custom processing nodes (effects, mixers, etc.)
    /// - Query graph-level properties
    ///
    /// ## Lifetime
    /// The returned [`NodeRef`] borrows the engine mutably and cannot outlive it.
    /// Only one mutable access to the node graph may exist at a time.
    fn endpoint(&self) -> Option<NodeRef<'_>> {
        // engine_ffi::ma_engine_get_endpoint(self)
        // TODO
        todo!()
    }

    /// Returns the current engine time in **PCM frames**.
    ///
    /// This is the engine’s global playback time, measured in sample frames
    /// at the engine’s sample rate.
    ///
    /// ## Use cases
    /// - Sample-accurate scheduling
    /// - Synchronizing multiple sounds
    /// - Implementing custom transport or timeline logic
    ///
    /// ## Notes
    /// - The time is monotonic unless explicitly modified with
    ///   [`EngineHost::set_time_pcm()`].
    /// - The value is independent of any individual sound’s playback position
    fn time_pcm(&self) -> MaResult<u64> {
        self.call(move |e, _s| e.time_pcm())
    }

    /// Returns the current engine time in **milliseconds**.
    ///
    /// This is a convenience wrapper over the engine’s internal PCM-frame
    /// clock, converted to milliseconds using the engine’s sample rate.
    ///
    /// - For sample-accurate work, prefer [`EngineHost::set_time_pcm()`].
    fn time_mili(&self) -> MaResult<u64> {
        self.call(move |e, _s| e.time_mili())
    }

    /// Sets the engine’s global time in **PCM frames**.
    ///
    /// This directly modifies the engine’s internal timeline.
    ///
    /// ## Effects
    /// - All sounds and nodes that depend on engine time will observe the new
    ///   value.
    /// - This can be used to implement seeking or timeline resets.
    ///
    /// ## Note
    /// Changing engine time while audio is playing may cause audible artifacts,
    /// depending on the active nodes and sounds.
    fn set_time_pcm(&self, time: u64) -> MaResult<()> {
        self.post(move |e, _s| e.set_time_pcm(time))
    }

    /// Sets the engine’s global time in **milliseconds**.
    ///
    /// This is equivalent to setting the time in PCM frames, but expressed
    /// in milliseconds.
    ///
    /// ## Notes
    /// - Internally converted to PCM frames.
    /// - Precision may be lower than [`set_time_pcm`](Self).
    fn set_time_mili(&self, time: u64) -> MaResult<()> {
        self.post(move |e, _s| e.set_time_mili(time))
    }

    /// Returns the number of output **channels** used by the engine.
    ///
    /// Common values include:
    /// - `1` — mono
    /// - `2` — stereo
    ///
    /// This reflects the channel count of the engine’s internal node graph
    /// and output device.
    fn channels(&self) -> MaResult<u32> {
        self.call(move |e, _s| e.channels())
    }

    /// Returns the engine’s **sample rate**, in Hz.
    ///
    /// This is the sample rate at which the engine processes audio and
    /// advances its internal time.
    ///
    /// ## Notes
    /// - Typically matches the output device’s sample rate.
    /// - Used to convert between PCM frames and real time.
    fn sample_rate(&self) -> MaResult<u32> {
        self.call(move |e, _s| e.sample_rate())
    }
}
