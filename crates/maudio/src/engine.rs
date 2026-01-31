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
//! Advanced users can access the endpoint node via [`EngineOps::endpoint()`] to
//! attach custom processing or inspect the graph.
//!
//! ## Time
//! The engine maintains a global timeline that advances as audio is processed.
//! Time can be queried or modified in either PCM frames or milliseconds.
//!
//! For sample-accurate control, prefer the PCM-frame APIs.
//!
//! ## Threading
//! The engine runs an internal audio callback on a real-time thread. Care should
//! be taken to avoid heavy work or allocations in contexts that must remain
//! real-time safe.
use std::{cell::Cell, marker::PhantomData, mem::MaybeUninit, path::Path, sync::Arc};

use crate::{
    audio::{math::vec3::Vec3, sample_rate::SampleRate, spatial::cone::Cone},
    data_source::AsSourcePtr,
    engine::{
        engine_builder::EngineBuilder,
        node_graph::{nodes::NodeRef, NodeGraphRef},
        process_notifier::ProcessState,
    },
    sound::{
        sound_builder::SoundBuilder,
        sound_ffi,
        sound_flags::SoundFlags,
        sound_group::{s_group_cfg_ffi, s_group_ffi, SoundGroup, SoundGroupConfig},
        Sound,
    },
    util::fence::Fence,
    Binding, MaResult,
};

use maudio_sys::ffi as sys;

pub mod engine_builder;
#[cfg(feature = "engine_host")]
pub mod engine_host;

pub mod node_graph;
pub mod process_notifier;

/// Prelude for the [`engine`](super) module.
///
/// This module re-exports the most commonly used engine types and traits
/// so they can be imported with a single global import.
///
/// Import this when you want access to [`Engine`] and [`EngineRef`] and all shared engine
/// methods (provided by [`EngineOps`]) without having to import each item
/// individually.
/// This is purely a convenience module; importing directly
/// works just as well if you prefer explicit imports.
pub mod prelude {
    pub use super::{Engine, EngineOps};
}

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
pub struct Engine {
    inner: *mut sys::ma_engine,
    process_notifier: Option<Arc<ProcessState>>,
    _not_sync: PhantomData<Cell<()>>,
}

impl Binding for Engine {
    type Raw = *mut sys::ma_engine;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[derive(Clone, Copy)]
pub struct EngineRef<'a> {
    ptr: *mut sys::ma_engine,
    _marker: PhantomData<&'a ()>,
    _not_sync: PhantomData<Cell<()>>,
}

unsafe impl Send for Engine {}

impl<'a> Binding for EngineRef<'a> {
    type Raw = *mut sys::ma_engine;

    fn from_ptr(raw: Self::Raw) -> Self {
        EngineRef {
            ptr: raw,
            _marker: PhantomData,
            _not_sync: PhantomData,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.ptr
    }
}

pub(crate) mod private_engine {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait EnginePtrProvider<T: ?Sized> {
        fn as_engine_ptr(t: &T) -> *mut sys::ma_engine;
    }

    pub struct EngineProvider;
    pub struct EngineRefProvider;

    impl EnginePtrProvider<Engine> for EngineProvider {
        #[inline]
        fn as_engine_ptr(t: &Engine) -> *mut sys::ma_engine {
            t.to_raw()
        }
    }

    impl<'a> EnginePtrProvider<EngineRef<'a>> for EngineRefProvider {
        fn as_engine_ptr(t: &EngineRef<'a>) -> *mut sys::ma_engine {
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
impl AsEnginePtr for EngineRef<'_> {
    type __PtrProvider = private_engine::EngineRefProvider;
}

impl<T: AsEnginePtr + ?Sized> EngineOps for T {}

pub trait EngineOps: AsEnginePtr {
    fn set_volume(&self, volume: f32) -> MaResult<()> {
        engine_ffi::ma_engine_set_volume(self, volume)
    }

    fn volume(&self) -> f32 {
        engine_ffi::ma_engine_get_volume(self)
    }

    fn set_gain_db(&self, db_gain: f32) -> MaResult<()> {
        engine_ffi::ma_engine_set_gain_db(self, db_gain)
    }

    fn gain_db(&self) -> f32 {
        engine_ffi::ma_engine_get_gain_db(self)
    }

    fn listener_count(&self) -> u32 {
        engine_ffi::ma_engine_get_listener_count(self)
    }

    fn closest_listener(&self, position: Vec3) -> u32 {
        engine_ffi::ma_engine_find_closest_listener(self, position)
    }

    fn set_position(&self, listener: u32, position: Vec3) {
        engine_ffi::ma_engine_listener_set_position(self, listener, position);
    }

    fn position(&self, listener: u32) -> Vec3 {
        engine_ffi::ma_engine_listener_get_position(self, listener)
    }

    fn set_direction(&self, listener: u32, direction: Vec3) {
        engine_ffi::ma_engine_listener_set_direction(self, listener, direction);
    }

    fn direction(&self, listener: u32) -> Vec3 {
        engine_ffi::ma_engine_listener_get_direction(self, listener)
    }

    fn set_velocity(&self, listener: u32, position: Vec3) {
        engine_ffi::ma_engine_listener_set_velocity(self, listener, position);
    }

    fn velocity(&self, listener: u32) -> Vec3 {
        engine_ffi::ma_engine_listener_get_velocity(self, listener)
    }

    fn set_cone(&self, listener: u32, cone: Cone) {
        engine_ffi::ma_engine_listener_set_cone(self, listener, cone);
    }

    fn cone(&self, listener: u32) -> Cone {
        engine_ffi::ma_engine_listener_get_cone(self, listener)
    }

    fn set_world_up(&self, listener: u32, up_direction: Vec3) {
        engine_ffi::ma_engine_listener_set_world_up(self, listener, up_direction);
    }

    fn get_world_up(&self, listener: u32) -> Vec3 {
        engine_ffi::ma_engine_listener_get_world_up(self, listener)
    }

    fn toggle_listener(&self, listener: u32, enabled: bool) {
        engine_ffi::ma_engine_listener_set_enabled(self, listener, enabled);
    }

    fn listener_enabled(&self, listener: u32) -> bool {
        engine_ffi::ma_engine_listener_is_enabled(self, listener)
    }

    fn as_node_graph(&self) -> Option<NodeGraphRef<'_>> {
        engine_ffi::ma_engine_get_node_graph(self)
    }

    /// This function pulls audio from the engine’s internal node graph and returns
    /// up to `frame_count` frames of interleaved PCM samples.
    ///
    /// - This is a **pull-based render operation**.
    /// - The engine will attempt to render `frame_count` frames, but it may return
    ///   **fewer frames**.
    /// - The number of frames actually rendered is returned alongside the samples.
    fn read_pcm_frames(&self, frame_count: u64) -> MaResult<(Vec<f32>, u64)> {
        engine_ffi::ma_engine_read_pcm_frames(self, frame_count)
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
    fn endpoint(&self) -> Option<NodeRef<'_>> {
        engine_ffi::ma_engine_get_endpoint(self)
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
    ///   [`EngineOps::set_time_pcm()`].
    /// - The value is independent of any individual sound’s playback position
    fn time_pcm(&self) -> u64 {
        engine_ffi::ma_engine_get_time_in_pcm_frames(self)
    }

    /// Returns the current engine time in **milliseconds**.
    ///
    /// This is a convenience wrapper over the engine’s internal PCM-frame
    /// clock, converted to milliseconds using the engine’s sample rate.
    ///
    /// - For sample-accurate work, prefer [`EngineOps::time_pcm()`].
    fn time_mili(&self) -> u64 {
        engine_ffi::ma_engine_get_time_in_milliseconds(self)
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
    fn set_time_pcm(&self, time: u64) {
        engine_ffi::ma_engine_set_time_in_pcm_frames(self, time);
    }

    /// Sets the engine’s global time in **milliseconds**.
    ///
    /// This is equivalent to setting the time in PCM frames, but expressed
    /// in milliseconds.
    ///
    /// ## Notes
    /// - Internally converted to PCM frames.
    /// - Precision may be lower than [`EngineOps::set_time_pcm()`].
    fn set_time_mili(&self, time: u64) {
        engine_ffi::ma_engine_set_time_in_milliseconds(self, time);
    }

    /// Returns the number of output **channels** used by the engine.
    ///
    /// Common values include:
    /// - `1` — mono
    /// - `2` — stereo
    ///
    /// This reflects the channel count of the engine’s internal node graph
    /// and output device.
    fn channels(&self) -> u32 {
        engine_ffi::ma_engine_get_channels(self)
    }

    /// Returns the engine’s **sample rate**, in Hz.
    ///
    /// This is the sample rate at which the engine processes audio and
    /// advances its internal time.
    ///
    /// ## Notes
    /// - Typically matches the output device’s sample rate.
    /// - Used to convert between PCM frames and real time.
    fn sample_rate(&self) -> u32 {
        engine_ffi::ma_engine_get_sample_rate(self)
    }
}

// These should be available to EngineRef
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

    pub(crate) fn new_for_tests() -> MaResult<Self> {
        if cfg!(feature = "ci-tests") {
            EngineBuilder::new()
                .no_device(true)
                .set_channels(2)
                .set_sample_rate(SampleRate::Sr44100)
                .build()
        } else {
            Engine::new()
        }
    }

    fn new_with_config(config: Option<&EngineBuilder>) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_engine>> = Box::new(MaybeUninit::uninit());
        engine_ffi::engine_init(config, mem.as_mut_ptr())?;
        // Safety: If mem is not initialized, engine_init will return an error
        let mem: Box<sys::ma_engine> = unsafe { mem.assume_init() };
        let inner = Box::into_raw(mem);
        Ok(Self {
            inner,
            process_notifier: None,
            _not_sync: PhantomData,
        })
    }

    /// Equivalent to calling [`SoundBuilder::new()`]
    pub fn sound(&self) -> SoundBuilder<'_> {
        SoundBuilder::init(self)
    }

    pub fn new_sound(&self) -> MaResult<Sound<'_>> {
        self.new_sound_with_config_internal(None)
    }

    pub fn new_sound_from_file(&self, path: &Path) -> MaResult<Sound<'_>> {
        self.new_sound_with_file_internal(path, SoundFlags::NONE, None, None)
    }

    pub fn new_sound_from_source<D: AsSourcePtr + ?Sized>(
        &self,
        source: &D,
    ) -> MaResult<Sound<'_>> {
        self.new_sound_with_source_internal(SoundFlags::NONE, None, source)
    }

    /// Manually starts the engine
    ///
    /// By default, an engine will be created with `no_auto_start` to false.
    /// Setting [`EngineBuilder::no_auto_start()`] will require a manual start
    ///
    /// Start and stop operations on an engine with no device will result in an error
    pub fn start(&self) -> MaResult<()> {
        engine_ffi::ma_engine_start(self)
    }

    /// Manually stops the engine
    ///
    /// Start and stop operations on an engine with no device will result in an error
    pub fn stop(&self) -> MaResult<()> {
        engine_ffi::ma_engine_stop(self)
    }

    pub fn new_sound_from_file_with_group<'a>(
        &'a self,
        path: &Path,
        sound_group: &'a SoundGroup,
        done_fence: Option<&Fence>,
    ) -> MaResult<Sound<'a>> {
        self.new_sound_with_file_internal(path, SoundFlags::NONE, Some(sound_group), done_fence)
    }

    /// Adding a Fence also requires setting the [`SoundFlags::ASYNC`] flag
    pub fn new_sound_from_file_with_flags(
        &self,
        path: &Path,
        flags: SoundFlags,
        done_fence: Option<&Fence>,
    ) -> MaResult<Sound<'_>> {
        self.new_sound_with_file_internal(path, flags, None, done_fence)
    }

    pub(crate) fn new_sound_with_config_internal(
        &self,
        config: Option<&SoundBuilder>,
    ) -> MaResult<Sound<'_>> {
        let temp_config = SoundBuilder::init(self);
        let config = config.unwrap_or(&temp_config);
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new(MaybeUninit::uninit());

        sound_ffi::ma_sound_init_ex(self, config, mem.as_mut_ptr())?;

        let mem: Box<sys::ma_sound> = unsafe { mem.assume_init() };
        let inner = Box::into_raw(mem);
        Ok(Sound::from_ptr(inner))
    }

    pub(crate) fn new_sound_with_source_internal<'a, D: AsSourcePtr + ?Sized>(
        &'a self,
        flags: SoundFlags,
        sound_group: Option<&'a SoundGroup>,
        data_source: &D,
    ) -> MaResult<Sound<'a>> {
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new(MaybeUninit::uninit());

        sound_ffi::ma_sound_init_from_data_source(
            self,
            data_source,
            flags,
            sound_group,
            mem.as_mut_ptr(),
        )?;

        let mem: Box<sys::ma_sound> = unsafe { mem.assume_init() };
        let inner = Box::into_raw(mem);
        Ok(Sound::from_ptr(inner))
    }

    pub(crate) fn new_sound_with_file_internal<'a>(
        &'a self,
        path: &Path,
        flags: SoundFlags,
        sound_group: Option<&'a SoundGroup>,
        done_fence: Option<&Fence>,
    ) -> MaResult<Sound<'a>> {
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new(MaybeUninit::uninit());

        Sound::init_from_file_internal(
            mem.as_mut_ptr(),
            self,
            path,
            flags,
            sound_group,
            done_fence,
        )?;

        let mem: Box<sys::ma_sound> = unsafe { mem.assume_init() };
        let inner = Box::into_raw(mem);
        Ok(Sound::from_ptr(inner))
    }

    pub fn clone_sound(&self, sound: &Sound, flags: SoundFlags) -> MaResult<Sound<'_>> {
        self.new_sound_instance_internal(sound, flags, None)
    }

    fn new_sound_instance_internal<'a>(
        &'a self,
        sound: &Sound,
        flags: SoundFlags,
        sound_group: Option<&mut SoundGroup>,
    ) -> MaResult<Sound<'a>> {
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new(MaybeUninit::uninit());

        sound_ffi::ma_sound_init_copy(self, sound, flags, sound_group, mem.as_mut_ptr())?;

        let mem: Box<sys::ma_sound> = unsafe { mem.assume_init() };
        let inner = Box::into_raw(mem);
        Ok(Sound::from_ptr(inner))
    }

    pub fn new_sound_group(&self) -> MaResult<SoundGroup<'_>> {
        let mut mem: Box<MaybeUninit<sys::ma_sound_group>> = Box::new(MaybeUninit::uninit());
        let config = self.new_sound_group_config();

        s_group_ffi::ma_sound_group_init_ex(self, config, mem.as_mut_ptr())?;

        let mem: Box<sys::ma_sound_group> = unsafe { mem.assume_init() };
        let inner: *mut sys::ma_sound_group = Box::into_raw(mem);
        Ok(SoundGroup::from_ptr(inner))
    }

    pub fn new_sound_group_config(&self) -> SoundGroupConfig {
        s_group_cfg_ffi::ma_sound_group_config_init_2(self)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        engine_ffi::engine_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
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

/// Custom memory allocation callbacks for miniaudio.
///
/// Miniaudio allows callers to override how heap memory is allocated and freed
/// by providing a `ma_allocation_callbacks` struct (malloc/realloc/free + user data).
///
/// Types such as `NodeGraph` may accept these callbacks at initialization time.
/// If callbacks are not provided, miniaudio uses its default allocator
/// (typically the system allocator).
///
/// ## Lifetimes when borrowed by other types
///
/// `AllocationCallbacks` itself owns the callback table and does not carry a lifetime.
/// However, types that *borrow* an `AllocationCallbacks` (for example `NodeGraph<'a>`)
/// use a lifetime parameter to ensure the callbacks outlive the initialized object.
///
/// This matters because miniaudio requires the same allocation callbacks to be passed
/// again during uninitialization so it can free any internal allocations consistently.
pub struct AllocationCallbacks {
    pub(crate) inner: sys::ma_allocation_callbacks,
}

#[cfg(test)]
mod test {
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
    fn test_engine_node_graph_and_endpoint_exist() {
        let engine = Engine::new_for_tests().unwrap();

        let graph = engine.as_node_graph();
        assert!(graph.is_some(), "engine should expose a node graph");

        let endpoint = engine.endpoint();
        assert!(endpoint.is_some(), "engine should expose an endpoint node");
    }

    #[test]
    fn test_engine_read_pcm_frames_shapes_output() {
        let engine = Engine::new_for_tests().unwrap();

        let requested = 256u64;
        let (samples, frames) = engine.read_pcm_frames(requested).unwrap();

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
        let sr = engine.sample_rate();

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

pub(crate) mod engine_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        audio::{math::vec3::Vec3, spatial::cone::Cone},
        engine::{
            engine_builder::EngineBuilder,
            node_graph::{nodes::NodeRef, NodeGraphRef},
            private_engine, AsEnginePtr, Binding, Engine, EngineOps,
        },
        MaRawResult, MaResult,
    };

    #[inline]
    pub fn engine_init(
        config: Option<&EngineBuilder>,
        engine: *mut sys::ma_engine,
    ) -> MaResult<()> {
        let p_config: *const sys::ma_engine_config =
            config.map_or(core::ptr::null(), |c| &c.to_raw() as *const _);
        let res = unsafe { sys::ma_engine_init(p_config, engine) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn engine_uninit(engine: &Engine) {
        unsafe {
            sys::ma_engine_uninit(engine.to_raw());
        }
    }

    #[inline]
    pub fn ma_engine_read_pcm_frames<E: AsEnginePtr + ?Sized>(
        engine: &E,
        frame_count: u64,
    ) -> MaResult<(Vec<f32>, u64)> {
        let channels = engine.channels();
        let mut buffer = vec![0.0f32; (frame_count * channels as u64) as usize];
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_engine_read_pcm_frames(
                private_engine::engine_ptr(engine),
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
                frame_count,
                &mut frames_read,
            )
        };
        MaRawResult::check(res)?;
        buffer.truncate((frames_read * channels as u64) as usize);
        Ok((buffer, frames_read))
    }

    #[inline]
    pub fn ma_engine_get_node_graph<'a, E: AsEnginePtr + ?Sized>(
        engine: &'a E,
    ) -> Option<NodeGraphRef<'a>> {
        let ptr = unsafe { sys::ma_engine_get_node_graph(private_engine::engine_ptr(engine)) };
        if ptr.is_null() {
            None
        } else {
            Some(NodeGraphRef::from_ptr(ptr))
        }
    }

    // AsEnginePtr
    // TODO: Create ResourceManRef. Implement MA_NO_RESOURCE_MANAGER?
    #[inline]
    pub fn ma_engine_get_resource_manager(engine: &Engine) -> *mut sys::ma_resource_manager {
        unsafe { sys::ma_engine_get_resource_manager(engine.to_raw()) }
    }

    // AsEnginePtr
    // TODO: Create Device(Ref?)
    #[inline]
    pub fn ma_engine_get_device(engine: &Engine) -> *mut sys::ma_device {
        unsafe { sys::ma_engine_get_device(engine.to_raw()) }
    }

    // AsEnginePtr
    // TODO: Implement Log(Ref?)
    #[inline]
    pub fn ma_engine_get_log(engine: &Engine) -> *mut sys::ma_log {
        unsafe { sys::ma_engine_get_log(engine.to_raw()) }
    }

    #[inline]
    pub fn ma_engine_get_endpoint<'a, E: AsEnginePtr + ?Sized>(
        engine: &'a E,
    ) -> Option<NodeRef<'a>> {
        let ptr = unsafe { sys::ma_engine_get_endpoint(private_engine::engine_ptr(engine)) };
        if ptr.is_null() {
            None
        } else {
            Some(NodeRef::from_ptr(ptr))
        }
    }

    #[inline]
    pub fn ma_engine_get_time_in_pcm_frames<E: AsEnginePtr + ?Sized>(engine: &E) -> u64 {
        unsafe {
            sys::ma_engine_get_time_in_pcm_frames(private_engine::engine_ptr(engine) as *const _)
        }
    }

    #[inline]
    pub fn ma_engine_get_time_in_milliseconds<E: AsEnginePtr + ?Sized>(engine: &E) -> u64 {
        unsafe {
            sys::ma_engine_get_time_in_milliseconds(private_engine::engine_ptr(engine) as *const _)
        }
    }

    #[inline]
    pub fn ma_engine_set_time_in_pcm_frames<E: AsEnginePtr + ?Sized>(engine: &E, time: u64) {
        unsafe { sys::ma_engine_set_time_in_pcm_frames(private_engine::engine_ptr(engine), time) };
    }

    #[inline]
    pub fn ma_engine_set_time_in_milliseconds<E: AsEnginePtr + ?Sized>(engine: &E, time: u64) {
        unsafe {
            sys::ma_engine_set_time_in_milliseconds(private_engine::engine_ptr(engine), time)
        };
    }

    #[inline]
    pub fn ma_engine_get_channels<E: AsEnginePtr + ?Sized>(engine: &E) -> u32 {
        unsafe { sys::ma_engine_get_channels(private_engine::engine_ptr(engine) as *const _) }
    }

    #[inline]
    pub fn ma_engine_get_sample_rate<E: AsEnginePtr + ?Sized>(engine: &E) -> u32 {
        unsafe { sys::ma_engine_get_sample_rate(private_engine::engine_ptr(engine) as *const _) }
    }

    #[inline]
    pub fn ma_engine_start(engine: &Engine) -> MaResult<()> {
        let res = unsafe { sys::ma_engine_start(engine.to_raw()) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_engine_stop(engine: &Engine) -> MaResult<()> {
        let res = unsafe { sys::ma_engine_stop(engine.to_raw()) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_engine_set_volume<E: AsEnginePtr + ?Sized>(engine: &E, volume: f32) -> MaResult<()> {
        let res = unsafe { sys::ma_engine_set_volume(private_engine::engine_ptr(engine), volume) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_engine_get_volume<E: AsEnginePtr + ?Sized>(engine: &E) -> f32 {
        unsafe { sys::ma_engine_get_volume(private_engine::engine_ptr(engine)) }
    }

    #[inline]
    pub fn ma_engine_set_gain_db<E: AsEnginePtr + ?Sized>(
        engine: &E,
        db_gain: f32,
    ) -> MaResult<()> {
        let res =
            unsafe { sys::ma_engine_set_gain_db(private_engine::engine_ptr(engine), db_gain) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_engine_get_gain_db<E: AsEnginePtr + ?Sized>(engine: &E) -> f32 {
        unsafe { sys::ma_engine_get_gain_db(private_engine::engine_ptr(engine)) }
    }

    #[inline]
    pub fn ma_engine_get_listener_count<E: AsEnginePtr + ?Sized>(engine: &E) -> u32 {
        unsafe { sys::ma_engine_get_listener_count(private_engine::engine_ptr(engine) as *const _) }
    }

    #[inline]
    pub fn ma_engine_find_closest_listener<E: AsEnginePtr + ?Sized>(
        engine: &E,
        position: Vec3,
    ) -> u32 {
        unsafe {
            sys::ma_engine_find_closest_listener(
                private_engine::engine_ptr(engine) as *const _,
                position.x,
                position.y,
                position.z,
            )
        }
    }

    #[inline]
    pub fn ma_engine_listener_set_position<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
        position: Vec3,
    ) {
        unsafe {
            sys::ma_engine_listener_set_position(
                private_engine::engine_ptr(engine),
                listener,
                position.x,
                position.y,
                position.z,
            )
        };
    }

    #[inline]
    pub fn ma_engine_listener_get_position<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
    ) -> Vec3 {
        let vec = unsafe {
            sys::ma_engine_listener_get_position(
                private_engine::engine_ptr(engine) as *const _,
                listener,
            )
        };
        vec.into()
    }

    #[inline]
    pub fn ma_engine_listener_set_direction<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
        position: Vec3,
    ) {
        unsafe {
            sys::ma_engine_listener_set_direction(
                private_engine::engine_ptr(engine),
                listener,
                position.x,
                position.y,
                position.z,
            )
        };
    }

    #[inline]
    pub fn ma_engine_listener_get_direction<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
    ) -> Vec3 {
        let vec = unsafe {
            sys::ma_engine_listener_get_direction(
                private_engine::engine_ptr(engine) as *const _,
                listener,
            )
        };
        vec.into()
    }

    #[inline]
    pub fn ma_engine_listener_set_velocity<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
        position: Vec3,
    ) {
        unsafe {
            sys::ma_engine_listener_set_velocity(
                private_engine::engine_ptr(engine),
                listener,
                position.x,
                position.y,
                position.z,
            )
        };
    }

    #[inline]
    pub fn ma_engine_listener_get_velocity<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
    ) -> Vec3 {
        let vec = unsafe {
            sys::ma_engine_listener_get_velocity(
                private_engine::engine_ptr(engine) as *const _,
                listener,
            )
        };
        vec.into()
    }

    #[inline]
    pub fn ma_engine_listener_set_cone<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
        cone: Cone,
    ) {
        unsafe {
            sys::ma_engine_listener_set_cone(
                private_engine::engine_ptr(engine),
                listener,
                cone.inner_angle_rad,
                cone.outer_angle_rad,
                cone.outer_gain,
            )
        };
    }

    #[inline]
    pub fn ma_engine_listener_get_cone<E: AsEnginePtr + ?Sized>(engine: &E, listener: u32) -> Cone {
        let mut inner = 0.0f32;
        let mut outer = 0.0f32;
        let mut gain = 1.0f32;

        unsafe {
            sys::ma_engine_listener_get_cone(
                private_engine::engine_ptr(engine) as *const _,
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
    pub fn ma_engine_listener_set_world_up<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
        vec: Vec3,
    ) {
        unsafe {
            sys::ma_engine_listener_set_world_up(
                private_engine::engine_ptr(engine),
                listener,
                vec.x,
                vec.y,
                vec.z,
            );
        }
    }

    #[inline]
    pub fn ma_engine_listener_get_world_up<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
    ) -> Vec3 {
        let vec = unsafe {
            sys::ma_engine_listener_get_world_up(
                private_engine::engine_ptr(engine) as *const _,
                listener,
            )
        };
        vec.into()
    }

    #[inline]
    pub fn ma_engine_listener_set_enabled<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
        enabled: bool,
    ) {
        unsafe {
            sys::ma_engine_listener_set_enabled(
                private_engine::engine_ptr(engine),
                listener,
                enabled as u32,
            )
        }
    }

    #[inline]
    pub fn ma_engine_listener_is_enabled<E: AsEnginePtr + ?Sized>(
        engine: &E,
        listener: u32,
    ) -> bool {
        let res = unsafe {
            sys::ma_engine_listener_is_enabled(
                private_engine::engine_ptr(engine) as *const _,
                listener,
            )
        };
        res == 1
    }
}
