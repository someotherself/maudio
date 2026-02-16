//! Builder for constructing a [`Sound`]
//!
//! Use this when the convenience constructors aren’t enough (custom attachment,
//! channel configuration, async loading fence, start-on-build, etc.).
//!
//! ## Source selection (exactly one)
//! A sound is initialized from **at most one** source:
//! - [`SoundBuilder::file_path`]
//! - [`SoundBuilder::data_source`] (via a `DataSource` type)
//! - [`SoundBuilder::no_source`] (no playback source; some flags become invalid)
//!
//! Calling a source setter replaces any previously set source.
//!
//! ## Platform path ownership
//! When using [`SoundBuilder::file_path`], the builder stores an owned, platform-specific
//! copy of the path (UTF-8 `CString` on Unix, wide + NUL on Windows) so the raw pointer
//! inside `ma_sound_config` remains valid until [`SoundBuilder::build`] completes.
//!
//! ## Async loading and fences
//! [`SoundBuilder::fence`] is only valid for file-based sounds. It implicitly enables
//! [`SoundFlags::ASYNC`]. Using a fence with [`SoundBuilder::data_source`] or
//! [`SoundBuilder::no_source`] returns `MA_INVALID_ARGS`.
//!
//! ## Start playing on build
//! [`SoundBuilder::start_playing`] will call `sound.play_sound()` after initialization,
//! but only when a real source is set. It is rejected for [`SoundBuilder::no_source`].
//!
//! ## End notifications
//! [`SoundBuilder::with_end_notifier`] builds the sound and returns an [`EndNotifier`]
//! that becomes `true` once the sound reaches the end callback.
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

use maudio_sys::ffi as sys;

use crate::{
    audio::math::vec3::Vec3,
    data_source::{private_data_source, AsSourcePtr, DataSourceRef},
    engine::{
        node_graph::nodes::{private_node, AsNodePtr},
        Engine, EngineOps,
    },
    sound::{
        notifier::EndNotifier, sound_flags::SoundFlags, sound_group::SoundGroup, Sound, SoundSource,
    },
    util::fence::Fence,
    Binding, MaResult,
};

/// Builder for constructing a [`Sound`]
///
/// # Examples
///
/// Initialize from a file:
///
/// ```no_run
/// # use std::path::Path;
/// # use maudio::engine::Engine;
/// # use maudio::sound::sound_builder::SoundBuilder;
/// # fn demo(engine: &Engine) -> maudio::MaResult<()> {
/// let sound = SoundBuilder::new(&engine)
///     .file_path(Path::new("assets/click.wav"))
///     .channels_in(1)
///     .channels_out(0) // 0 = engine's native channel count
///     .build()?;
/// # Ok(())
/// # }
/// ```
///
/// Initialize from an existing data source:
///
/// ```no_run
/// # use maudio::engine::Engine;
/// # use maudio::sound::sound_builder::SoundBuilder;
/// # use maudio::data_source::sources::buffer::AudioBuffer;
/// # fn demo(engine: &Engine, ds: &AudioBuffer<f32>) -> maudio::MaResult<()> {
/// let sound = SoundBuilder::new(&engine).data_source(ds).build()?;
/// # Ok(())
/// # }
/// ```
///
/// Attach to a node/bus on init:
///
/// ```no_run
/// # use maudio::engine::Engine;
/// # use maudio::engine::EngineOps;
/// # use maudio::sound::sound_builder::SoundBuilder;
/// # fn demo(engine: &Engine) -> maudio::MaResult<()> {
/// // Attach to the engine's endpoint input bus 0 at creation time.
/// let endpoint = engine.endpoint().expect("engine has an endpoint");
///
/// let sound = SoundBuilder::new(&engine)
///     .file_path("assets/music.ogg".as_ref())
///     .initial_attachment(&endpoint, 0) // redundant here, only as example
///     .build()?;
/// # Ok(())
/// # }
/// ```
///
/// # Notes
/// - `SoundBuilder` is consumed by `build()` and should be used once, matching
///   miniaudio's "fill config → init" workflow.
/// - If you only need a simple sound, prefer the convenience constructors on [`Engine`] / [`Sound`].
pub struct SoundBuilder<'a> {
    pub(crate) inner: sys::ma_sound_config,
    engine: &'a Engine,
    source: SoundSource<'a>,
    owned_path: OwnedPathBuf,
    fence: Option<&'a Fence>,
    flags: SoundFlags,
    group: Option<&'a SoundGroup<'a>>,
    end_notifier: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    sound_state: SoundState,
}

#[derive(Default)]
struct SoundState {
    min_distance: Option<f32>,
    max_distance: Option<f32>,
    rolloff: Option<f32>,
    position: Option<Vec3>,
    velocity: Option<Vec3>,
    direction: Option<Vec3>,
    start_playing: bool,
}

// Keeps the ptr to the path alive
#[derive(Default)]
enum OwnedPathBuf {
    #[default]
    None,
    #[cfg(unix)]
    Utf8(std::ffi::CString),
    #[cfg(windows)]
    Wide(Vec<u16>),
}

// TODO: Add ma_mono_expansion_mode
impl<'a> SoundBuilder<'a> {
    pub fn new(engine: &'a Engine) -> Self {
        SoundBuilder::init(engine)
    }

    fn set_end_notifier(&mut self) -> EndNotifier {
        let notifier = EndNotifier::new();
        self.end_notifier = Some(notifier.clone_flag());

        self.inner.pEndCallbackUserData = notifier.as_user_data_ptr();
        self.inner.endCallback = Some(crate::sound::notifier::on_end_callback);

        notifier
    }

    pub fn with_end_notifier(&mut self) -> MaResult<(Sound<'a>, EndNotifier)> {
        self.set_source()?;
        let notifier = self.set_end_notifier();

        let sound = self.start_sound(Some(notifier.clone_flag()))?;

        Ok((sound, notifier))
    }

    fn start_sound(&mut self, notif: Option<Arc<AtomicBool>>) -> MaResult<Sound<'a>> {
        let mut sound = match self.source {
            SoundSource::DataSource(_) => {
                if self.fence.is_some() {
                    return Err(crate::MaudioError::from_ma_result(
                        sys::ma_result_MA_INVALID_ARGS,
                    ));
                }
                self.engine.new_sound_with_config_internal(Some(self))?
            }
            #[cfg(unix)]
            SoundSource::FileUtf8(_) => self.engine.new_sound_with_config_internal(Some(self))?,
            #[cfg(windows)]
            SoundSource::FileWide(_) => self.engine.new_sound_with_config_internal(Some(self))?,
            SoundSource::None => {
                self.check_flags_without_source()?;

                if self.fence.is_some() || self.sound_state.start_playing {
                    return Err(crate::MaudioError::from_ma_result(
                        sys::ma_result_MA_INVALID_ARGS,
                    ));
                }
                self.engine.new_sound_with_config_internal(Some(self))?
            }
        };

        self.configure_sound(&mut sound);
        sound.end_notifier = notif;
        if self.source.is_valid() && self.sound_state.start_playing {
            sound.play_sound()?;
        }
        Ok(sound)
    }

    pub fn build(&mut self) -> MaResult<Sound<'a>> {
        self.set_source()?;
        self.start_sound(None)
    }

    /// Explicitly sets the sound to have no playback source.
    ///
    /// This is a convenience method for creating a silent sound or clearing a
    /// previously configured source.
    ///
    /// If no source was previously added, this has no effect
    pub fn no_source(&mut self) -> &mut Self {
        self.source = SoundSource::None;
        self
    }

    /// Sets the source of the sound from a path
    ///
    /// A sound can be initialized from **either** a file path **or** a data source,
    /// but not both. Calling this method overrides any previously set data_source.
    ///
    /// The provided path is converted to the platform-specific format required by
    /// miniaudio and is only used during sound initialization.
    pub fn file_path(&mut self, path: &'a Path) -> &mut Self {
        self.source = SoundSource::None;
        #[cfg(unix)]
        {
            self.source = SoundSource::FileUtf8(path);
        }
        #[cfg(windows)]
        {
            self.source = SoundSource::FileWide(path);
        }
        self
    }

    /// Sets the source of the sound as a data source (ma_data_source)
    ///
    /// In miniaudio, a data source is an abstraction used to supply decoded audio data
    /// to the engine. File decoders, procedural generators, and custom audio streams
    /// are all exposed through the `ma_data_source` interface.
    ///
    /// When a data source is provided, the sound will pull audio directly from it
    /// instead of loading from a file path.
    ///
    /// A sound can be initialized from **either** a file path **or** a data source,
    /// but not both. Calling this method overrides any previously set file path.
    ///
    /// # Lifetime
    /// The provided `source` must:
    ///
    /// - point to a valid, initialized [`DataSource`](crate::data_source::DataSource)
    /// - remain alive for the entire lifetime of the created sound
    ///
    /// # When to use this
    /// This method is intended for more advanced use cases, such as:
    ///
    /// - procedural or generated audio
    /// - streaming audio from memory or network sources
    /// - reusing a single data source across multiple sounds
    ///
    /// For simple file playback, prefer initializing the sound from a file path.
    pub fn data_source<S: AsSourcePtr + ?Sized>(&mut self, source: &'a S) -> &mut Self {
        self.source = SoundSource::DataSource(DataSourceRef::from_ptr(
            private_data_source::source_ptr(source),
        ));
        self
    }

    pub fn sound_group(&mut self, group: &'a SoundGroup) -> &mut Self {
        self.inner.pInitialAttachment = private_node::node_ptr(&group.as_node());
        self.group = Some(group);

        self
    }

    /// Attach a [`Fence`] that will be signaled when asynchronous sound loading completes.
    ///
    /// This implicitly enables [`SoundFlags::ASYNC`].
    ///
    /// A fence is only meaningful when the sound is created from a file.
    /// Using a fence without a file source will result in a runtime error.
    pub fn fence(&mut self, fence: &'a Fence) -> &mut Self {
        self.fence = Some(fence);
        self.async_load(true)
    }

    /// By default, a newly created sound is attached to the engine's main output graph,
    /// unless [`SoundFlags::NO_DEFAULT_ATTACHMENT`] is set in `flags`.
    ///
    /// Calling this method allows you to override that behavior (regardless of the flag) and immediately connect
    /// the sound to a specific miniaudio node instead.
    ///
    /// # Inputs
    /// `node` is the target node to attach to
    ///
    /// `input_bus` specifies which input bus on that node the sound should be connected to.
    ///
    /// # When you do NOT need this
    /// If you are simply playing sounds through the engine's default output (the most
    /// common case), you should not call this method. The engine will automatically
    /// attach the sound for you.
    pub fn initial_attachment<N: AsNodePtr + ?Sized>(
        &mut self,
        node: &N,
        input_bus: u32,
    ) -> &mut Self {
        self.inner.pInitialAttachment = private_node::node_ptr(node);
        self.inner.initialAttachmentInputBusIndex = input_bus;
        self
    }

    /// Sets the number of input channels for the sound node.
    ///
    /// The "channel" does not refer to a speaker, sound channel and it does not control spatialization directly.
    ///
    /// Is ignored if source is a [`DataSource`](crate::data_source::DataSource)
    ///
    /// This controls how many channels miniaudio expects from the sound's data source.
    /// In most cases this should be left at `0`, which allows miniaudio to infer the
    /// channel count automatically from the source.
    ///
    /// This is primarily useful for custom or procedural data sources.
    pub fn channels_in(&mut self, ch: u32) -> &mut Self {
        self.inner.channelsIn = ch;
        self
    }

    /// Sets the number of output channels for the sound node.
    ///
    /// The "channel" does not refer to a speaker, sound channel and it does not control spatialization directly.
    ///
    /// This controls how many channels the sound outputs into the node graph.
    /// A value of `0` means "use the engine's native channel count", which is the
    /// recommended default.
    ///
    /// Miniaudio will automatically convert between input and output channel counts
    /// as needed (e.g. mono → stereo).
    pub fn channels_out(&mut self, ch: u32) -> &mut Self {
        self.inner.channelsOut = ch;
        self
    }

    /// Sets the [`SoundFlags`]
    pub fn flags(&mut self, flags: SoundFlags) -> &mut Self {
        self.inner.flags = flags.bits();
        self.flags = flags;
        self
    }

    /// Sets the volume smoothing time, in PCM frames.
    ///
    /// Larger values smooth abrupt volume changes over a longer period.
    pub fn volume_smooth_frames(&mut self, pcm_frames: u32) -> &mut Self {
        self.inner.volumeSmoothTimeInPCMFrames = pcm_frames;
        self
    }

    /// Sets the first PCM frame that can be played.
    ///
    /// Frames before this point are skipped during playback.
    pub fn range_begin_frames(&mut self, pcm_frames: u64) -> &mut Self {
        self.inner.rangeBegInPCMFrames = pcm_frames;
        self
    }

    /// Sets the last PCM frame that can be played.
    ///
    /// Playback stops when this frame is reached.
    pub fn range_end_frames(&mut self, pcm_frames: u64) -> &mut Self {
        self.inner.rangeEndInPCMFrames = pcm_frames;
        self
    }

    /// Sets the loop start position, in PCM frames.
    ///
    /// Only meaningful when looping is enabled.
    pub fn loop_begin_frames(&mut self, pcm_frames: u64) -> &mut Self {
        self.inner.loopPointBegInPCMFrames = pcm_frames;
        self
    }

    /// Sets the loop end position, in PCM frames.
    ///
    /// When reached, playback jumps back to the loop begin frame.
    pub fn loop_end_frames(&mut self, pcm_frames: u64) -> &mut Self {
        self.inner.loopPointEndInPCMFrames = pcm_frames;
        self
    }

    /// Sets the initial seek position, in PCM frames.
    ///
    /// Playback starts from this frame instead of the beginning.
    pub fn seek_point_frames(&mut self, pcm_frames: u64) -> &mut Self {
        self.inner.initialSeekPointInPCMFrames = pcm_frames;
        self
    }

    /// Sets the volume smoothing time, in PCM frames.
    ///
    /// Alternative to `range_begin_frames`. Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn volume_smooth_millis(&mut self, millis: f64) -> &mut Self {
        self.inner.volumeSmoothTimeInPCMFrames = self.millis_to_frames(millis) as u32;
        self
    }

    /// Anything before this point is skipped during playback.
    ///
    /// Alternative to `range_begin_frames`. Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn range_begin_millis(&mut self, millis: f64) -> &mut Self {
        self.inner.rangeBegInPCMFrames = self.millis_to_frames(millis);
        self
    }

    /// Playback stops when this frame is reached.
    ///
    /// Alternative to `range_end_frames`. Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn range_end_millis(&mut self, millis: f64) -> &mut Self {
        self.inner.rangeEndInPCMFrames = self.millis_to_frames(millis);
        self
    }

    /// Alternative to `loop_begin_frames`. Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn loop_begin_millis(&mut self, millis: f64) -> &mut Self {
        self.inner.loopPointBegInPCMFrames = self.millis_to_frames(millis);
        self
    }

    /// Alternative to `loop_end_frames`. Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn loop_end_millis(&mut self, millis: f64) -> &mut Self {
        self.inner.loopPointEndInPCMFrames = self.millis_to_frames(millis);
        self
    }

    /// Alternative to `seek_point_frames`. Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn seek_point_millis(&mut self, millis: f64) -> &mut Self {
        self.inner.initialSeekPointInPCMFrames = self.millis_to_frames(millis);
        self
    }

    /// Convenience method for calling [`Self::range_begin_frames`] and [`Self::range_end_frames`] in the same call
    pub fn range_frames(&mut self, begin: u64, end: u64) -> &mut Self {
        self.inner.rangeBegInPCMFrames = begin;
        self.inner.rangeEndInPCMFrames = end;
        self
    }

    /// Convenience method for calling [`Self::range_begin_millis`] and [`Self::range_end_millis`] in the same call
    ///
    /// Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn range_millis(&mut self, begin: f64, end: f64) -> &mut Self {
        self.inner.rangeBegInPCMFrames = self.millis_to_frames(begin);
        self.inner.rangeEndInPCMFrames = self.millis_to_frames(end);
        self
    }

    /// Convenience method for calling [`Self::loop_begin_frames`] and [`Self::loop_end_frames`] in the same call
    pub fn loop_frames(&mut self, begin: u64, end: u64) -> &mut Self {
        self.inner.loopPointBegInPCMFrames = begin;
        self.inner.loopPointEndInPCMFrames = end;
        self
    }

    /// Convenience method for calling [`Self::loop_begin_millis`] and [`Self::loop_end_millis`] in the same call
    ///
    /// Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn loop_millis(&mut self, begin: f64, end: f64) -> &mut Self {
        self.inner.loopPointBegInPCMFrames = self.millis_to_frames(begin);
        self.inner.loopPointEndInPCMFrames = self.millis_to_frames(end);
        self
    }

    /// Equivalent to adding [SoundFlags::LOOPING]
    ///
    /// Does not modify any other existing flags
    pub fn looping(&mut self, yes: bool) -> &mut Self {
        // self.inner.isLooping is deprecated
        let mut flags = SoundFlags::from_bits(self.inner.flags);
        if yes {
            flags.insert(SoundFlags::LOOPING);
        } else {
            flags.remove(SoundFlags::LOOPING);
        }
        self.inner.flags = flags.bits();
        self.flags = flags;
        self
    }

    /// Equivalent to adding [SoundFlags::STREAM]
    ///
    /// Does not modify any other existing flags
    pub fn streaming(&mut self, yes: bool) -> &mut Self {
        let mut flags = SoundFlags::from_bits(self.inner.flags);
        if yes {
            flags.insert(SoundFlags::STREAM);
        } else {
            flags.remove(SoundFlags::STREAM);
        }
        self.inner.flags = flags.bits();
        self.flags = flags;
        self
    }

    /// Equivalent to adding [SoundFlags::DECODE]
    ///
    /// Does not modify any other existing flags
    pub fn decode(&mut self, yes: bool) -> &mut Self {
        let mut flags = SoundFlags::from_bits(self.inner.flags);
        if yes {
            flags.insert(SoundFlags::DECODE);
        } else {
            flags.remove(SoundFlags::DECODE);
        }
        self.inner.flags = flags.bits();
        self.flags = flags;
        self
    }

    /// Equivalent to adding [SoundFlags::ASYNC]
    ///
    /// Does not modify any other existing flags
    pub fn async_load(&mut self, yes: bool) -> &mut Self {
        let mut flags = SoundFlags::from_bits(self.inner.flags);
        if yes {
            flags.insert(SoundFlags::ASYNC);
        } else {
            flags.remove(SoundFlags::ASYNC);
        }
        self.inner.flags = flags.bits();
        self.flags = flags;
        self
    }

    /// Sets the `min_distance` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_min_distance`]
    pub fn min_distance(&mut self, d: f32) -> &mut Self {
        self.sound_state.min_distance = Some(d);
        self
    }

    /// Sets the `max_distance` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_max_distance`]
    pub fn max_distance(&mut self, d: f32) -> &mut Self {
        self.sound_state.max_distance = Some(d);
        self
    }

    /// Sets the `rolloff` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_rolloff`]
    pub fn rolloff(&mut self, r: f32) -> &mut Self {
        self.sound_state.rolloff = Some(r);
        self
    }

    /// Sets the `position` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_position`]
    pub fn position(&mut self, position: Vec3) -> &mut Self {
        self.sound_state.position = Some(position);
        self
    }

    /// Sets the `velocity` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_velocity`]
    pub fn velocity(&mut self, velocity: Vec3) -> &mut Self {
        self.sound_state.velocity = Some(velocity);
        self
    }

    /// Sets the `direction` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_direction`]
    pub fn direction(&mut self, direction: Vec3) -> &mut Self {
        self.sound_state.direction = Some(direction);
        self
    }

    /// Equivalent to calling [`Sound::play_sound()`] after sound is initialized
    pub fn start_playing(&mut self, yes: bool) -> &mut Self {
        self.sound_state.start_playing = yes;
        self
    }

    #[inline]
    fn millis_to_frames(&self, millis: f64) -> u64 {
        if !millis.is_finite() || millis <= 0.0 {
            return 0;
        }
        let sr = self.engine.sample_rate() as f64;
        (millis.max(0.0) * sr / 1000.0).round() as u64
    }

    #[inline]
    fn seconds_to_frames(&self, seconds: f64) -> u64 {
        if !seconds.is_finite() || seconds <= 0.0 {
            return 0;
        }
        let sr = self.engine.sample_rate() as f64;
        (seconds.max(0.0) * sr).round() as u64
    }

    fn configure_sound(&self, sound: &mut Sound) {
        if let Some(min_d) = self.sound_state.min_distance {
            sound.set_min_distance(min_d)
        };
        if let Some(max_d) = self.sound_state.max_distance {
            sound.set_max_distance(max_d)
        };
        if let Some(r) = self.sound_state.rolloff {
            sound.set_rolloff(r);
        }
        if let Some(p) = self.sound_state.position {
            sound.set_position(p);
        }
        if let Some(v) = self.sound_state.velocity {
            sound.set_velocity(v);
        }
        if let Some(d) = self.sound_state.direction {
            sound.set_direction(d);
        }
    }

    /// Some flags don't make sense without a source.
    fn check_flags_without_source(&self) -> MaResult<()> {
        let invalid_flags: SoundFlags =
            SoundFlags::STREAM | SoundFlags::DECODE | SoundFlags::ASYNC | SoundFlags::WAIT_INIT;

        if self.flags.intersects(invalid_flags) {
            return Err(crate::MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_ARGS,
            ));
        }

        Ok(())
    }

    /// Sets the source in the config. This way, we can call ma_sound_init_ex for all source types (including None)
    fn set_source(&mut self) -> MaResult<()> {
        let null_fields = |cfg: &mut SoundBuilder| {
            cfg.inner.pDataSource = core::ptr::null_mut();
            cfg.inner.pFilePath = core::ptr::null();
            cfg.inner.pFilePathW = core::ptr::null();
        };
        match self.source {
            SoundSource::None => null_fields(self),
            SoundSource::DataSource(src) => {
                null_fields(self);
                self.inner.pDataSource = private_data_source::source_ptr(&src);
            }
            #[cfg(unix)]
            SoundSource::FileUtf8(p) => {
                null_fields(self);
                let cstring = crate::engine::cstring_from_path(p)?;
                self.inner.pFilePath = cstring.as_ptr();
                self.owned_path = OwnedPathBuf::Utf8(cstring); // keep the pointer alive
            }
            #[cfg(windows)]
            SoundSource::FileWide(p) => {
                null_fields(self);
                let wide_path = crate::engine::wide_null_terminated(p);
                self.inner.pFilePathW = wide_path.as_ptr();
                self.owned_path = OwnedPathBuf::Wide(wide_path); // keep the pointer alive
            }
        }
        Ok(())
    }
}

impl<'a> SoundBuilder<'a> {
    pub(crate) fn init(inner: &'a Engine) -> Self {
        let ptr = unsafe { sys::ma_sound_config_init_2(inner.to_raw()) };
        let state = SoundState::default();
        Self {
            inner: ptr,
            engine: inner,
            source: SoundSource::None,
            owned_path: OwnedPathBuf::None,
            group: None,
            fence: None,
            flags: SoundFlags::NONE,
            end_notifier: None,
            sound_state: state,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::engine::Engine;

    #[test]
    fn sound_builder_test_basic() {
        let engine = Engine::new_for_tests().unwrap();
        let _sound = engine.sound().channels_in(1).build().unwrap();
    }
}
