//! Audio engine.
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
//! # fn main() -> maudio::Result<()> {
//! let engine = Engine::new()?;
//! // let mut sound = engine.new_sound_from_file("music.ogg")?;
//! // sound.start()?;
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
//! Advanced users can access the endpoint node via [`Engine::endpoint`] to
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
use std::{
    cell::Cell, ffi::CString, marker::PhantomData, mem::MaybeUninit, path::Path, ptr::NonNull,
};

use crate::{
    Binding, ErrorKinds, MaError, Result,
    engine::node_graph::{NodeGraphRef, nodes::NodeRef},
    sound::{
        Sound,
        sound_builder::SoundBuilder,
        sound_ffi,
        sound_flags::SoundFlags,
        sound_group::{SoundGroup, SoundGroupConfig, s_group_cfg_ffi, s_group_ffi},
    },
};

use maudio_sys::ffi as sys;

pub mod node_graph;

pub enum EngineError {}

impl From<EngineError> for ErrorKinds {
    fn from(e: EngineError) -> Self {
        ErrorKinds::Engine(e)
    }
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
///
/// ## Threading model
/// Miniaudio runs an internal audio callback on a real-time thread (created by
/// the backend). Methods on `Engine` generally forward to the underlying
/// `ma_engine` and may be called while audio is running.
///
/// This type does **not** automatically guarantee that every method is
/// real-time safe. Avoid doing allocations or other heavy work from contexts
/// that must be real-time safe.
///
/// ## Pinning and FFI safety
/// `ma_engine` contains self-references and pointers to internal state, and must
/// not be moved after initialization. To uphold this invariant, `Engine` stores
/// the underlying engine in a pinned allocation.
///
/// Any references returned from the engine (for example, endpoint / node graph
/// accessors) are **borrows** into engine-owned state and cannot outlive the
/// engine.
pub struct Engine {
    inner: *mut sys::ma_engine,
    _not_sync: PhantomData<Cell<()>>,
}

pub struct EngineRef<'a> {
    ptr: *mut sys::ma_engine,
    _marker: PhantomData<&'a ()>,
    _not_sync: PhantomData<Cell<()>>,
}

impl Binding for EngineRef<'_> {
    type Raw = *mut sys::ma_engine;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            ptr: raw,
            _marker: PhantomData,
            _not_sync: PhantomData,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.ptr
    }
}

impl Engine {
    /// Creates a new engine using the default configuration.
    ///
    /// This is a convenience constructor equivalent to calling
    /// [`Engine::with_config`] with a default [`EngineConfig`].
    ///
    /// Most applications should start with this method.
    pub fn new() -> Result<Self> {
        Self::new_with_config(None)
    }

    /// Creates a new engine using a custom configuration.
    ///
    /// This allows fine-grained control over engine initialization, such as:
    /// - output format (sample rate, channels)
    /// - resource manager behavior
    /// - backend- or device-specific options
    ///
    /// For a detailed description of each option, see [`EngineConfig`].
    ///
    /// ## Notes
    /// - The engine takes a snapshot of the configuration during initialization.
    /// - The configuration does not need to outlive the engine.
    pub fn with_config(config: EngineConfig) -> Result<Self> {
        Self::new_with_config(Some(&config))
    }

    fn new_with_config(config: Option<&EngineConfig>) -> Result<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_engine>> = Box::new_uninit();
        engine_ffi::engine_init(config, mem.as_mut_ptr())?;
        // Safety: If mem is not initialized, engine_init will return an error
        let mem: Box<sys::ma_engine> = unsafe { mem.assume_init() };
        let inner = Box::into_raw(mem);
        Ok(Self::from_ptr(inner))
    }

    pub fn new_sound(&self) -> Result<Sound<'_>> {
        self.new_sound_with_config_internal(None)
    }

    pub fn new_sound_with_config(&self, config: SoundBuilder) -> Result<Sound<'_>> {
        self.new_sound_with_config_internal(Some(config))
    }

    pub fn new_sound_from_file(&self, path: &Path) -> Result<Sound<'_>> {
        self.new_sound_with_file_internal(path, SoundFlags::NONE, None)
    }

    pub fn new_sound_from_file_with_group(
        &self,
        path: &Path,
        sound_group: &mut SoundGroup,
    ) -> Result<Sound<'_>> {
        self.new_sound_with_file_internal(path, SoundFlags::NONE, Some(sound_group))
    }

    pub fn new_sound_from_file_with_flags(
        &self,
        path: &Path,
        flags: SoundFlags,
    ) -> Result<Sound<'_>> {
        self.new_sound_with_file_internal(path, flags, None)
    }

    // TODO
    pub fn pcm_frames(&mut self) {
        // let frames = engine_ffi::ma_engine_read_pcm_frames(engine, frames_out, frame_count, frames_read);
        todo!()
    }

    pub fn node_graph(&mut self) -> Option<NodeGraphRef<'_>> {
        engine_ffi::ma_engine_get_node_graph(self)
    }

    pub fn resource_manager(&mut self) {
        // engine_ffi::ma_engine_get_resource_manager(self);
        todo!()
    }

    pub fn device(&mut self) {
        // engine_ffi::ma_engine_get_device(self);
        todo!()
    }

    pub fn log(&mut self) {
        // engine_ffi::ma_engine_get_log(self);
        todo!()
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
    pub fn endpoint(&mut self) -> Option<NodeRef<'_>> {
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
    ///   [`set_time_pcm`].
    /// - The value is independent of any individual sound’s playback position.
    pub fn time_pcm(&self) -> u64 {
        engine_ffi::ma_engine_get_time_in_pcm_frames(self)
    }

    /// Returns the current engine time in **milliseconds**.
    ///
    /// This is a convenience wrapper over the engine’s internal PCM-frame
    /// clock, converted to milliseconds using the engine’s sample rate.
    ///
    /// ## Notes
    /// - This value may lose precision compared to [`time_pcm`].
    /// - For sample-accurate work, prefer [`time_pcm`].
    pub fn time_mili(&self) -> u64 {
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
    pub fn set_time_pcm(&mut self, time: u64) {
        engine_ffi::ma_engine_set_time_in_pcm_frames(self, time);
    }

    /// Sets the engine’s global time in **milliseconds**.
    ///
    /// This is equivalent to setting the time in PCM frames, but expressed
    /// in milliseconds.
    ///
    /// ## Notes
    /// - Internally converted to PCM frames.
    /// - Precision may be lower than [`set_time_pcm`].
    pub fn set_time_mili(&mut self, time: u64) {
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
    pub fn channels(&self) -> u32 {
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
    pub fn sample_rate(&self) -> u32 {
        engine_ffi::ma_engine_get_sample_rate(self)
    }

    fn new_sound_with_config_internal(&self, config: Option<SoundBuilder>) -> Result<Sound<'_>> {
        let config = config.unwrap_or(SoundBuilder::init(self.to_raw()));
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new_uninit();

        sound_ffi::ma_sound_init_ex(self, &config, mem.as_mut_ptr())?;

        let mem: Box<sys::ma_sound> = unsafe { mem.assume_init() };
        let inner = Box::into_raw(mem);
        Ok(Sound::from_ptr(inner))
    }

    fn new_sound_with_file_internal(
        &self,
        path: &Path,
        flags: SoundFlags,
        sound_group: Option<&mut SoundGroup>,
    ) -> Result<Sound<'_>> {
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new_uninit();

        Sound::init_from_file_internal(mem.as_mut_ptr(), self, path, flags, sound_group, None)?;

        let mem: Box<sys::ma_sound> = unsafe { mem.assume_init() };
        let inner = Box::into_raw(mem);
        Ok(Sound::from_ptr(inner))
    }

    // TODO: Not yet exposed to the public API
    fn new_sound_instance_internal(
        &self,
        sound: &Sound,
        flags: SoundFlags,
        sound_group: Option<&mut SoundGroup>,
    ) -> Result<Sound<'_>> {
        let mut mem: Box<MaybeUninit<sys::ma_sound>> = Box::new_uninit();

        sound_ffi::ma_sound_init_copy(self, sound, flags, sound_group, mem.as_mut_ptr())?;

        let mem: Box<sys::ma_sound> = unsafe { mem.assume_init() };
        let inner = Box::into_raw(mem);
        Ok(Sound::from_ptr(inner))
    }

    pub fn new_sound_group(&self) -> Result<SoundGroup> {
        let mut mem: Box<MaybeUninit<sys::ma_sound_group>> = Box::new_uninit();
        let config = self.new_sound_group_config();

        s_group_ffi::ma_sound_group_init_ex(self, config, mem.as_mut_ptr())?;

        let mem: Box<sys::ma_sound> = unsafe { mem.assume_init() };
        let inner = NonNull::new(Box::into_raw(mem)).expect("NonNull ma_sound");
        Ok(SoundGroup::new(inner))
    }

    pub fn new_sound_group_config(&self) -> SoundGroupConfig {
        s_group_cfg_ffi::ma_sound_group_config_init_2(self)
    }
}

impl Binding for Engine {
    type Raw = *mut sys::ma_engine;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            _not_sync: PhantomData,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        engine_ffi::engine_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

#[cfg(unix)]
pub(crate) fn cstring_from_path(path: &Path) -> Result<CString> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(path.as_os_str().as_bytes()).map_err(|_| MaError(sys::ma_result_MA_INVALID_ARGS))
}

#[cfg(windows)]
pub(crate) fn wide_null_terminated(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    // UTF-16 + trailing NUL
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub struct EngineConfig {
    inner: sys::ma_engine_config,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineConfig {
    pub fn new() -> Self {
        Self {
            inner: unsafe { sys::ma_engine_config_init() },
        }
    }

    fn get_raw(&mut self) -> &mut sys::ma_engine_config {
        &mut self.inner
    }
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
    inner: sys::ma_allocation_callbacks,
}

#[cfg(test)]
mod test {

    #[test]
    #[cfg(feature = "device-tests")]
    fn engine_test_works_with_default() {
        use super::*;

        let _engine = Engine::new().unwrap();
    }

    #[test]
    #[cfg(feature = "device-tests")]
    fn engine_test_works_with_cfg() {
        use super::*;

        let config = EngineConfig::new();
        let _engine = Engine::with_config(config).unwrap();
    }

    #[test]
    #[cfg(feature = "device-tests")]
    fn engine_test_init_engine_and_sound() {
        use super::*;

        let engine = Engine::new().unwrap();
        let _sound = engine.new_sound().unwrap();
    }

    #[test]
    #[cfg(feature = "device-tests")]
    fn engine_test_init_sound_from_path() {
        use super::*;
        use std::path::Path;

        let engine = Engine::new().unwrap();
        let path = Path::new("examples/assets/Goldberg Variations, BWV. 988 - Variation 4.mp3");
        let mut sound = engine.new_sound_from_file(path).unwrap();
        sound.play_sound().unwrap();
    }
}

pub(crate) mod engine_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        MaRawResult, Result,
        engine::{
            Binding, Engine, EngineConfig,
            node_graph::{NodeGraphRef, nodes::NodeRef},
        },
    };

    #[inline]
    pub fn engine_init(config: Option<&EngineConfig>, engine: *mut sys::ma_engine) -> Result<()> {
        let p_config: *const sys::ma_engine_config =
            config.map_or(core::ptr::null(), |c| &c.inner as *const _);
        let res = unsafe { sys::ma_engine_init(p_config, engine) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn engine_uninit(engine: &Engine) {
        unsafe {
            sys::ma_engine_uninit(engine.to_raw());
        }
    }

    // TODO
    #[inline]
    pub fn ma_engine_read_pcm_frames(
        engine: &Engine,
        frames_out: *mut core::ffi::c_void,
        frame_count: sys::ma_uint64,
        frames_read: *mut sys::ma_uint64,
    ) -> i32 {
        unsafe {
            sys::ma_engine_read_pcm_frames(engine.to_raw(), frames_out, frame_count, frames_read)
        }
    }

    #[inline]
    pub fn ma_engine_get_node_graph(engine: &Engine) -> Option<NodeGraphRef<'_>> {
        let ptr = unsafe { sys::ma_engine_get_node_graph(engine.to_raw()) };
        if ptr.is_null() {
            None
        } else {
            Some(NodeGraphRef::from_ptr(ptr))
        }
    }

    // TODO: Create ResourceManRef. Implement MA_NO_RESOURCE_MANAGER?
    #[inline]
    pub fn ma_engine_get_resource_manager(engine: &Engine) -> *mut sys::ma_resource_manager {
        unsafe { sys::ma_engine_get_resource_manager(engine.to_raw()) }
    }

    // TODO: Create Device(Ref?)
    #[inline]
    pub fn ma_engine_get_device(engine: &Engine) -> *mut sys::ma_device {
        unsafe { sys::ma_engine_get_device(engine.to_raw()) }
    }

    // TODO: Implement Log(Ref?)
    #[inline]
    pub fn ma_engine_get_log(engine: &Engine) -> *mut sys::ma_log {
        unsafe { sys::ma_engine_get_log(engine.to_raw()) }
    }

    #[inline]
    pub fn ma_engine_get_endpoint<'a>(engine: &'a Engine) -> Option<NodeRef<'a>> {
        let ptr = unsafe { sys::ma_engine_get_endpoint(engine.to_raw()) };
        if ptr.is_null() {
            None
        } else {
            Some(NodeRef::from_ptr(ptr))
        }
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
    pub fn ma_engine_get_channels(engine: &Engine) -> u32 {
        unsafe { sys::ma_engine_get_channels(engine.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_engine_get_sample_rate(engine: &Engine) -> u32 {
        unsafe { sys::ma_engine_get_sample_rate(engine.to_raw() as *const _) }
    }
}
