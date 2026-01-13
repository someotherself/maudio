use std::path::Path;

use maudio_sys::ffi as sys;

use crate::{
    Binding, Result,
    audio::math::vec3::Vec3,
    engine::{Engine, EngineOps, cstring_from_path},
    sound::{Sound, SoundSource, sound_flags::SoundFlags},
};

/// Builder for constructing a [`Sound`]
///
/// # What this is for
/// Miniaudio exposes `ma_sound_config` as a configuration struct that is filled out
/// and then used to create a sound (ma_sound). `SoundBuilder` is the wrapper around that pattern:
///
/// - You configure *how the sound should be initialized*
/// - Then you "build" it once, producing a fully initialized [`Sound`].
///
/// This is especially useful when you need more control than the convenience
/// constructors (for example: attaching the sound to a specific node/bus, selecting
/// input/output channel behavior, or initializing from a custom data source).
///
/// # What this is NOT
/// `SoundBuilder` configures *initialization-time* options only. Runtime properties
/// like volume, pitch, pan/spatialization, and looping state are controlled via
/// methods on [`Sound`] after it has been built (e.g. `sound.set_volume(...)`)
///
/// # Sound sources
/// A sound can be initialized from **at most one** of:
///
/// - a file path (`pFilePath` / `pFilePathW`)
/// - an existing miniaudio data source (`pDataSource`)
///
///
/// `SoundBuilder` keeps an owned copy of the path (`CString` on Unix, wide buffer on
/// Windows) so the raw pointer inside `ma_sound_config` remains valid until
/// [`SoundBuilder::build`] is called.
///
/// # Examples
///
/// Initialize from a file:
///
/// ```no_run
/// # use std::path::Path;
/// # use maudio::engine::Engine;
/// # use maudio::sound::sound_builder::SoundBuilder;
/// # fn demo(engine: &Engine) -> maudio::Result<()> {
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
/// # fn demo(engine: &Engine, ds: *mut maudio_sys::ffi::ma_data_source) -> maudio::Result<()> { // TODO
/// let sound = SoundBuilder::new(&engine).data_source(ds).build()?;
/// # Ok(())
/// # }
/// ```
///
/// Attach to a node/bus on init:
///
/// ```no_run
/// # use maudio::engine::Engine;
/// # use maudio::sound::sound_builder::SoundBuilder;
/// # fn demo(engine: &Engine) -> maudio::Result<()> {
/// let sound = SoundBuilder::new(&engine)
///     .file_path("assets/music.ogg".as_ref()).build()?;;
/// # Ok(())
/// }
/// ```
///
/// # Notes
/// - `SoundBuilder` is consumed by `build()` and should be used once, matching
///   miniaudio's "fill config → init" workflow.
/// - If you only need a simple sound, prefer the convenience constructors on [`Engine`] / [`Sound`].
pub struct SoundBuilder<'a> {
    inner: sys::ma_sound_config,
    engine: &'a Engine,
    source: SoundSource<'a>,
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

impl<'a> SoundBuilder<'a> {
    pub fn new(engine: &'a Engine) -> Self {
        SoundBuilder::init(engine)
    }

    // TODO: Write a documentation on why it doesn't take a sound group
    // TODO: and how this can be used to create a sound group
    // TODO: Add method to edit pInitialAttachment and initialAttachmentInputBusIndex fields
    // TODO: If build_from_file and source are not implemented, remove the Self::group()?
    pub fn build(mut self) -> Result<Sound<'a>> {
        self.set_source()?;

        let mut sound = self.engine.new_sound_with_config_internal(Some(&self))?;

        self.configure_sound(&mut sound);

        Ok(sound)
    }

    /// Explicitly sets the sound to have no playback source.
    ///
    /// This is a convenience method for creating a silent sound or clearing a
    /// previously configured source.
    ///
    /// If no source was previously added, this has no effect
    pub fn no_source(mut self) -> Self {
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
    pub fn file_path(mut self, path: &'a Path) -> Self {
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

    // TODO: wrap ma_data_source
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
    /// - point to a valid, initialized `ma_data_source` (TODO)
    /// - remain alive for the entire lifetime of the created sound
    ///
    /// The sound does **not** take ownership of the data source.
    ///
    /// # When to use this
    /// This method is intended for more advanced use cases, such as:
    ///
    /// - procedural or generated audio
    /// - streaming audio from memory or network sources
    /// - reusing a single data source across multiple sounds
    ///
    /// For simple file playback, prefer initializing the sound from a file path.
    pub fn data_source(mut self, source: *mut sys::ma_data_source) -> Self {
        self.source = SoundSource::DataSource(source);
        // self.inner.pDataSource = core::ptr::null_mut();
        // self.inner.pFilePath = core::ptr::null();
        // self.inner.pFilePathW = core::ptr::null();

        // self.inner.pDataSource = source;

        // self.path_utf8 = None;
        // self.path_wide = None;

        self
    }

    // TODO: Wrap ma_node to provide safety for node and input bus
    /// By default, a newly created sound is attached to the engine's main output graph,
    /// unless `SoundFlags::NO_DEFAULT_ATTACHMENT` is set in `flags`.
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
    pub fn initial_attachment(mut self, node: *mut sys::ma_node, input_bus: u32) -> Self {
        self.inner.pInitialAttachment = node;
        self.inner.initialAttachmentInputBusIndex = input_bus;
        self
    }

    /// Sets the number of input channels for the sound node.
    ///
    /// The "channel" does not refer to a speaker, sound channel or does it control spatialization directly.
    ///
    /// This controls how many channels miniaudio expects from the sound's data source.
    /// In most cases this should be left at `0`, which allows miniaudio to infer the
    /// channel count automatically from the source.
    ///
    /// This is primarily useful for custom or procedural data sources.
    pub fn channels_in(mut self, ch: u32) -> Self {
        self.inner.channelsIn = ch;
        self
    }
    /// Sets the number of output channels for the sound node.
    ///
    /// The "channel" does not refer to a speaker, sound channel or does it control spatialization directly.
    ///
    /// This controls how many channels the sound outputs into the node graph.
    /// A value of `0` means "use the engine's native channel count", which is the
    /// recommended default.
    ///
    /// Miniaudio will automatically convert between input and output channel counts
    /// as needed (e.g. mono → stereo).
    pub fn channels_out(mut self, ch: u32) -> Self {
        self.inner.channelsOut = ch;
        self
    }

    /// See [`SoundFlags`]
    pub fn flags(mut self, flags: SoundFlags) -> Self {
        self.inner.flags = flags.bits();
        self
    }

    pub fn looping(mut self, looping: bool) -> Self {
        self.inner.isLooping = looping as u32;
        self
    }

    pub fn volume_smooth_frames(mut self, pcm_frames: u32) -> Self {
        self.inner.volumeSmoothTimeInPCMFrames = pcm_frames;
        self
    }

    pub fn range_begin_frames(mut self, pcm_frames: u64) -> Self {
        self.inner.rangeBegInPCMFrames = pcm_frames;
        self
    }

    pub fn range_end_frames(mut self, pcm_frames: u64) -> Self {
        self.inner.rangeEndInPCMFrames = pcm_frames;
        self
    }

    pub fn loop_begin_frames(mut self, pcm_frames: u64) -> Self {
        self.inner.loopPointBegInPCMFrames = pcm_frames;
        self
    }

    pub fn loop_end_frames(mut self, pcm_frames: u64) -> Self {
        self.inner.loopPointEndInPCMFrames = pcm_frames;
        self
    }

    pub fn seek_point_frames(mut self, pcm_frames: u64) -> Self {
        self.inner.initialSeekPointInPCMFrames = pcm_frames;
        self
    }

    /// Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn volume_smooth_millis(mut self, millis: f64) -> Self {
        self.inner.volumeSmoothTimeInPCMFrames = self.millis_to_frames(millis) as u32;
        self
    }

    /// Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn range_begin_millis(mut self, millis: f64) -> Self {
        self.inner.rangeBegInPCMFrames = self.millis_to_frames(millis);
        self
    }

    /// Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn range_end_millis(mut self, millis: f64) -> Self {
        self.inner.rangeEndInPCMFrames = self.millis_to_frames(millis);
        self
    }

    /// Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn loop_begin_millis(mut self, millis: f64) -> Self {
        self.inner.loopPointBegInPCMFrames = self.millis_to_frames(millis);
        self
    }

    /// Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn loop_end_millis(mut self, millis: f64) -> Self {
        self.inner.loopPointEndInPCMFrames = self.millis_to_frames(millis);
        self
    }

    /// Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn seek_point_millis(mut self, millis: f64) -> Self {
        self.inner.initialSeekPointInPCMFrames = self.millis_to_frames(millis);
        self
    }

    /// Convenience method for calling [`Self::range_begin_frames`] and [`Self::range_end_frames`] in the same call
    pub fn range_frames(mut self, begin: u64, end: u64) -> Self {
        self.inner.rangeBegInPCMFrames = begin;
        self.inner.rangeEndInPCMFrames = end;
        self
    }

    /// Convenience method for calling [`Self::range_begin_millis`] and [`Self::range_end_millis`] in the same call
    ///
    /// Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn range_millis(mut self, begin: f64, end: f64) -> Self {
        self.inner.rangeBegInPCMFrames = self.millis_to_frames(begin);
        self.inner.rangeEndInPCMFrames = self.millis_to_frames(end);
        self
    }

    /// Convenience method for calling [`Self::loop_begin_frames`] and [`Self::loop_end_frames`] in the same call
    pub fn loop_frames(mut self, begin: u64, end: u64) -> Self {
        self.inner.loopPointBegInPCMFrames = begin;
        self.inner.loopPointEndInPCMFrames = end;
        self
    }

    /// Convenience method for calling [`Self::loop_begin_millis`] and [`Self::loop_end_millis`] in the same call
    ///
    /// Interprets `millis` in engine time and converts it to PCM frames using the engine sample rate.
    pub fn loop_millis(mut self, begin: f64, end: f64) -> Self {
        self.inner.loopPointBegInPCMFrames = self.millis_to_frames(begin);
        self.inner.loopPointEndInPCMFrames = self.millis_to_frames(end);
        self
    }

    /// Equivalent to adding [SoundFlags::STREAM]
    ///
    /// Does not modify any other existing flags
    pub fn streaming(mut self, yes: bool) -> Self {
        let mut flags = SoundFlags::from_bits(self.inner.flags);
        if yes {
            flags.insert(SoundFlags::STREAM);
        } else {
            flags.remove(SoundFlags::STREAM);
        }
        self.inner.flags = flags.bits();
        self
    }

    /// Equivalent to adding [SoundFlags::DECODE]
    ///
    /// Does not modify any other existing flags
    pub fn decode(mut self, yes: bool) -> Self {
        let mut flags = SoundFlags::from_bits(self.inner.flags);
        if yes {
            flags.insert(SoundFlags::DECODE);
        } else {
            flags.remove(SoundFlags::DECODE);
        }
        self.inner.flags = flags.bits();
        self
    }

    /// Equivalent to adding [SoundFlags::ASYNC]
    ///
    /// Does not modify any other existing flags
    pub fn async_load(mut self, yes: bool) -> Self {
        let mut flags = SoundFlags::from_bits(self.inner.flags);
        if yes {
            flags.insert(SoundFlags::ASYNC);
        } else {
            flags.remove(SoundFlags::ASYNC);
        }
        self.inner.flags = flags.bits();
        self
    }

    /// Sets the `min_distance` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_min_distance`]
    pub fn min_distance(mut self, d: f32) -> Self {
        self.sound_state.min_distance = Some(d);
        self
    }

    /// Sets the `max_distance` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_max_distance`]
    pub fn max_distance(mut self, d: f32) -> Self {
        self.sound_state.max_distance = Some(d);
        self
    }

    /// Sets the `rolloff` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_rolloff`]
    pub fn rolloff(mut self, r: f32) -> Self {
        self.sound_state.rolloff = Some(r);
        self
    }

    /// Sets the `position` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_position`]
    pub fn position(mut self, position: Vec3) -> Self {
        self.sound_state.position = Some(position);
        self
    }

    /// Sets the `velocity` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_velocity`]
    pub fn velocity(mut self, velocity: Vec3) -> Self {
        self.sound_state.velocity = Some(velocity);
        self
    }

    /// Sets the `direction` field on the newly created sound
    ///
    /// Equivalent to calling [`Sound::set_direction`]
    pub fn direction(mut self, direction: Vec3) -> Self {
        self.sound_state.direction = Some(direction);
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

    fn configure_sound(self, sound: &mut Sound) {
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

    fn set_source(&mut self) -> Result<()> {
        let null_fields = |cfg: &mut SoundBuilder| {
            cfg.inner.pDataSource = core::ptr::null_mut();
            cfg.inner.pFilePath = core::ptr::null();
            cfg.inner.pFilePathW = core::ptr::null();
        };
        match self.source {
            SoundSource::None => null_fields(self),
            SoundSource::DataSource(src) => {
                null_fields(self);
                self.inner.pDataSource = src;
            }
            #[cfg(unix)]
            SoundSource::FileUtf8(p) => {
                null_fields(self);
                let cstring = cstring_from_path(p)?;
                self.inner.pFilePath = cstring.as_ptr();
            }
            #[cfg(windows)]
            SoundSource::FileWide(p) => {
                null_fields(self);
                let wide_path = wide_null_terminated(p);
                self.inner.pFilePathW = wide_path.as_ptr();
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
            sound_state: state,
        }
    }
}

impl Binding for SoundBuilder<'_> {
    type Raw = *const sys::ma_sound_config;

    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        &self.inner as *const _
    }
}

#[cfg(test)]
mod test {
    use crate::{engine::Engine, sound::sound_builder::SoundBuilder};

    #[test]
    fn sound_builder_test_basic() {
        let engine = Engine::new().unwrap();
        let _s_config = SoundBuilder::new(&engine).channels_in(1);
    }
}
