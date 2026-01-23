use std::{cell::Cell, marker::PhantomData, path::Path};

use maudio_sys::ffi as sys;

use crate::{
    Binding, MaRawResult, MaResult,
    audio::{
        dsp::pan::PanMode,
        math::vec3::Vec3,
        spatial::{attenuation::AttenuationModel, cone::Cone, positioning::Positioning},
    },
    data_source::{DataFormat, DataSourceRef},
    engine::{Engine, EngineRef, node_graph::nodes::NodeRef},
    notifier::EndNotifier,
    sound::{sound_flags::SoundFlags, sound_group::SoundGroup},
    util::fence::Fence,
};

pub mod sound_builder;
pub mod sound_flags;
pub mod sound_group;

/// The initialization source for a sound.
///
/// Only one source may be active at a time (file path OR data source).
#[derive(PartialEq)]
pub enum SoundSource<'a> {
    None,
    #[cfg(unix)]
    FileUtf8(&'a Path),
    #[cfg(windows)]
    FileWide(&'a Path),
    DataSource(DataSourceRef<'a>),
}

impl SoundSource<'_> {
    pub(crate) fn is_valid(&self) -> bool {
        !matches!(self, Self::None)
    }
}

pub struct Sound<'a> {
    inner: *mut sys::ma_sound,
    _engine: PhantomData<&'a Engine>,
    _not_sync: PhantomData<Cell<()>>,
    // Miniaudio stores only one ma_sound_end_proc and pUserData per ma_sound.
    // One end_notifier at a time will be ok
    end_notifier: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Binding for Sound<'_> {
    type Raw = *mut sys::ma_sound;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            _engine: PhantomData,
            _not_sync: PhantomData,
            end_notifier: None,
        }
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<'a> Sound<'a> {
    pub fn engine(&self) -> Option<EngineRef<'_>> {
        sound_ffi::ma_sound_get_engine(self)
    }

    /// Returns a **borrowed view** of this sound as a node in the engine's node graph.
    ///
    /// In miniaudio, sounds participate in the audio routing system as graph nodes.
    ///
    /// In addition to its high-level playback API, a sound can also be viewed as a node in the engine’s node graph.
    /// This method exposes that internal node so it can be connected, routed, or
    /// inspected using node-graph APIs.
    ///
    /// # What this is for
    ///
    /// Use `as_node()` when you want to:
    /// - connect this sound to other nodes (effects, mixers, splitters, etc.)
    /// - insert the sound into a custom routing graph
    /// - query node-level state exposed by the graph
    ///
    /// Most sound configuration (playback, volume, looping, spatialization, etc.)
    /// should be done through [`Sound`] methods directly, not through the node view.
    pub fn as_node(&self) -> NodeRef<'a> {
        debug_assert!(!self.inner.is_null());
        let ptr: *mut sys::ma_node = self.inner.cast::<sys::ma_node>();
        NodeRef::from_ptr(ptr)
    }

    pub fn data_source(&mut self) -> Option<DataSourceRef<'_>> {
        sound_ffi::ma_sound_get_data_source(self)
    }

    pub fn play_sound(&mut self) -> MaResult<()> {
        sound_ffi::ma_sound_start(self)
    }

    pub fn stop_sound(&mut self) -> MaResult<()> {
        sound_ffi::ma_sound_stop(self)
    }

    pub fn stop_at_with_fade_frames(&mut self, fade_frames: u64) -> MaResult<()> {
        sound_ffi::ma_sound_stop_with_fade_in_pcm_frames(self, fade_frames)
    }

    pub fn stop_at_with_fade_millis(&mut self, fade_milis: u64) -> MaResult<()> {
        sound_ffi::ma_sound_stop_with_fade_in_milis(self, fade_milis)
    }

    pub fn volume(&self) -> f32 {
        sound_ffi::ma_sound_get_volume(self)
    }

    pub fn set_volume(&mut self, volume: f32) {
        sound_ffi::ma_sound_set_volume(self, volume);
    }

    pub fn pan(&self) -> f32 {
        sound_ffi::ma_sound_get_pan(self)
    }

    pub fn set_pan(&mut self, pan: f32) {
        sound_ffi::ma_sound_set_pan(self, pan);
    }

    pub fn pan_mode(&self) -> MaResult<PanMode> {
        sound_ffi::ma_sound_get_pan_mode(self)
    }

    pub fn set_pan_mode(&mut self, mode: PanMode) {
        sound_ffi::ma_sound_set_pan_mode(self, mode);
    }

    pub fn pitch(&self) -> f32 {
        sound_ffi::ma_sound_get_pitch(self)
    }

    pub fn set_pitch(&mut self, pitch: f32) {
        sound_ffi::ma_sound_set_pitch(self, pitch);
    }

    pub fn spatialization(&self) -> bool {
        sound_ffi::ma_sound_is_spatialization_enabled(self)
    }

    pub fn set_spatialization(&mut self, enabled: bool) {
        sound_ffi::ma_sound_set_spatialization_enabled(self, enabled);
    }

    pub fn pinned_listener(&self) -> u32 {
        sound_ffi::ma_sound_get_pinned_listener_index(self)
    }

    pub fn set_pinned_listener(&mut self, listener: u32) {
        sound_ffi::ma_sound_set_pinned_listener_index(self, listener);
    }

    pub fn listener(&self) -> u32 {
        sound_ffi::ma_sound_get_listener_index(self)
    }

    pub fn direction_to_listener(&self) -> Vec3 {
        sound_ffi::ma_sound_get_direction_to_listener(self)
    }

    pub fn position(&self) -> Vec3 {
        sound_ffi::ma_sound_get_position(self)
    }

    pub fn set_position(&mut self, vec3: Vec3) {
        sound_ffi::ma_sound_set_position(self, vec3);
    }

    pub fn direction(&self) -> Vec3 {
        sound_ffi::ma_sound_get_direction(self)
    }

    pub fn set_direction(&mut self, vec3: Vec3) {
        sound_ffi::ma_sound_set_direction(self, vec3);
    }

    pub fn velocity(&self) -> Vec3 {
        sound_ffi::ma_sound_get_velocity(self)
    }

    pub fn set_velocity(&mut self, vec3: Vec3) {
        sound_ffi::ma_sound_set_velocity(self, vec3);
    }

    pub fn attenuation(&self) -> MaResult<AttenuationModel> {
        sound_ffi::ma_sound_get_attenuation_model(self)
    }

    pub fn set_attenuation(&mut self, model: AttenuationModel) {
        sound_ffi::ma_sound_set_attenuation_model(self, model);
    }

    pub fn positioning(&self) -> MaResult<Positioning> {
        sound_ffi::ma_sound_get_positioning(self)
    }
    pub fn set_positioning(&mut self, positioning: Positioning) {
        sound_ffi::ma_sound_set_positioning(self, positioning);
    }

    pub fn rolloff(&self) -> f32 {
        sound_ffi::ma_sound_get_rolloff(self)
    }

    pub fn set_rolloff(&mut self, rolloff: f32) {
        sound_ffi::ma_sound_set_rolloff(self, rolloff);
    }

    pub fn min_gain(&self) -> f32 {
        sound_ffi::ma_sound_get_min_gain(self)
    }

    pub fn set_min_gain(&mut self, gain: f32) {
        sound_ffi::ma_sound_set_min_gain(self, gain);
    }

    pub fn max_gain(&self) -> f32 {
        sound_ffi::ma_sound_get_max_gain(self)
    }

    pub fn set_max_gain(&mut self, gain: f32) {
        sound_ffi::ma_sound_set_max_gain(self, gain);
    }

    pub fn min_distance(&self) -> f32 {
        sound_ffi::ma_sound_get_min_distance(self)
    }

    pub fn set_min_distance(&mut self, distance: f32) {
        sound_ffi::ma_sound_set_min_distance(self, distance);
    }

    pub fn max_distance(&self) -> f32 {
        sound_ffi::ma_sound_get_max_distance(self)
    }

    pub fn set_max_distance(&mut self, distance: f32) {
        sound_ffi::ma_sound_set_max_distance(self, distance);
    }

    pub fn cone(&self) -> Cone {
        sound_ffi::ma_sound_get_cone(self)
    }

    pub fn set_cone(&mut self, cone: Cone) {
        sound_ffi::ma_sound_set_cone(self, cone);
    }

    pub fn doppler_factor(&self) -> f32 {
        sound_ffi::ma_sound_get_doppler_factor(self)
    }

    pub fn set_doppler_factor(&mut self, factor: f32) {
        sound_ffi::ma_sound_set_doppler_factor(self, factor);
    }

    pub fn directional_attenuation(&mut self) -> f32 {
        sound_ffi::ma_sound_get_directional_attenuation_factor(self)
    }

    pub fn set_directional_attenuation(&mut self, factor: f32) {
        sound_ffi::ma_sound_set_directional_attenuation_factor(self, factor);
    }

    pub fn set_fade_pcm(&mut self, vol_start: f32, vol_end: f32, fade_length_frames: u64) {
        sound_ffi::ma_sound_set_fade_in_pcm_frames(self, vol_start, vol_end, fade_length_frames);
    }

    pub fn set_fade_mili(&mut self, vol_start: f32, vol_end: f32, fade_length_mili: u64) {
        sound_ffi::ma_sound_set_fade_in_milliseconds(self, vol_start, vol_end, fade_length_mili);
    }

    pub fn set_fade_start_pcm(
        &mut self,
        vol_start: f32,
        vol_end: f32,
        fade_length_frames: u64,
        time_in_frames: u64,
    ) {
        sound_ffi::ma_sound_set_fade_start_in_pcm_frames(
            self,
            vol_start,
            vol_end,
            fade_length_frames,
            time_in_frames,
        );
    }

    pub fn set_fade_start_millis(
        &mut self,
        vol_start: f32,
        vol_end: f32,
        fade_length_mili: u64,
        time_in_frames: u64,
    ) {
        sound_ffi::ma_sound_set_fade_start_in_milliseconds(
            self,
            vol_start,
            vol_end,
            fade_length_mili,
            time_in_frames,
        );
    }

    pub fn current_fade_volume(&self) -> f32 {
        sound_ffi::ma_sound_get_current_fade_volume(self)
    }

    pub fn set_start_time_pcm(&mut self, abs_time_frames: u64) {
        sound_ffi::ma_sound_set_start_time_in_pcm_frames(self, abs_time_frames);
    }

    pub fn set_start_time_millis(&mut self, abs_time_millis: u64) {
        sound_ffi::ma_sound_set_start_time_in_milliseconds(self, abs_time_millis);
    }

    pub fn set_stop_time_pcm(&mut self, abs_time_frames: u64) {
        sound_ffi::ma_sound_set_stop_time_in_pcm_frames(self, abs_time_frames);
    }

    pub fn set_stop_time_millis(&mut self, abs_time_millis: u64) {
        sound_ffi::ma_sound_set_stop_time_in_milliseconds(self, abs_time_millis);
    }

    pub fn set_stop_time_with_fade_pcm(&mut self, stop_time_frames: u64, fade_length_frames: u64) {
        sound_ffi::ma_sound_set_stop_time_with_fade_in_pcm_frames(
            self,
            stop_time_frames,
            fade_length_frames,
        );
    }

    pub fn set_stop_time_with_fade_millis(
        &mut self,
        stop_time_millis: u64,
        fade_length_millis: u64,
    ) {
        sound_ffi::ma_sound_set_stop_time_with_fade_in_milliseconds(
            self,
            stop_time_millis,
            fade_length_millis,
        );
    }

    pub fn is_playing(&self) -> bool {
        sound_ffi::ma_sound_is_playing(self)
    }

    pub fn time_pcm(&self) -> u64 {
        sound_ffi::ma_sound_get_time_in_pcm_frames(self)
    }

    pub fn time_millis(&self) -> u64 {
        sound_ffi::ma_sound_get_time_in_milliseconds(self)
    }

    pub fn looping(&self) -> bool {
        sound_ffi::ma_sound_is_looping(self)
    }

    pub fn set_looping(&mut self, looping: bool) {
        sound_ffi::ma_sound_set_looping(self, looping);
    }

    pub fn ended(&self) -> bool {
        sound_ffi::ma_sound_at_end(self)
    }

    pub fn seek_to_pcm(&mut self, frame_index: u64) -> MaResult<()> {
        sound_ffi::ma_sound_seek_to_pcm_frame(self, frame_index)
    }

    pub fn seek_to_second(&mut self, seek_point_seconds: f32) -> MaResult<()> {
        sound_ffi::ma_sound_seek_to_second(self, seek_point_seconds)
    }

    fn data_format(&self) -> MaResult<DataFormat> {
        sound_ffi::ma_sound_get_data_format(self)
    }

    fn cursor_pcm(&self) -> MaResult<u64> {
        sound_ffi::ma_sound_get_cursor_in_pcm_frames(self)
    }

    fn length_pcm(&self) -> MaResult<u64> {
        sound_ffi::ma_sound_get_length_in_pcm_frames(self)
    }

    fn cursor_seconds(&self) -> MaResult<f32> {
        sound_ffi::ma_sound_get_cursor_in_seconds(self)
    }

    fn length_seconds(&self) -> MaResult<f32> {
        sound_ffi::ma_sound_get_length_in_seconds(self)
    }

    pub fn set_end_callback(&mut self) -> MaResult<EndNotifier> {
        let notifier = EndNotifier::new();
        self.end_notifier = Some(notifier.clone_flag());

        let user_data = notifier.as_user_data_ptr();

        let res = unsafe {
            sys::ma_sound_set_end_callback(
                self.to_raw(),
                Some(crate::notifier::on_end_callback),
                user_data,
            )
        };
        MaRawResult::check(res)?;

        Ok(notifier)
    }
}

// Private methods
impl<'a> Sound<'a> {
    pub(crate) fn init_from_file_internal(
        sound: *mut sys::ma_sound,
        engine: &Engine,
        path: &Path,
        flags: SoundFlags,
        sound_group: Option<&SoundGroup>,
        fence: Option<&Fence>,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            use crate::engine::cstring_from_path;

            let path = cstring_from_path(path)?;
            sound_ffi::ma_sound_init_from_file(engine, path, flags, sound_group, fence, sound)
        }
        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated;

            let path = wide_null_terminated(path);
            sound_ffi::ma_sound_init_from_file_w(engine, &path, flags, sound_group, fence, sound)
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init_sound_from_file is only supported on unix and windows");
    }
}

impl<'a> Drop for Sound<'a> {
    fn drop(&mut self) {
        unsafe {
            sys::ma_sound_uninit(self.to_raw());
        }
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

/// Converts a gain value expressed in decibels (dB) to a linear volume factor.
///
/// A value of `0.0` dB corresponds to a linear factor of `1.0` (no change),
/// negative values reduce the volume, and positive values increase it.
///
/// The returned value can be passed directly to [`Sound::set_volume`].
pub fn sound_volume_db_to_linear(gain: f32) -> f32 {
    unsafe { sys::ma_volume_db_to_linear(gain) }
}

/// Converts a linear volume factor to a gain value expressed in decibels (dB).
///
/// A factor of `1.0` corresponds to `0.0` dB, values less than `1.0` produce
/// negative decibel values, and values greater than `1.0` produce positive ones.
///
/// This is useful for inspecting or serializing the current volume in dB.
pub fn sound_volume_linear_to_db(factor: f32) -> f32 {
    unsafe { sys::ma_volume_linear_to_db(factor) }
}

pub(crate) mod sound_ffi {
    use maudio_sys::ffi as sys;

    use crate::Binding;
    use crate::MaResult;
    use crate::audio::math::vec3::Vec3;
    use crate::audio::spatial::{
        attenuation::AttenuationModel, cone::Cone, positioning::Positioning,
    };
    use crate::data_source::{DataFormat, DataSource, DataSourceRef, private_data_source};
    use crate::util::fence::Fence;
    use crate::{
        MaRawResult,
        audio::dsp::pan::PanMode,
        engine::{Engine, EngineRef},
        sound::{
            Sound, sound_builder::SoundBuilder, sound_flags::SoundFlags, sound_group::SoundGroup,
        },
    };

    #[inline]
    #[cfg(unix)]
    pub fn ma_sound_init_from_file(
        engine: &Engine,
        path: std::ffi::CString,
        flags: SoundFlags,
        s_group: Option<&SoundGroup>,
        done_fence: Option<&Fence>,
        sound: *mut sys::ma_sound,
    ) -> MaResult<()> {
        let s_group: *mut sys::ma_sound_group =
            s_group.map_or(core::ptr::null_mut(), |g| g.to_raw());
        let done_fence = done_fence.map_or(core::ptr::null_mut(), |f| f.to_raw());

        let res = unsafe {
            use crate::Binding;

            sys::ma_sound_init_from_file(
                engine.to_raw(),
                path.as_ptr(),
                flags.bits(),
                s_group,
                done_fence,
                sound,
            )
        };
        MaRawResult::check(res)
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_sound_init_from_file_w(
        engine: &Engine,
        path: &[u16],
        flags: SoundFlags,
        s_group: Option<&SoundGroup>,
        done_fence: Option<&    Fence>,
        sound: *mut sys::ma_sound,
    ) -> MaResult<()> {
        let s_group: *mut sys::ma_sound_group =
            s_group.map_or(core::ptr::null_mut(), |g| g.to_raw());
        let done_fence = done_fence.map_or(core::ptr::null_mut(), |f| f.to_raw());

        let res = unsafe {
            sys::ma_sound_init_from_file_w(
                engine.to_raw(),
                path.as_ptr(),
                flags.bits(),
                s_group,
                done_fence,
                sound,
            )
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_sound_init_copy(
        engine: &Engine,
        existing_sound: &Sound,
        flags: SoundFlags,
        s_group: Option<&mut SoundGroup>,
        new_sound: *mut sys::ma_sound,
    ) -> MaResult<()> {
        let s_group: *mut sys::ma_sound_group =
            s_group.map_or(core::ptr::null_mut(), |g| g.to_raw());

        let res = unsafe {
            sys::ma_sound_init_copy(
                engine.to_raw(),
                existing_sound.to_raw() as *const _,
                flags.bits(),
                s_group,
                new_sound,
            )
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_sound_init_from_data_source(
        engine: &Engine,
        data_source: &DataSource,
        flags: SoundFlags,
        s_group: Option<&SoundGroup>,
        sound: *mut sys::ma_sound,
    ) -> MaResult<()> {
        let s_group: *mut sys::ma_sound_group =
            s_group.map_or(core::ptr::null_mut(), |g| g.to_raw());

        let res = unsafe {
            sys::ma_sound_init_from_data_source(
                engine.to_raw(),
                private_data_source::source_ptr(data_source),
                flags.bits(),
                s_group,
                sound,
            )
        };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_sound_init_ex(
        engine: &Engine,
        config: &SoundBuilder,
        sound: *mut sys::ma_sound,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_sound_init_ex(engine.to_raw(), config.to_raw(), sound) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_sound_get_engine<'a>(sound: &'a Sound) -> Option<EngineRef<'a>> {
        let ptr = unsafe { sys::ma_sound_get_engine(sound.to_raw() as *const _) };
        if ptr.is_null() {
            None
        } else {
            Some(EngineRef::from_ptr(ptr))
        }
    }

    #[inline]
    pub fn ma_sound_get_data_source<'a>(sound: &'a mut Sound) -> Option<DataSourceRef<'a>> {
        let ptr = unsafe { sys::ma_sound_get_data_source(sound.to_raw() as *const _) };
        if ptr.is_null() {
            None
        } else {
            Some(DataSourceRef::from_ptr(ptr))
        }
    }

    #[inline]
    pub fn ma_sound_start(sound: &mut Sound) -> MaResult<()> {
        let res = unsafe { sys::ma_sound_start(sound.to_raw()) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_sound_stop(sound: &mut Sound) -> MaResult<()> {
        let res = unsafe { sys::ma_sound_stop(sound.to_raw()) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_sound_stop_with_fade_in_pcm_frames(
        sound: &mut Sound,
        fade_frames: u64,
    ) -> MaResult<()> {
        let res =
            unsafe { sys::ma_sound_stop_with_fade_in_pcm_frames(sound.to_raw(), fade_frames) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_sound_stop_with_fade_in_milis(sound: &mut Sound, fade_milis: u64) -> MaResult<()> {
        let res =
            unsafe { sys::ma_sound_stop_with_fade_in_milliseconds(sound.to_raw(), fade_milis) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_sound_set_volume(sound: &mut Sound, volume: f32) {
        unsafe { sys::ma_sound_set_volume(sound.to_raw(), volume) }
    }

    #[inline]
    pub fn ma_sound_get_volume(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_volume(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_pan(sound: &mut Sound, pan: f32) {
        unsafe { sys::ma_sound_set_pan(sound.to_raw(), pan) }
    }

    #[inline]
    pub fn ma_sound_get_pan(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_pan(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_pan_mode(sound: &mut Sound, mode: PanMode) {
        unsafe {
            sys::ma_sound_set_pan_mode(sound.to_raw(), mode.into());
        }
    }

    #[inline]
    pub fn ma_sound_get_pan_mode(sound: &Sound) -> MaResult<PanMode> {
        let res = unsafe { sys::ma_sound_get_pan_mode(sound.to_raw() as *const _) };
        res.try_into()
    }

    #[inline]
    pub fn ma_sound_set_pitch(sound: &mut Sound, pitch: f32) {
        unsafe { sys::ma_sound_set_pitch(sound.to_raw(), pitch) }
    }

    #[inline]
    pub fn ma_sound_get_pitch(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_pitch(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_spatialization_enabled(sound: &mut Sound, enabled: bool) {
        let enabled = enabled as sys::ma_bool32;
        unsafe { sys::ma_sound_set_spatialization_enabled(sound.to_raw(), enabled) }
    }

    #[inline]
    pub fn ma_sound_is_spatialization_enabled(sound: &Sound) -> bool {
        let res = unsafe { sys::ma_sound_is_spatialization_enabled(sound.to_raw() as *const _) };
        res == 1
    }

    #[inline]
    pub fn ma_sound_set_pinned_listener_index(sound: &mut Sound, listener_idx: u32) {
        unsafe { sys::ma_sound_set_pinned_listener_index(sound.to_raw(), listener_idx) }
    }

    #[inline]
    pub fn ma_sound_get_pinned_listener_index(sound: &Sound) -> u32 {
        unsafe { sys::ma_sound_get_pinned_listener_index(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_get_listener_index(sound: &Sound) -> u32 {
        unsafe { sys::ma_sound_get_listener_index(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_get_direction_to_listener(sound: &Sound) -> Vec3 {
        let vec = unsafe { sys::ma_sound_get_direction_to_listener(sound.to_raw() as *const _) };
        vec.into()
    }

    #[inline]
    pub fn ma_sound_set_position(sound: &mut Sound, vec3: Vec3) {
        unsafe {
            sys::ma_sound_set_position(sound.to_raw(), vec3.x, vec3.y, vec3.z);
        }
    }

    #[inline]
    pub fn ma_sound_get_position(sound: &Sound) -> Vec3 {
        let vec = unsafe { sys::ma_sound_get_position(sound.to_raw() as *const _) };
        vec.into()
    }

    #[inline]
    pub fn ma_sound_set_direction(sound: &mut Sound, vec3: Vec3) {
        unsafe { sys::ma_sound_set_direction(sound.to_raw(), vec3.x, vec3.y, vec3.z) }
    }

    #[inline]
    pub fn ma_sound_get_direction(sound: &Sound) -> Vec3 {
        let vec = unsafe { sys::ma_sound_get_direction(sound.to_raw() as *const _) };
        vec.into()
    }

    #[inline]
    pub fn ma_sound_set_velocity(sound: &mut Sound, vec3: Vec3) {
        unsafe { sys::ma_sound_set_velocity(sound.to_raw(), vec3.x, vec3.y, vec3.z) }
    }

    #[inline]
    pub fn ma_sound_get_velocity(sound: &Sound) -> Vec3 {
        let vec = unsafe { sys::ma_sound_get_velocity(sound.to_raw() as *const _) };
        vec.into()
    }

    #[inline]
    pub fn ma_sound_set_attenuation_model(sound: &mut Sound, model: AttenuationModel) {
        unsafe { sys::ma_sound_set_attenuation_model(sound.to_raw(), model.into()) }
    }

    #[inline]
    pub fn ma_sound_get_attenuation_model(sound: &Sound) -> MaResult<AttenuationModel> {
        let model = unsafe { sys::ma_sound_get_attenuation_model(sound.to_raw() as *const _) };
        model.try_into()
    }

    #[inline]
    pub fn ma_sound_set_positioning(sound: &mut Sound, positioning: Positioning) {
        unsafe { sys::ma_sound_set_positioning(sound.to_raw(), positioning.into()) }
    }

    #[inline]
    pub fn ma_sound_get_positioning(sound: &Sound) -> MaResult<Positioning> {
        let pos = unsafe { sys::ma_sound_get_positioning(sound.to_raw() as *const _) };
        pos.try_into()
    }

    #[inline]
    pub fn ma_sound_set_rolloff(sound: &mut Sound, rolloff: f32) {
        unsafe { sys::ma_sound_set_rolloff(sound.to_raw(), rolloff) }
    }

    #[inline]
    pub fn ma_sound_get_rolloff(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_rolloff(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_min_gain(sound: &mut Sound, min_gain: f32) {
        unsafe { sys::ma_sound_set_min_gain(sound.to_raw(), min_gain) }
    }

    #[inline]
    pub fn ma_sound_get_min_gain(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_min_gain(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_max_gain(sound: &mut Sound, max_gain: f32) {
        unsafe { sys::ma_sound_set_max_gain(sound.to_raw(), max_gain) }
    }

    #[inline]
    pub fn ma_sound_get_max_gain(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_max_gain(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_min_distance(sound: &mut Sound, min_distance: f32) {
        unsafe { sys::ma_sound_set_min_distance(sound.to_raw(), min_distance) }
    }

    #[inline]
    pub fn ma_sound_get_min_distance(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_min_distance(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_max_distance(sound: &mut Sound, max_distance: f32) {
        unsafe { sys::ma_sound_set_max_distance(sound.to_raw(), max_distance) }
    }

    #[inline]
    pub fn ma_sound_get_max_distance(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_max_distance(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_cone(sound: &mut Sound, cone: Cone) {
        unsafe {
            sys::ma_sound_set_cone(
                sound.to_raw(),
                cone.inner_angle_rad,
                cone.outer_angle_rad,
                cone.outer_gain,
            );
        }
    }

    #[inline]
    pub fn ma_sound_get_cone(sound: &Sound) -> Cone {
        let mut inner = 0.0f32;
        let mut outer = 0.0f32;
        let mut gain = 1.0f32;

        unsafe {
            sys::ma_sound_get_cone(
                sound.to_raw() as *const _,
                &mut inner,
                &mut outer,
                &mut gain,
            );
        }

        Cone {
            inner_angle_rad: inner,
            outer_angle_rad: outer,
            outer_gain: gain,
        }
    }

    #[inline]
    pub fn ma_sound_set_doppler_factor(sound: &mut Sound, doppler_factor: f32) {
        unsafe { sys::ma_sound_set_doppler_factor(sound.to_raw(), doppler_factor) }
    }

    #[inline]
    pub fn ma_sound_get_doppler_factor(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_doppler_factor(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_directional_attenuation_factor(
        sound: &mut Sound,
        dir_attenuation_factor: f32,
    ) {
        unsafe {
            sys::ma_sound_set_directional_attenuation_factor(
                sound.to_raw(),
                dir_attenuation_factor,
            );
        }
    }

    #[inline]
    pub fn ma_sound_get_directional_attenuation_factor(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_directional_attenuation_factor(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_fade_in_pcm_frames(
        sound: &mut Sound,
        volume_start: f32,
        volume_end: f32,
        fade_length_frames: u64,
    ) {
        unsafe {
            sys::ma_sound_set_fade_in_pcm_frames(
                sound.to_raw(),
                volume_start,
                volume_end,
                fade_length_frames,
            )
        };
    }

    #[inline]
    pub fn ma_sound_set_fade_in_milliseconds(
        sound: &mut Sound,
        volume_start: f32,
        volume_end: f32,
        fade_length_mili: u64,
    ) {
        unsafe {
            sys::ma_sound_set_fade_in_milliseconds(
                sound.to_raw(),
                volume_start,
                volume_end,
                fade_length_mili,
            );
        }
    }

    pub fn ma_sound_set_fade_start_in_pcm_frames(
        sound: &mut Sound,
        volume_start: f32,
        volume_end: f32,
        fade_length_pcm: u64,
        time_in_frames: u64,
    ) {
        unsafe {
            sys::ma_sound_set_fade_start_in_pcm_frames(
                sound.to_raw(),
                volume_start,
                volume_end,
                fade_length_pcm,
                time_in_frames,
            )
        }
    }

    pub fn ma_sound_set_fade_start_in_milliseconds(
        sound: &mut Sound,
        volume_start: f32,
        volume_end: f32,
        fade_length_mili: u64,
        time: u64,
    ) {
        unsafe {
            sys::ma_sound_set_fade_start_in_milliseconds(
                sound.to_raw(),
                volume_start,
                volume_end,
                fade_length_mili,
                time,
            )
        }
    }

    #[inline]
    pub fn ma_sound_get_current_fade_volume(sound: &Sound) -> f32 {
        unsafe { sys::ma_sound_get_current_fade_volume(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_start_time_in_pcm_frames(sound: &mut Sound, abs_time_frames: u64) {
        unsafe {
            sys::ma_sound_set_start_time_in_pcm_frames(sound.to_raw(), abs_time_frames);
        }
    }

    #[inline]
    pub fn ma_sound_set_start_time_in_milliseconds(sound: &mut Sound, abs_time_millis: u64) {
        unsafe {
            sys::ma_sound_set_start_time_in_milliseconds(sound.to_raw(), abs_time_millis);
        }
    }

    #[inline]
    pub fn ma_sound_set_stop_time_in_pcm_frames(sound: &mut Sound, abs_time_frames: u64) {
        unsafe {
            sys::ma_sound_set_stop_time_in_pcm_frames(sound.to_raw(), abs_time_frames);
        }
    }

    #[inline]
    pub fn ma_sound_set_stop_time_in_milliseconds(sound: &mut Sound, abs_time_mili: u64) {
        unsafe {
            sys::ma_sound_set_stop_time_in_milliseconds(sound.to_raw(), abs_time_mili);
        }
    }

    pub fn ma_sound_set_stop_time_with_fade_in_pcm_frames(
        sound: &mut Sound,
        stop_time_frames: u64,
        fade_length_frames: u64,
    ) {
        unsafe {
            sys::ma_sound_set_stop_time_with_fade_in_pcm_frames(
                sound.to_raw(),
                stop_time_frames,
                fade_length_frames,
            );
        }
    }

    pub fn ma_sound_set_stop_time_with_fade_in_milliseconds(
        sound: &mut Sound,
        stop_time_millis: u64,
        fade_length_millis: u64,
    ) {
        unsafe {
            sys::ma_sound_set_stop_time_with_fade_in_milliseconds(
                sound.to_raw(),
                stop_time_millis,
                fade_length_millis,
            );
        }
    }

    #[inline]
    pub fn ma_sound_is_playing(sound: &Sound) -> bool {
        let res = unsafe { sys::ma_sound_is_playing(sound.to_raw() as *const _) };
        res == 1
    }

    #[inline]
    pub fn ma_sound_get_time_in_pcm_frames(sound: &Sound) -> u64 {
        unsafe { sys::ma_sound_get_time_in_pcm_frames(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_get_time_in_milliseconds(sound: &Sound) -> u64 {
        unsafe { sys::ma_sound_get_time_in_milliseconds(sound.to_raw() as *const _) }
    }

    #[inline]
    pub fn ma_sound_set_looping(sound: &mut Sound, looping: bool) {
        let looping = looping as u32;
        unsafe {
            sys::ma_sound_set_looping(sound.to_raw(), looping);
        }
    }

    #[inline]
    pub fn ma_sound_is_looping(sound: &Sound) -> bool {
        let res = unsafe { sys::ma_sound_is_looping(sound.to_raw() as *const _) };
        res == 1
    }

    #[inline]
    pub fn ma_sound_at_end(sound: &Sound) -> bool {
        let res = unsafe { sys::ma_sound_at_end(sound.to_raw() as *const _) };
        res == 1
    }

    #[inline]
    pub fn ma_sound_seek_to_pcm_frame(sound: &mut Sound, frame_index: u64) -> MaResult<()> {
        let res = unsafe { sys::ma_sound_seek_to_pcm_frame(sound.to_raw(), frame_index) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_sound_seek_to_second(sound: &mut Sound, seek_point_seconds: f32) -> MaResult<()> {
        let res = unsafe { sys::ma_sound_seek_to_second(sound.to_raw(), seek_point_seconds) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_sound_get_data_format(sound: &Sound) -> MaResult<DataFormat> {
        let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        let mut channels: sys::ma_uint32 = 0;
        let mut sample_rate: sys::ma_uint32 = 0;

        let mut channel_map = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];
        let res = unsafe {
            sys::ma_sound_get_data_format(
                sound.to_raw(),
                &mut format_raw,
                &mut channels,
                &mut sample_rate,
                channel_map.as_mut_ptr(),
                channel_map.len(),
            )
        };
        MaRawResult::check(res)?;

        channel_map.truncate(channels as usize);

        Ok(DataFormat {
            format: format_raw.try_into()?,
            channels: channels as u32,
            sample_rate: sample_rate as u32,
            channel_map,
        })
    }

    #[inline]
    pub fn ma_sound_get_cursor_in_pcm_frames(sound: &Sound) -> MaResult<u64> {
        let mut cursor: sys::ma_uint64 = 0;
        let res = unsafe {
            sys::ma_sound_get_cursor_in_pcm_frames(sound.to_raw() as *const _, &mut cursor)
        };
        MaRawResult::check(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_sound_get_length_in_pcm_frames(sound: &Sound) -> MaResult<u64> {
        let mut length: sys::ma_uint64 = 0;
        let res = unsafe {
            sys::ma_sound_get_length_in_pcm_frames(sound.to_raw() as *const _, &mut length)
        };
        MaRawResult::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_sound_get_cursor_in_seconds(sound: &Sound) -> MaResult<f32> {
        let mut cursor: f32 = 0.0;
        let res =
            unsafe { sys::ma_sound_get_cursor_in_seconds(sound.to_raw() as *const _, &mut cursor) };
        MaRawResult::check(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_sound_get_length_in_seconds(sound: &Sound) -> MaResult<f32> {
        let mut length: f32 = 0.0;
        let res =
            unsafe { sys::ma_sound_get_length_in_seconds(sound.to_raw() as *const _, &mut length) };
        MaRawResult::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_sound_set_end_callback(
        sound: &mut Sound,
        callback: sys::ma_sound_end_proc,
        user_data: *mut core::ffi::c_void,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_sound_set_end_callback(sound.to_raw(), callback, user_data) };
        MaRawResult::check(res)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        audio::{
            dsp::pan::PanMode,
            math::vec3::Vec3,
            spatial::{attenuation::AttenuationModel, cone::Cone, positioning::Positioning},
        },
        data_source::sources::buffer::{AudioBufferBuilder, AudioBufferOps},
        engine::{Engine, EngineOps, node_graph::nodes::NodeOps},
    };

    fn assert_f32_eq(a: f32, b: f32) {
        assert!(
            (a - b).abs() <= 1.0e-6,
            "expected {a} ~= {b}, diff={}",
            (a - b).abs()
        );
    }

    fn assert_vec3_eq(a: Vec3, b: Vec3) {
        assert_f32_eq(a.x, b.x);
        assert_f32_eq(a.y, b.y);
        assert_f32_eq(a.z, b.z);
    }

    #[test]
    fn sound_test_cast_to_node() {
        let engine = Engine::new_for_tests().unwrap();
        let sound = engine.new_sound().unwrap();
        let node_ref = sound.as_node();
        let state = node_ref.state();
        assert!(state.is_ok());
        let _state = state.unwrap();
    }

    #[test]
    fn test_sound_play_stop_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.play_sound().unwrap();
        let _ = sound.is_playing();

        sound.stop_sound().unwrap();
        let _ = sound.is_playing();
    }

    #[test]
    fn test_sound_stop_with_fade_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.play_sound().unwrap();

        sound.stop_at_with_fade_frames(128).unwrap();
        sound.stop_at_with_fade_millis(10).unwrap();
    }

    #[test]
    fn test_sound_volume_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_volume(0.25);
        assert_f32_eq(sound.volume(), 0.25);

        sound.set_volume(1.0);
        assert_f32_eq(sound.volume(), 1.0);
    }

    #[test]
    fn test_sound_pan_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_pan(-0.5);
        assert_f32_eq(sound.pan(), -0.5);

        sound.set_pan(0.5);
        assert_f32_eq(sound.pan(), 0.5);
    }

    #[test]
    fn test_sound_pan_mode_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_pan_mode(PanMode::Pan);
        assert_eq!(sound.pan_mode().unwrap(), PanMode::Pan);

        sound.set_pan_mode(PanMode::Balance);
        assert_eq!(sound.pan_mode().unwrap(), PanMode::Balance);
    }

    #[test]
    fn test_sound_pitch_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_pitch(0.75);
        assert_f32_eq(sound.pitch(), 0.75);

        sound.set_pitch(1.25);
        assert_f32_eq(sound.pitch(), 1.25);
    }

    #[test]
    fn test_sound_spatialization_toggle() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_spatialization(false);
        assert!(!sound.spatialization());

        sound.set_spatialization(true);
        assert!(sound.spatialization());
    }

    #[test]
    fn test_sound_pinned_listener_set_get() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        // If the engine only has 1 listener, 0 is the only valid value.
        let n = engine.listener_count();
        if n < 2 {
            sound.set_pinned_listener(0);
            assert_eq!(sound.pinned_listener(), 0);
            return;
        }

        sound.set_pinned_listener(0);
        assert_eq!(sound.pinned_listener(), 0);

        sound.set_pinned_listener(1);
        assert_eq!(sound.pinned_listener(), 1);
    }

    #[test]
    fn test_sound_listener_index_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let sound = engine.new_sound().unwrap();

        let idx = sound.listener();
        assert!(idx < engine.listener_count());
    }

    #[test]
    fn test_sound_direction_to_listener_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        // Give it a non-zero position so direction is better defined.
        sound.set_position(Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        let _dir = sound.direction_to_listener();
        // Should not assert exact values because listener positions can vary by backend/config.
    }

    #[test]
    fn test_sound_position_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        let p = Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        sound.set_position(p);
        assert_vec3_eq(sound.position(), p);
    }

    #[test]
    fn test_sound_direction_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        let d = Vec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        };
        sound.set_direction(d);
        assert_vec3_eq(sound.direction(), d);
    }

    #[test]
    fn test_sound_velocity_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        let v = Vec3 {
            x: -1.0,
            y: 0.5,
            z: 10.0,
        };
        sound.set_velocity(v);
        assert_vec3_eq(sound.velocity(), v);
    }

    #[test]
    fn test_sound_attenuation_model_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_attenuation(AttenuationModel::Inverse);
        assert_eq!(sound.attenuation().unwrap(), AttenuationModel::Inverse);

        sound.set_attenuation(AttenuationModel::Linear);
        assert_eq!(sound.attenuation().unwrap(), AttenuationModel::Linear);
    }

    #[test]
    fn test_sound_positioning_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_positioning(Positioning::Absolute);
        assert_eq!(sound.positioning().unwrap(), Positioning::Absolute);

        sound.set_positioning(Positioning::Relative);
        assert_eq!(sound.positioning().unwrap(), Positioning::Relative);
    }

    #[test]
    fn test_sound_rolloff_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_rolloff(0.5);
        assert_f32_eq(sound.rolloff(), 0.5);

        sound.set_rolloff(2.0);
        assert_f32_eq(sound.rolloff(), 2.0);
    }

    #[test]
    fn test_sound_min_max_gain_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_min_gain(0.1);
        assert_f32_eq(sound.min_gain(), 0.1);

        sound.set_max_gain(0.9);
        assert_f32_eq(sound.max_gain(), 0.9);
    }

    #[test]
    fn test_sound_min_max_distance_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_min_distance(1.0);
        assert_f32_eq(sound.min_distance(), 1.0);

        sound.set_max_distance(100.0);
        assert_f32_eq(sound.max_distance(), 100.0);
    }

    #[test]
    fn test_sound_cone_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        let cone = Cone {
            inner_angle_rad: 0.5,
            outer_angle_rad: 1.0,
            outer_gain: 0.25,
        };

        sound.set_cone(cone);
        let got = sound.cone();

        assert_f32_eq(got.inner_angle_rad, cone.inner_angle_rad);
        assert_f32_eq(got.outer_angle_rad, cone.outer_angle_rad);
        assert_f32_eq(got.outer_gain, cone.outer_gain);
    }

    #[test]
    fn test_sound_doppler_factor_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_doppler_factor(0.25);
        assert_f32_eq(sound.doppler_factor(), 0.25);

        sound.set_doppler_factor(2.0);
        assert_f32_eq(sound.doppler_factor(), 2.0);
    }

    #[test]
    fn test_sound_directional_attenuation_roundtrip() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_directional_attenuation(0.2);
        assert_f32_eq(sound.directional_attenuation(), 0.2);

        sound.set_directional_attenuation(1.0);
        assert_f32_eq(sound.directional_attenuation(), 1.0);
    }

    #[test]
    fn test_sound_fade_apis_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_fade_pcm(1.0, 0.0, 128);
        sound.set_fade_mili(1.0, 0.0, 10);

        sound.set_fade_start_pcm(1.0, 0.0, 128, 0);
        sound.set_fade_start_millis(1.0, 0.0, 10, 0);

        let _ = sound.current_fade_volume();
    }

    #[test]
    fn test_sound_start_stop_times_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        sound.set_start_time_pcm(0);
        sound.set_start_time_millis(0);

        sound.set_stop_time_pcm(0);
        sound.set_stop_time_millis(0);

        sound.set_stop_time_with_fade_pcm(0, 128);
        sound.set_stop_time_with_fade_millis(0, 10);
    }

    fn ramp_f32_interleaved(channels: u32, frames: u64) -> Vec<f32> {
        let mut data = vec![0.0f32; (channels as usize) * (frames as usize)];
        for f in 0..frames as usize {
            for c in 0..channels as usize {
                // unique value per (frame, channel)
                data[f * channels as usize + c] = (f as f32) * 10.0 + (c as f32);
            }
        }
        data
    }

    #[test]
    fn test_sound_looping_toggle() {
        let engine = Engine::new_for_tests().unwrap();
        let data = ramp_f32_interleaved(2, 32);

        let buf = AudioBufferBuilder::from_f32(2, 32, &data)
            .unwrap()
            .build_copy()
            .unwrap();

        let src = buf.as_source();

        let mut sound = engine.sound().data_source(&src).build().unwrap();

        sound.set_looping(false);
        assert!(!sound.looping());

        sound.set_looping(true);
        assert!(sound.looping());
    }

    #[test]
    fn test_sound_time_queries_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let sound = engine.new_sound().unwrap();

        let _t0 = sound.time_pcm();
        let _t1 = sound.time_millis();
    }

    #[test]
    fn test_sound_ended_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let sound = engine.new_sound().unwrap();

        let _ = sound.ended();
    }

    #[test]
    fn test_sound_seek_apis_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let mut sound = engine.new_sound().unwrap();

        let _ = sound.seek_to_pcm(0);
        let _ = sound.seek_to_second(0.0);
    }

    #[test]
    fn test_sound_data_format_and_ranges_smoke() {
        let engine = Engine::new_for_tests().unwrap();
        let sound = engine.new_sound().unwrap();

        // These may fail depending on how your test sound is created.
        // Still valuable: if they succeed, basic sanity checks; if not, no panic.
        if let Ok(df) = sound.data_format() {
            // Optional: assert df.channels > 0 etc, depending on your DataFormat type.
            let _ = df;
        }

        let _ = sound.cursor_pcm();
        let _ = sound.length_pcm();
        let _ = sound.cursor_seconds();
        let _ = sound.length_seconds();
    }
}
