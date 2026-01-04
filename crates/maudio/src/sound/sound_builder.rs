use std::path::Path;

use maudio_sys::ffi as sys;

use crate::{
    Result,
    engine::Engine,
    sound::{Sound, sound_flags::SoundFlags},
};

/// Builder for constructing a [`Sound`]
///
/// # What this is for
/// Miniaudio exposes `ma_sound_config` as a configuration struct that is filled out
/// and then used to create a sound (ma_sound). `SoundBuilder` is the wrapper around that pattern:
///
/// - You configure *how the sound should be initialized* (source, flags, routing, channels, etc.)
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
/// # fn demo(mut engine: Engine) -> Result<()> {
/// let sound = SoundBuilder::new(&mut engine)
///     .file_path(Path::new("assets/click.wav"))?
///     .channels_in(1)
///     .channels_out(0) // 0 = engine's native channel count
///     .build(&mut engine)?;
/// # Ok(())
/// # }
/// ```
///
/// Initialize from an existing data source:
///
/// ```no_run
/// # fn demo(mut engine: Engine, ds: *mut sys::ma_data_source) -> Result<()> {
/// let sound = SoundBuilder::new(&mut engine)
///     .data_source(ds)
///     .build(&mut engine)?;
/// # Ok(())
/// # }
/// ```
///
/// Attach to a node/bus on init:
///
/// ```no_run
/// # fn demo(mut engine: Engine, node: *mut sys::ma_node) -> Result<()> {
/// let sound = SoundBuilder::new(&mut engine)
///     .file_path("assets/music.ogg".as_ref())?
///     .initial_attachment(node, 0)
///     .build(&mut engine)?;
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
    flags: SoundFlags,
    source: SoundSource<'a>,
    path_utf8: Option<std::ffi::CString>,
    path_wide: Option<Vec<u16>>,
}

/// The initialization source for a sound.
///
/// Only one source may be active at a time (file path OR data source).
#[derive(PartialEq)]
pub enum SoundSource<'a> {
    None,
    File(&'a Path),
    DataSource(*mut sys::ma_data_source),
}

impl<'a> SoundBuilder<'a> {
    pub fn new(engine: &Engine) -> Self {
        SoundBuilder::init(unsafe { engine.as_mut_ptr_from_ref() })
    }

    pub fn build(&self, engine: &mut Engine) -> Result<Sound<'_>> {
        if self.check_invalid_sources() {
            // TODO: Re-do errors
            return Err(crate::MaError(sys::ma_result_MA_INVALID_ARGS));
        }
        if let SoundSource::DataSource(src) = self.source
            && src.is_null()
        {
            return Err(crate::MaError(sys::ma_result_MA_INVALID_ARGS));
        }

        let mut sound = Sound::new_uninit(self.flags.clone());
        let res = unsafe {
            sys::ma_sound_init_ex(
                engine.assume_init_mut_ptr(),
                &self.inner,
                sound.maybe_uninit_mut_ptr(),
            )
        };
        crate::MaRawResult::resolve(res)?;
        sound.set_init();
        Ok(sound)
    }

    /// See [`SoundFlags`]
    pub fn flags(mut self, flags: SoundFlags) -> Self {
        self.flags.insert_bits(&flags);
        self.inner.flags = flags.bits();
        self
    }

    /// Sets the source of the sound from a path
    ///
    /// A sound can be initialized from **either** a file path **or** a data source,
    /// but not both. Calling this method overrides any previously set data_source.
    ///
    /// The provided path is converted to the platform-specific format required by
    /// miniaudio and is only used during sound initialization.
    pub fn file_path(mut self, path: &'a Path) -> Result<Self> {
        self.source = SoundSource::File(path);
        self.inner.pDataSource = core::ptr::null_mut();
        self.inner.pFilePath = core::ptr::null();
        self.inner.pFilePathW = core::ptr::null();

        #[cfg(unix)]
        {
            use crate::engine::cstring_from_path;

            let utf8_path = cstring_from_path(path)?;

            self.inner.pFilePath = utf8_path.as_ptr();
            self.path_utf8 = Some(utf8_path);
            Ok(self)
        }
        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated;

            let wide_path = cstring_from_path(path)?;

            self.inner.pFilePathW = wide_path.as_ptr();
            self.inner.path_wide = Some(wide_path);
            Ok(self)
        }
        // TODO. What other platforms can be added
        #[cfg(not(any(unix, windows)))]
        compile_error!("set_path_source is only supported on unix and windows");
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

        self.inner.pDataSource = core::ptr::null_mut();
        self.inner.pFilePath = core::ptr::null();
        self.inner.pFilePathW = core::ptr::null();

        self.inner.pDataSource = source;

        self.path_utf8 = None;
        self.path_wide = None;

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
    ///
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
}

impl<'a> SoundBuilder<'a> {
    pub(crate) fn init(inner: *mut sys::ma_engine) -> Self {
        let inner = unsafe { sys::ma_sound_config_init_2(inner) };
        Self {
            inner,
            flags: SoundFlags::NONE,
            source: SoundSource::None,
            path_utf8: None,
            path_wide: None,
        }
    }

    fn check_invalid_sources(&self) -> bool {
        self.source == SoundSource::None && self.path_utf8.is_none() && self.path_wide.is_none()
    }

    pub(crate) fn get_raw(&self) -> *const sys::ma_sound_config {
        &self.inner as *const _
    }
}

#[cfg(test)]
mod test {
    use crate::{engine::Engine, sound::sound_builder::SoundBuilder};

    #[test]
    fn sound_builder() {
        let mut engine = Engine::new().unwrap();
        let _s_config = SoundBuilder::new(&mut engine).channels_in(1);
    }
}
