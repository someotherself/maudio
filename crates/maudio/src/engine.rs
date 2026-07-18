//! High level audio engine.
//!
//! [`Engine`] is the main entry point for playback and mixing. It wraps
//! miniaudio’s `ma_engine` and provides access to the engine’s global clock,
//! output format (channels + sample rate), and endpoint node.
//!
//! Internally, the engine coordinates:
//! - a resource manager for loading and caching audio data
//! - a node graph for routing, mixing, and processing audio
//! - an output device and global engine clock
//!
//! ## Quick start
//! ```no_run
//! # use maudio::engine::Engine;
//! # fn main() -> maudio::MaResult<()> {
//! let engine = Engine::new()?;
//! // let mut sound = engine.new_sound_from_file("music.ogg")?;
//! // sound.play_sound()?;
//! /* block the main thread while the sound is playing */
//! # Ok(())
//! # }
//! ```
//!
//! ## Important notes
//!
//! Some types, like a [`Sound`], are intrinsicly tied to the engine and
//! a Sound cannot exit without the Engine being alive.
//!
//! This is enforced using using reference counting instead of lifetimes,
//! to more closely match the thread-safety model from miniaudio.
//!
//! This makes makes storing types or moving them across threads easier
//! but it also allows running `drop(engine)` at any point. This is accepted
//! behaviour and the engine can be recoved using [`Sound::engine`].
//!
//! ## Internals and thread-safety
//!
//! Internally, the Engine is a node_graph, but only a borrowed version of that node
//! graph can be obtained via [`Engine::as_node_graph`]. The [`NodeGraphRef`]
//! must be used if the user wants to create nodes, but it cannot run `read_pcm_frames`,
//! as that must be managed through the engine. The engine can also be recovered
//! using [`NodeGraphRef::engine`]. Be aware that it returns an `Option` as this
//! is not the only way to create a [`NodeGraphRef`].
//!
//! Across the library, many types will expose a function called `read_pcm_frames`.
//! This is the default way of manually pulling PCM frames. However, doing this
//! on the Engine is optional. While is has a device, `read_pcm_frames` is not
//! possible as the internal `Device` already calls this function internally.
//! Concurrent calls to (any) `read_pcm_frames` (on any type) are not safe.
//!
//! The engine exposes manually reading pcm frames on the `EngineReader` obtained
//! via [`Engine::try_acquire_reader`]. Only one instance of `EngineReader` can exist at any time.
//!
//! For more information about the Node Graph and it's thread safety model, see [`node_graph`].
//!
//! ## Resource manager
//! The resource manager handles loading and lifetime management of audio data
//! (for example, decoding audio files and sharing them between sounds).
//!
//! While the resource manager can be used independently in miniaudio, the
//! engine provides a higher-level abstraction that integrates it directly
//! with playback and mixing.
//!
//! ## Node graph
//! Audio flows through the engine’s internal node graph. Each sound is
//! represented as a node, and all nodes ultimately connect to the engine’s
//! endpoint.
//! See [`node_graph::NodeGraph`]
//!
//! The endpoint can be accessed via [`Engine::endpoint()`] to attach custom
//! processing nodes.
//!
//! ## Time
//! The engine maintains a global timeline that advances as audio is processed.
//! Time can be queried or modified in either PCM frames or milliseconds.
//!
//! For sample-accurate control, prefer the PCM-frame APIs.
use std::{
    mem::MaybeUninit,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::{
    audio::{
        formats::SampleBuffer, math::vec3::Vec3, sample_rate::SampleRate, spatial::cone::Cone,
    },
    data_source::AsSourcePtr,
    device::{device_id::DeviceId, DeviceInner, DeviceRef},
    engine::{
        engine_builder::EngineBuilder,
        engine_cb_notif::engine_notification_callback,
        node_graph::{nodes::NodeRef, NodeGraphRef},
        process_cb::ProcessState,
        resource::{ResourceManager, ResourceManagerRef},
    },
    sound::{
        sound_builder::SoundBuilder,
        sound_ffi,
        sound_flags::SoundFlags,
        sound_group::{SoundGroup, SoundGroupBuilder},
        Sound,
    },
    util::{device_notif::DeviceStateNotifier, fence::Fence, proc_notif::ProcFramesNotif},
    AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
};

use maudio_sys::ffi as sys;

pub mod engine_builder;

pub(crate) mod engine_cb_notif;
pub mod node_graph;
pub(crate) mod process_cb;
pub mod resource;

/// High-level audio engine.
///
/// `Engine` is the main entry point for playback and mixing. Internally it wraps
/// a `ma_engine` from miniaudio, which owns (or coordinates) the output device,
/// the engine’s node graph, and the global engine clock.
///
/// Most users will:
/// - create an [`Engine`]
/// - load or create sounds
/// - control playback and volume
/// - optionally interact with the engine’s endpoint node / node graph for effects
pub struct Engine(pub(crate) Arc<EngineInner>);

#[doc(hidden)]
pub struct EngineInner {
    inner: *mut sys::ma_engine,
    _playback_device_id: Option<DeviceId>,  // keep alive
    _device: Option<Arc<DeviceInner<f32>>>, // keep alive
    _resource_manager: Option<ResourceManager<f32>>, // keep alive
    process_data_ptr: Option<*mut ProcessState>, // userdata (self.inner.pProcessUserData)
    process_data_panic: Option<Arc<AtomicBool>>, // true = callback panicked and is now poisoned
    process_data_notif: Option<ProcFramesNotif>,
    state_notifier: Option<DeviceStateNotifier>,
    reader_exists: Arc<AtomicBool>,
}

unsafe impl Send for EngineInner {}
unsafe impl Sync for EngineInner {}

impl Binding for Engine {
    type Raw = *mut sys::ma_engine;

    fn to_raw(&self) -> Self::Raw {
        self.0.inner
    }
}

/// Dedicated type for reading frames from the Engine
///
/// Concurrent calls to `read_pcm_frames` are not safe. This type exists to enforce this.
pub struct EngineReader(Arc<EngineInner>);

unsafe impl Send for EngineReader {}

impl Binding for EngineReader {
    type Raw = *mut sys::ma_engine;

    fn to_raw(&self) -> Self::Raw {
        self.0.inner
    }
}
impl EngineReader {
    /// Reads PCM frames into `dst`, returning the number of frames read.
    pub fn read_pcm_frames_into(&mut self, dst: &mut [f32]) -> MaResult<usize> {
        if engine_ffi::ma_engine_get_device(self).is_some() {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "read_pcm_frames is not allowed when engine has a device",
            )));
        }
        engine_ffi::ma_engine_read_pcm_frames_into(self, dst)
    }

    /// This function pulls audio from the engine’s internal node graph and returns
    /// up to `frame_count` frames of interleaved PCM samples.
    ///
    /// - This is a **pull-based render operation**.
    /// - The engine will attempt to render `frame_count` frames, but it may return
    ///   **fewer frames**.
    pub fn read_pcm_frames(&mut self, frame_count: u64) -> MaResult<SampleBuffer<f32>> {
        if engine_ffi::ma_engine_get_device(self).is_some() {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "read_pcm_frames is not allowed when engine has a device",
            )));
        }
        engine_ffi::ma_engine_read_pcm_frames(self, frame_count)
    }
}

pub(crate) mod private_engine {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait EnginePtrProvider<T: ?Sized> {
        fn as_engine_ptr(t: &T) -> *mut sys::ma_engine;
    }

    pub struct EngineProvider;
    pub struct EngineReaderProvider;

    impl EnginePtrProvider<Engine> for EngineProvider {
        #[inline]
        fn as_engine_ptr(t: &Engine) -> *mut sys::ma_engine {
            t.to_raw()
        }
    }

    impl EnginePtrProvider<EngineReader> for EngineReaderProvider {
        #[inline]
        fn as_engine_ptr(t: &EngineReader) -> *mut sys::ma_engine {
            t.to_raw()
        }
    }

    pub fn engine_ptr<T: AsEnginePtr + ?Sized>(t: &T) -> *mut sys::ma_engine {
        <T as AsEnginePtr>::__PtrProvider::as_engine_ptr(t)
    }
}

#[doc(hidden)]
pub trait AsEnginePtr {
    type __PtrProvider: private_engine::EnginePtrProvider<Self>;
}

#[doc(hidden)]
impl AsEnginePtr for Engine {
    type __PtrProvider = private_engine::EngineProvider;
}

#[doc(hidden)]
impl AsEnginePtr for EngineReader {
    type __PtrProvider = private_engine::EngineReaderProvider;
}

impl Engine {
    /// Creates a new engine using the default configuration.
    ///
    /// This is a convenience constructor equivalent to using
    /// an [`EngineBuilder`] (`ma_engine_config`) with a default configuration.
    ///
    /// Most applications should start with this method.
    pub fn new() -> MaResult<Self> {
        Self::new_with_config(None)
    }

    /// Retrieves a [`ProcFramesNotif`] if one is present.
    ///
    /// `ProcFramesNotif` is cheap to clone, and this function can be safely called multiple times
    pub fn get_data_notifier(&self) -> Option<ProcFramesNotif> {
        self.0.process_data_notif.clone()
    }

    /// Checks if the data onProcess callback is poisoned
    pub fn data_callback_panicked(&self) -> bool {
        match &self.0.process_data_panic {
            Some(flag) => flag.load(std::sync::atomic::Ordering::Relaxed),
            None => false,
        }
    }

    /// Retrieves a [`DeviceStateNotifier`] if one is present, that fires when the state of the device is changed
    ///
    /// `DeviceStateNotifier` is cheap to clone, and this function can be safely called multiple times
    pub fn get_state_notifier(&self) -> Option<DeviceStateNotifier> {
        self.0.state_notifier.clone()
    }

    fn new_with_config(config: Option<&EngineBuilder>) -> MaResult<Self> {
        let (device, rm, dev_id) = config.map_or((None, None, None), |c| {
            (
                c.device.clone(),
                c.resource_manager.clone(),
                c.playback_device_id.clone(),
            )
        });
        let mut mem: Box<MaybeUninit<sys::ma_engine>> = Box::new(MaybeUninit::uninit());
        engine_ffi::engine_init(config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_engine = Box::into_raw(mem) as *mut sys::ma_engine;
        Ok(Self(Arc::new(EngineInner {
            inner,
            _playback_device_id: dev_id,
            _device: device,
            _resource_manager: rm,
            process_data_ptr: None,
            process_data_panic: None,
            process_data_notif: None,
            state_notifier: None,
            reader_exists: Arc::new(AtomicBool::new(false)),
        })))
    }

    fn new_with_process_data(
        config: &mut EngineBuilder,
        data_notif: Option<ProcFramesNotif>,
    ) -> MaResult<Self> {
        let state_notif = if config.inner.noDevice == 0 && config.process_data.state_notif_exists {
            config.inner.notificationCallback = Some(engine_notification_callback);
            config.process_data.state_notif.take()
        } else {
            None
        };

        let mut mem: Box<MaybeUninit<sys::ma_engine>> = Box::new(MaybeUninit::uninit());
        engine_ffi::engine_init(Some(config), mem.as_mut_ptr())?;

        let inner: *mut sys::ma_engine = Box::into_raw(mem) as *mut sys::ma_engine;
        Ok(Self(Arc::new(EngineInner {
            inner,
            _playback_device_id: config.playback_device_id.take(),
            _device: config.device.take(),
            _resource_manager: config.resource_manager.take(),
            process_data_ptr: config.process_data.process_data_ptr,
            process_data_panic: config.process_data.process_data_panic.take(),
            process_data_notif: data_notif,
            state_notifier: state_notif,
            reader_exists: Arc::new(AtomicBool::new(false)),
        })))
    }

    /// Equivalent to calling [`SoundBuilder::new()`]
    pub fn sound_config<'a, 'b>(&'a self) -> SoundBuilder<'a, 'b> {
        SoundBuilder::init(self)
    }

    /// Creates an empty sound node with no audio source.
    ///
    /// Unlike sounds created from a file or data source, this object does not
    /// produce audio by itself. It is mainly useful as an intermediate node in
    /// the engine's node graph, where other sounds or nodes can be attached to it.
    pub fn new_sound(&self) -> MaResult<Sound> {
        self.new_sound_with_config_internal(None)
    }

    pub fn new_sound_from_file(&self, path: &Path) -> MaResult<Sound> {
        self.new_sound_with_file_internal(path, SoundFlags::NONE, None, None)
    }

    pub fn new_sound_from_source<D: AsSourcePtr + ?Sized>(&self, source: &D) -> MaResult<Sound> {
        self.new_sound_with_source_internal(SoundFlags::NONE, None, source)
    }

    pub fn clone_sound(&self, sound: &Sound, flags: SoundFlags) -> MaResult<Sound> {
        self.new_sound_instance_internal(sound, flags, None)
    }

    // Thread-safe
    /// Manually starts the engine
    ///
    /// By default, an engine will be created with `no_auto_start` to false.
    /// Setting [`EngineBuilder::no_auto_start()`] will require a manual start
    ///
    /// Start and stop operations on an engine with no device will result in an error
    pub fn start(&self) -> MaResult<()> {
        engine_ffi::ma_engine_start(self)
    }

    // Thread-safe
    /// Manually stops the engine
    ///
    /// Start and stop operations on an engine with no device will result in an error
    pub fn stop(&self) -> MaResult<()> {
        engine_ffi::ma_engine_stop(self)
    }

    pub fn new_sound_from_file_with_group(
        &self,
        path: &Path,
        sound_group: &SoundGroup,
        done_fence: Option<Fence>,
    ) -> MaResult<Sound> {
        self.new_sound_with_file_internal(path, SoundFlags::NONE, Some(sound_group), done_fence)
    }

    /// Adding a Fence also requires setting the [`SoundFlags::ASYNC`] flag
    pub fn new_sound_from_file_with_flags(
        &self,
        path: &Path,
        flags: SoundFlags,
        done_fence: Option<Fence>,
    ) -> MaResult<Sound> {
        self.new_sound_with_file_internal(path, flags, None, done_fence)
    }

    /// Convenience method for `SoundGroupBuilder::new(&engine).build()`
    pub fn new_sound_group(&self) -> MaResult<SoundGroup> {
        SoundGroupBuilder::new(self).build()
    }

    // Thread-safe
    /// Sets the master volume (of the output node).
    pub fn set_volume(&self, volume: f32) -> MaResult<()> {
        engine_ffi::ma_engine_set_volume(self, volume)
    }

    /// Returns the master volume.
    pub fn volume(&self) -> f32 {
        engine_ffi::ma_engine_get_volume(self)
    }

    // Thread-safe
    /// Sets the master gain in dB.
    pub fn set_gain_db(&self, db_gain: f32) -> MaResult<()> {
        engine_ffi::ma_engine_set_gain_db(self, db_gain)
    }

    /// Returns the master gain in dB.
    pub fn gain_db(&self) -> f32 {
        engine_ffi::ma_engine_get_gain_db(self)
    }

    /// Returns the number of listeners.
    pub fn listener_count(&self) -> u32 {
        engine_ffi::ma_engine_get_listener_count(self)
    }

    /// Returns the index of the closest listener to `position`.
    pub fn closest_listener(&self, position: Vec3) -> u32 {
        engine_ffi::ma_engine_find_closest_listener(self, position)
    }

    // Thread-safe
    /// Sets the position of `listener`.
    pub fn set_position(&self, listener: u32, position: Vec3) {
        engine_ffi::ma_engine_listener_set_position(self, listener, position);
    }

    /// Returns the position of `listener`.
    pub fn position(&self, listener: u32) -> Vec3 {
        engine_ffi::ma_engine_listener_get_position(self, listener)
    }

    // Thread-safe
    /// Sets the facing direction of `listener`.
    pub fn set_direction(&self, listener: u32, direction: Vec3) {
        engine_ffi::ma_engine_listener_set_direction(self, listener, direction);
    }

    /// Returns the facing direction of `listener`.
    pub fn direction(&self, listener: u32) -> Vec3 {
        engine_ffi::ma_engine_listener_get_direction(self, listener)
    }

    // Thread-safe
    /// Sets the velocity of `listener`.
    pub fn set_velocity(&self, listener: u32, position: Vec3) {
        engine_ffi::ma_engine_listener_set_velocity(self, listener, position);
    }

    /// Returns the velocity of `listener`.
    pub fn velocity(&self, listener: u32) -> Vec3 {
        engine_ffi::ma_engine_listener_get_velocity(self, listener)
    }

    // Thread-safe
    /// Sets the directional cone of `listener`.
    pub fn set_cone(&self, listener: u32, cone: Cone) {
        engine_ffi::ma_engine_listener_set_cone(self, listener, cone);
    }

    /// Returns the directional cone of `listener`.
    pub fn cone(&self, listener: u32) -> Cone {
        engine_ffi::ma_engine_listener_get_cone(self, listener)
    }

    // Thread-safe
    /// Sets the world-up vector of `listener`.
    pub fn set_world_up(&self, listener: u32, up_direction: Vec3) {
        engine_ffi::ma_engine_listener_set_world_up(self, listener, up_direction);
    }

    /// Returns the world-up vector of `listener`.
    pub fn get_world_up(&self, listener: u32) -> Vec3 {
        engine_ffi::ma_engine_listener_get_world_up(self, listener)
    }

    pub fn try_acquire_reader(&self) -> MaResult<EngineReader> {
        match self.0.reader_exists.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(EngineReader(self.0.clone())),
            Err(_) => Err(MaudioError::new_ma_error(ErrorKinds::ReaderExists)),
        }
    }

    // Thread-safe
    /// Enables or disables `listener`.
    pub fn toggle_listener(&self, listener: u32, enabled: bool) {
        engine_ffi::ma_engine_listener_set_enabled(self, listener, enabled);
    }

    /// Returns `true` if `listener` is enabled.
    pub fn listener_enabled(&self, listener: u32) -> bool {
        engine_ffi::ma_engine_listener_is_enabled(self, listener)
    }

    /// Returns the engine's internal node graph.
    pub fn as_node_graph(&self) -> NodeGraphRef {
        engine_ffi::ma_engine_get_node_graph(self)
    }

    /// Returns the engine's internal resource manager, if available.
    pub fn resource_manager(&self) -> Option<ResourceManagerRef<'_, f32>> {
        engine_ffi::ma_engine_get_resource_manager(self)
    }

    /// Returns the engine's internal device, if available
    pub fn device(&self) -> Option<DeviceRef<'_>> {
        engine_ffi::ma_engine_get_device(self)
    }

    /// Returns the engine’s **endpoint node**.
    ///
    /// The endpoint node is the final node in the engine’s internal node graph.
    /// All sounds ultimately connect to this node before audio is sent to the
    /// output device.
    pub fn endpoint(&self) -> NodeRef<'_> {
        engine_ffi::ma_engine_get_endpoint(self)
    }

    /// Returns the current local time (in PCM frames) of the output node.
    pub fn time_pcm(&self) -> u64 {
        engine_ffi::ma_engine_get_time_in_pcm_frames(self)
    }

    /// Returns the current local time (in PCM frames) of the output node.
    ///
    /// For sample-accurate work, prefer [`Engine::time_pcm()`].
    pub fn time_mili(&self) -> u64 {
        engine_ffi::ma_engine_get_time_in_milliseconds(self)
    }

    // Thread-safe
    /// Sets the current local time (in PCM frames) of the output node.
    pub fn set_time_pcm(&self, time: u64) {
        engine_ffi::ma_engine_set_time_in_pcm_frames(self, time);
    }

    // Thread-safe
    /// Sets the current local time (in PCM frames) of the output node.
    ///
    /// Precision may be lower than [`Engine::set_time_pcm()`].
    pub fn set_time_mili(&self, time: u64) {
        engine_ffi::ma_engine_set_time_in_milliseconds(self, time);
    }

    /// Returns the number of output **channels** used by the engine.
    /// and output device.
    pub fn channels(&self) -> u32 {
        engine_ffi::ma_engine_get_channels(self)
    }

    /// Returns the engine’s **sample rate**, in Hz.
    pub fn sample_rate(&self) -> MaResult<SampleRate> {
        let res = engine_ffi::ma_engine_get_sample_rate(self);
        res.try_into()
    }
}

// Private mathods
impl Engine {
    fn new_sound_instance_internal(
        &self,
        sound: &Sound,
        flags: SoundFlags,
        sound_group: Option<&mut SoundGroup>,
    ) -> MaResult<Sound> {
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new(MaybeUninit::uninit());

        sound_ffi::ma_sound_init_copy(self, sound, flags, sound_group, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_sound = Box::into_raw(mem) as *mut sys::ma_sound;
        Ok(Sound::new_sound(inner, self.0.clone(), None, None))
    }

    pub(crate) fn sample_rate_u32(&self) -> u32 {
        engine_ffi::ma_engine_get_sample_rate(self)
    }

    #[allow(dead_code)]
    pub(crate) fn new_for_tests() -> MaResult<Self> {
        if cfg!(feature = "ci-tests") {
            EngineBuilder::new()
                .no_device(2, SampleRate::Sr44100)
                .build()
        } else {
            Engine::new()
        }
    }

    pub(crate) fn new_sound_with_config_internal(
        &self,
        config: Option<&SoundBuilder>,
    ) -> MaResult<Sound> {
        let temp_config = &SoundBuilder::init(self);
        let config = config.unwrap_or(temp_config);
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new(MaybeUninit::uninit());

        sound_ffi::ma_sound_init_ex(self, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_sound = Box::into_raw(mem) as *mut sys::ma_sound;
        Ok(Sound::new_sound(
            inner,
            self.0.clone(),
            config.fence.clone(),
            config.end_notifier.clone(),
        ))
    }

    pub(crate) fn new_sound_with_source_internal<D: AsSourcePtr + ?Sized>(
        &self,
        flags: SoundFlags,
        sound_group: Option<&SoundGroup>,
        data_source: &D,
    ) -> MaResult<Sound> {
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new(MaybeUninit::uninit());

        sound_ffi::ma_sound_init_from_data_source(
            self,
            data_source,
            flags,
            sound_group,
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_sound = Box::into_raw(mem) as *mut sys::ma_sound;
        Ok(Sound::new_sound(inner, self.0.clone(), None, None))
    }

    pub(crate) fn new_sound_with_file_internal(
        &self,
        path: &Path,
        flags: SoundFlags,
        sound_group: Option<&SoundGroup>,
        done_fence: Option<Fence>,
    ) -> MaResult<Sound> {
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new(MaybeUninit::uninit());

        Sound::init_from_file_internal(
            mem.as_mut_ptr(),
            self,
            path,
            flags,
            sound_group,
            done_fence,
        )?;

        let inner: *mut sys::ma_sound = Box::into_raw(mem) as *mut sys::ma_sound;
        Ok(Sound::new_sound(inner, self.0.clone(), None, None))
    }
}

impl Drop for EngineInner {
    fn drop(&mut self) {
        engine_ffi::engine_uninit(self);
        if let Some(proc_data_ptr) = self.process_data_ptr {
            drop(unsafe { Box::from_raw(proc_data_ptr) });
        }
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

impl Drop for EngineReader {
    fn drop(&mut self) {
        self.0.reader_exists.store(false, Ordering::Relaxed);
    }
}

#[cfg(unix)]
pub(crate) fn cstring_from_path(path: &Path) -> MaResult<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| crate::MaudioError::new_ma_error(crate::ErrorKinds::InvalidCString))
}

#[cfg(windows)]
pub(crate) fn wide_null_terminated(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
pub(crate) fn wide_null_terminated_name(name: &str) -> Vec<u16> {
    use std::os::windows::prelude::OsStrExt;

    std::ffi::OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
/// Custom memory allocation callbacks for miniaudio.
///
/// Miniaudio allows callers to override how heap memory is allocated and freed
/// by providing a `ma_allocation_callbacks` struct (malloc/realloc/free + user data).
///
/// Types such as `NodeGraph` may accept these callbacks at initialization time.
/// If callbacks are not provided, miniaudio uses its default allocator
/// (typically the system allocator).
///
/// Custom allocators are currently not implemented.
pub(crate) struct AllocationCallbacks {
    inner: sys::ma_allocation_callbacks,
}

impl AsRawRef for AllocationCallbacks {
    type Raw = sys::ma_allocation_callbacks;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

pub(crate) mod engine_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        audio::{formats::SampleBuffer, math::vec3::Vec3, spatial::cone::Cone},
        device::DeviceRef,
        engine::{
            engine_builder::EngineBuilder,
            engine_ffi,
            node_graph::{nodes::NodeRef, GraphOwner, NodeGraphRef},
            private_engine,
            resource::ResourceManagerRef,
            AsEnginePtr, Binding, Engine, EngineInner, EngineReader,
        },
        AsRawRef, MaResult, MaudioError,
    };

    #[inline]
    pub fn engine_init(
        config: Option<&EngineBuilder>,
        engine: *mut sys::ma_engine,
    ) -> MaResult<()> {
        let p_config: *const sys::ma_engine_config =
            config.map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_engine_init(p_config, engine) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn engine_uninit(engine: &mut EngineInner) {
        unsafe {
            sys::ma_engine_uninit(engine.inner);
        }
    }

    #[inline]
    pub fn ma_engine_read_pcm_frames_into(
        engine: &EngineReader,
        dst: &mut [f32],
    ) -> MaResult<usize> {
        let channels = engine_ffi::ma_engine_get_channels(engine);
        let len = dst.len() as u64;

        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        // May truncate, and that is desired
        let frame_count = len / channels as u64;

        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_engine_read_pcm_frames(
                engine.to_raw(),
                dst.as_mut_ptr() as *mut std::ffi::c_void,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;

        Ok(frames_read as usize)
    }

    #[inline]
    pub fn ma_engine_read_pcm_frames(
        engine: &EngineReader,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<f32>> {
        let channels = engine_ffi::ma_engine_get_channels(engine);
        let mut buffer = vec![0.0f32; (frame_count * channels as u64) as usize];
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_engine_read_pcm_frames(
                engine.to_raw(),
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        SampleBuffer::<f32>::from_storage(buffer, frames_read as usize, channels)
    }

    #[inline]
    pub fn ma_engine_get_node_graph(engine: &Engine) -> NodeGraphRef {
        let ptr = unsafe { sys::ma_engine_get_node_graph(engine.to_raw()) };
        NodeGraphRef {
            inner: ptr,
            owner: GraphOwner::Engine(engine.0.clone()),
        }
    }

    #[inline]
    pub fn ma_engine_get_resource_manager<'a>(
        engine: &'a Engine,
    ) -> Option<ResourceManagerRef<'a, f32>> {
        let ptr = unsafe { sys::ma_engine_get_resource_manager(engine.to_raw()) };
        if ptr.is_null() {
            None
        } else {
            Some(ResourceManagerRef::from_ptr(ptr))
        }
    }

    // AsEnginePtr
    #[inline]
    pub fn ma_engine_get_device<'a, E: AsEnginePtr + ?Sized>(
        engine: &'a E,
    ) -> Option<DeviceRef<'a>> {
        let ptr = unsafe { sys::ma_engine_get_device(private_engine::engine_ptr(engine)) };
        if ptr.is_null() {
            None
        } else {
            Some(DeviceRef::from_ptr(ptr))
        }
    }

    // TODO: Implement Log(Ref?)
    #[inline]
    #[allow(dead_code)]
    pub fn ma_engine_get_log(engine: &Engine) -> *mut sys::ma_log {
        unsafe { sys::ma_engine_get_log(engine.to_raw()) }
    }

    #[inline]
    pub fn ma_engine_get_endpoint<'a>(engine: &'a Engine) -> NodeRef<'a> {
        let ptr = unsafe { sys::ma_engine_get_endpoint(engine.to_raw()) };
        NodeRef::from_ptr(ptr)
    }

    #[inline]
    pub fn ma_engine_get_time_in_pcm_frames(engine: &Engine) -> u64 {
        unsafe { sys::ma_engine_get_time_in_pcm_frames(engine.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_engine_get_time_in_milliseconds(engine: &Engine) -> u64 {
        unsafe { sys::ma_engine_get_time_in_milliseconds(engine.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_engine_set_time_in_pcm_frames(engine: &Engine, time: u64) {
        unsafe { sys::ma_engine_set_time_in_pcm_frames(engine.to_raw(), time) };
    }

    #[inline]
    pub fn ma_engine_set_time_in_milliseconds(engine: &Engine, time: u64) {
        unsafe { sys::ma_engine_set_time_in_milliseconds(engine.to_raw(), time) };
    }

    #[inline]
    pub fn ma_engine_get_channels<E: AsEnginePtr + ?Sized>(engine: &E) -> u32 {
        unsafe { sys::ma_engine_get_channels(private_engine::engine_ptr(engine) as *const _) }
    }

    #[inline]
    pub fn ma_engine_get_sample_rate(engine: &Engine) -> u32 {
        unsafe { sys::ma_engine_get_sample_rate(engine.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_engine_start(engine: &Engine) -> MaResult<()> {
        let res = unsafe { sys::ma_engine_start(engine.to_raw()) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_engine_stop(engine: &Engine) -> MaResult<()> {
        let res = unsafe { sys::ma_engine_stop(engine.to_raw()) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_engine_set_volume(engine: &Engine, volume: f32) -> MaResult<()> {
        let res = unsafe { sys::ma_engine_set_volume(engine.to_raw(), volume) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_engine_get_volume(engine: &Engine) -> f32 {
        unsafe { sys::ma_engine_get_volume(engine.to_raw()) }
    }

    #[inline]
    pub fn ma_engine_set_gain_db(engine: &Engine, db_gain: f32) -> MaResult<()> {
        let res = unsafe { sys::ma_engine_set_gain_db(engine.to_raw(), db_gain) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_engine_get_gain_db(engine: &Engine) -> f32 {
        unsafe { sys::ma_engine_get_gain_db(engine.to_raw()) }
    }

    #[inline]
    pub fn ma_engine_get_listener_count(engine: &Engine) -> u32 {
        unsafe { sys::ma_engine_get_listener_count(engine.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_engine_find_closest_listener(engine: &Engine, position: Vec3) -> u32 {
        unsafe {
            sys::ma_engine_find_closest_listener(
                engine.to_raw() as *const _,
                position.x,
                position.y,
                position.z,
            )
        }
    }

    #[inline]
    pub fn ma_engine_listener_set_position(engine: &Engine, listener: u32, position: Vec3) {
        unsafe {
            sys::ma_engine_listener_set_position(
                engine.to_raw(),
                listener,
                position.x,
                position.y,
                position.z,
            )
        };
    }

    #[inline]
    pub fn ma_engine_listener_get_position(engine: &Engine, listener: u32) -> Vec3 {
        let vec =
            unsafe { sys::ma_engine_listener_get_position(engine.to_raw() as *const _, listener) };
        vec.into()
    }

    #[inline]
    pub fn ma_engine_listener_set_direction(engine: &Engine, listener: u32, position: Vec3) {
        unsafe {
            sys::ma_engine_listener_set_direction(
                engine.to_raw(),
                listener,
                position.x,
                position.y,
                position.z,
            )
        };
    }

    #[inline]
    pub fn ma_engine_listener_get_direction(engine: &Engine, listener: u32) -> Vec3 {
        let vec =
            unsafe { sys::ma_engine_listener_get_direction(engine.to_raw() as *const _, listener) };
        vec.into()
    }

    #[inline]
    pub fn ma_engine_listener_set_velocity(engine: &Engine, listener: u32, position: Vec3) {
        unsafe {
            sys::ma_engine_listener_set_velocity(
                engine.to_raw(),
                listener,
                position.x,
                position.y,
                position.z,
            )
        };
    }

    #[inline]
    pub fn ma_engine_listener_get_velocity(engine: &Engine, listener: u32) -> Vec3 {
        let vec =
            unsafe { sys::ma_engine_listener_get_velocity(engine.to_raw() as *const _, listener) };
        vec.into()
    }

    #[inline]
    pub fn ma_engine_listener_set_cone(engine: &Engine, listener: u32, cone: Cone) {
        unsafe {
            sys::ma_engine_listener_set_cone(
                engine.to_raw(),
                listener,
                cone.inner_angle_rad,
                cone.outer_angle_rad,
                cone.outer_gain,
            )
        };
    }

    #[inline]
    pub fn ma_engine_listener_get_cone(engine: &Engine, listener: u32) -> Cone {
        let mut inner = 0.0f32;
        let mut outer = 0.0f32;
        let mut gain = 1.0f32;

        unsafe {
            sys::ma_engine_listener_get_cone(
                engine.to_raw() as *const _,
                listener,
                &mut inner,
                &mut outer,
                &mut gain,
            )
        };

        Cone {
            inner_angle_rad: inner,
            outer_angle_rad: outer,
            outer_gain: gain,
        }
    }

    #[inline]
    pub fn ma_engine_listener_set_world_up(engine: &Engine, listener: u32, vec: Vec3) {
        unsafe {
            sys::ma_engine_listener_set_world_up(engine.to_raw(), listener, vec.x, vec.y, vec.z);
        }
    }

    #[inline]
    pub fn ma_engine_listener_get_world_up(engine: &Engine, listener: u32) -> Vec3 {
        let vec =
            unsafe { sys::ma_engine_listener_get_world_up(engine.to_raw() as *const _, listener) };
        vec.into()
    }

    #[inline]
    pub fn ma_engine_listener_set_enabled(engine: &Engine, listener: u32, enabled: bool) {
        unsafe { sys::ma_engine_listener_set_enabled(engine.to_raw(), listener, enabled as u32) }
    }

    #[inline]
    pub fn ma_engine_listener_is_enabled(engine: &Engine, listener: u32) -> bool {
        let res =
            unsafe { sys::ma_engine_listener_is_enabled(engine.to_raw() as *const _, listener) };
        res == 1
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_engine_acquire_reader() {
        let engine = Engine::new_for_tests().unwrap();
        let res = engine.try_acquire_reader();
        assert!(res.is_ok());
    }

    #[test]
    fn test_engine_acquire_reader_twice() {
        let engine = Engine::new_for_tests().unwrap();
        let _reader = engine.try_acquire_reader().unwrap();

        let res = engine.try_acquire_reader();
        assert!(res.is_err());
    }

    #[test]
    fn test_engine_acquire_reader_drop_reacquire() {
        let engine = Engine::new_for_tests().unwrap();
        let reader = engine.try_acquire_reader().unwrap();
        drop(reader);

        let res = engine.try_acquire_reader();
        assert!(res.is_ok());
    }

    use super::*;

    fn assert_f32_eq(a: f32, b: f32) {
        assert!(
            (a - b).abs() <= 1.0e-6,
            "expected {a} ~= {b}, diff={}",
            (a - b).abs()
        );
    }

    #[test]
    fn engine_test_works_with_default() {
        let _engine = Engine::new_for_tests().unwrap();
    }

    fn assert_vec3_eq(a: Vec3, b: Vec3) {
        assert_f32_eq(a.x, b.x);
        assert_f32_eq(a.y, b.y);
        assert_f32_eq(a.z, b.z);
    }

    #[test]
    fn test_engine_test_init_engine_and_sound() {
        let engine = Engine::new_for_tests().unwrap();
        let _sound = engine.new_sound().unwrap();
    }

    #[test]
    fn test_engine_volume_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();

        engine.set_volume(0.25).unwrap();
        assert_f32_eq(engine.volume(), 0.25);

        engine.set_volume(1.0).unwrap();
        assert_f32_eq(engine.volume(), 1.0);
    }

    #[test]
    fn test_engine_gain_db_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();

        engine.set_gain_db(-6.0).unwrap();
        assert_f32_eq(engine.gain_db(), -6.0);

        engine.set_gain_db(0.0).unwrap();
        assert_f32_eq(engine.gain_db(), 0.0);
    }

    #[test]
    fn test_engine_listener_count_and_enabled_toggle() {
        let engine = Engine::new_for_tests().unwrap();

        let n = engine.listener_count();
        assert!(n >= 1, "engine should have at least 1 listener");

        // Toggle first listener (should always exist if n>=1).
        engine.toggle_listener(0, false);
        assert!(!engine.listener_enabled(0));

        engine.toggle_listener(0, true);
        assert!(engine.listener_enabled(0));
    }

    #[test]
    fn test_engine_listener_position_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();

        let p = Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        engine.set_position(0, p);

        let got = engine.position(0);
        assert_vec3_eq(got, p);
    }

    #[test]
    fn test_engine_listener_velocity_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();

        let v = Vec3 {
            x: -1.0,
            y: 0.5,
            z: 10.0,
        };
        engine.set_velocity(0, v);

        let got = engine.velocity(0);
        assert_vec3_eq(got, v);
    }

    #[test]
    fn test_engine_listener_world_up_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();

        let up = Vec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        };
        engine.set_world_up(0, up);

        let got = engine.get_world_up(0);
        assert_vec3_eq(got, up);
    }

    #[test]
    fn test_engine_listener_cone_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();

        // Adjust field names if your Cone differs; the point is roundtripping.
        let cone = Cone {
            inner_angle_rad: 0.5,
            outer_angle_rad: 1.0,
            outer_gain: 0.25,
        };

        engine.set_cone(0, cone);
        let got = engine.cone(0);

        assert_f32_eq(got.inner_angle_rad, cone.inner_angle_rad);
        assert_f32_eq(got.outer_angle_rad, cone.outer_angle_rad);
        assert_f32_eq(got.outer_gain, cone.outer_gain);
    }

    #[test]
    fn test_engine_closest_listener_basic() {
        let engine = Engine::new_for_tests().unwrap();

        // If only 1 listener, the only valid answer is 0.
        let n = engine.listener_count();
        if n < 2 {
            let idx = engine.closest_listener(Vec3 {
                x: 100.0,
                y: 0.0,
                z: 0.0,
            });
            assert_eq!(idx, 0);
            return;
        }

        // If >=2 listeners, we can make a meaningful test.
        engine.set_position(
            0,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        );
        engine.set_position(
            1,
            Vec3 {
                x: 1000.0,
                y: 0.0,
                z: 0.0,
            },
        );

        let idx = engine.closest_listener(Vec3 {
            x: 0.1,
            y: 0.0,
            z: 0.0,
        });
        assert_eq!(idx, 0);

        let idx = engine.closest_listener(Vec3 {
            x: 999.9,
            y: 0.0,
            z: 0.0,
        });
        assert_eq!(idx, 1);
    }

    #[test]
    fn test_engine_read_pcm_frames_shapes_output() {
        let engine = EngineBuilder::new()
            .no_device(2, SampleRate::Sr44100)
            .build()
            .unwrap();

        let requested = 256u64;
        let mut reader = engine.try_acquire_reader().unwrap();
        let buffer = reader.read_pcm_frames(requested).unwrap();
        let frames = buffer.frames() as u64;
        let samples = buffer.as_ref();

        assert!(
            frames <= requested,
            "engine returned more frames than requested"
        );

        let channels = engine.channels() as u64;
        assert!(channels >= 1);

        let expected_len = (frames * channels) as usize;
        assert_eq!(
            samples.len(),
            expected_len,
            "samples must be interleaved: len == frames * channels"
        );
    }

    #[test]
    fn test_engine_time_pcm_set_get() {
        let engine = Engine::new_for_tests().unwrap();

        engine.set_time_pcm(12345);
        assert_eq!(engine.time_pcm(), 12345);

        engine.set_time_pcm(0);
        assert_eq!(engine.time_pcm(), 0);
    }

    #[test]
    fn test_engine_time_mili_set_get() {
        let engine = Engine::new_for_tests().unwrap();

        engine.set_time_mili(500);
        assert_eq!(engine.time_mili(), 500);

        engine.set_time_mili(0);
        assert_eq!(engine.time_mili(), 0);
    }

    #[test]
    fn test_engine_channels_and_sample_rate_are_sane() {
        let engine = Engine::new_for_tests().unwrap();

        let ch = engine.channels();
        let sr = engine.sample_rate().unwrap();
        let sr = u32::from(sr);

        assert!(ch >= 1, "channels must be >= 1");
        assert!(sr >= 8000, "sample rate looks wrong: {sr}");
    }

    #[test]
    fn test_engine_listener_direction_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();

        let dir = Vec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        };
        engine.set_direction(0, dir);

        let got = engine.direction(0);
        assert_vec3_eq(got, dir);
    }
}
