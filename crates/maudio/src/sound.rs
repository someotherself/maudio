use std::{cell::Cell, marker::PhantomData, path::Path};

use maudio_sys::ffi as sys;

use crate::{
    Binding, ErrorKinds, Result,
    audio::{
        dsp::pan::PanMode,
        math::vec3::Vec3,
        spatial::{attenuation::AttenuationModel, cone::Cone, positioning::Positioning},
    },
    engine::{Engine, EngineRef, node_graph::nodes::NodeRef},
    sound::{sound_flags::SoundFlags, sound_group::SoundGroup},
};

pub mod data_source;
pub mod sound_builder;
pub mod sound_flags;
pub mod sound_group;

pub enum SoundError {}

impl From<SoundError> for ErrorKinds {
    fn from(e: SoundError) -> Self {
        ErrorKinds::Sound(e)
    }
}

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
    DataSource(*mut sys::ma_data_source),
}

pub struct Sound<'a> {
    inner: *mut sys::ma_sound,
    _engine: PhantomData<&'a Engine>,
    _not_sync: PhantomData<Cell<()>>,
}

impl Binding for Sound<'_> {
    type Raw = *mut sys::ma_sound;

    fn from_ptr(raw: Self::Raw) -> Self {
        Self {
            inner: raw,
            _engine: PhantomData,
            _not_sync: PhantomData,
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

    // TODO: Implement data source
    pub fn data_source(&mut self) {}

    pub fn play_sound(&mut self) -> Result<()> {
        sound_ffi::ma_sound_start(self)
    }

    pub fn stop_sound(&mut self) -> Result<()> {
        sound_ffi::ma_sound_stop(self)
    }

    pub fn stop_at_with_fade_frames(&mut self, fade_frames: u64) -> Result<()> {
        sound_ffi::ma_sound_stop_with_fade_in_pcm_frames(self, fade_frames)
    }

    pub fn stop_at_with_fade_millis(&mut self, fade_milis: u64) -> Result<()> {
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

    pub fn pan_mode(&self) -> Result<PanMode> {
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

    pub fn attenuation(&self) -> Result<AttenuationModel> {
        sound_ffi::ma_sound_get_attenuation_model(self)
    }

    pub fn set_attenuation(&mut self, model: AttenuationModel) {
        sound_ffi::ma_sound_set_attenuation_model(self, model);
    }

    pub fn positioning(&self) -> Result<Positioning> {
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

    pub fn set_start_time_mili(&mut self, abs_time_millis: u64) {
        sound_ffi::ma_sound_set_start_time_in_milliseconds(self, abs_time_millis);
    }

    pub fn set_stop_time_pcm(&mut self, abs_time_frames: u64) {
        sound_ffi::ma_sound_set_stop_time_in_pcm_frames(self, abs_time_frames);
    }

    pub fn set_stop_time_mili(&mut self, abs_time_millis: u64) {
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

    pub fn time_pcm(&mut self) -> u64 {
        sound_ffi::ma_sound_get_time_in_pcm_frames(self)
    }

    pub fn time_millis(&mut self) -> u64 {
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

    pub fn seek_to_pcm(&mut self, seek_point_frames: u64) -> Result<()> {
        sound_ffi::ma_sound_seek_to_pcm_frame(self, seek_point_frames)
    }

    pub fn seek_to_second(&mut self, seek_point_seconds: f32) -> Result<()> {
        sound_ffi::ma_sound_seek_to_second(self, seek_point_seconds)
    }

    // TODO
    fn data_format(
        &self,
        format: *mut sys::ma_format,
        channels: *mut sys::ma_uint32,
        sample_rate: *mut sys::ma_uint32,
        channel_map: *mut sys::ma_channel,
        channel_map_cap: usize,
    ) -> Result<()> {
        sound_ffi::ma_sound_get_data_format(
            self,
            format,
            channels,
            sample_rate,
            channel_map,
            channel_map_cap,
        )
    }

    fn cursor_pcm(&self) -> Result<u64> {
        sound_ffi::ma_sound_get_cursor_in_pcm_frames(self)
    }

    fn length_pcm(&self) -> Result<u64> {
        sound_ffi::ma_sound_get_length_in_pcm_frames(self)
    }

    fn cursor_seconds(&self) -> Result<f32> {
        sound_ffi::ma_sound_get_cursor_in_seconds(self)
    }

    fn length_seconds(&self) -> Result<f32> {
        sound_ffi::ma_sound_get_length_in_seconds(self)
    }

    // TODO
    fn set_end_callback(
        &mut self,
        callback: sys::ma_sound_end_proc,
        user_data: *mut core::ffi::c_void,
    ) -> Result<()> {
        sound_ffi::ma_sound_set_end_callback(self, callback, user_data)
    }
}

// Private methods
impl<'a> Sound<'a> {
    // TODO: Wrap ma_fence
    pub(crate) fn init_from_file_internal(
        sound: *mut sys::ma_sound,
        engine: &Engine,
        path: &Path,
        flags: SoundFlags,
        sound_group: &mut Option<&SoundGroup>,
        fence: Option<*mut sys::ma_fence>,
    ) -> Result<()> {
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

        // TODO. What other platforms can be added
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
    use crate::Result;
    use crate::audio::math::vec3::Vec3;
    use crate::audio::spatial::{
        attenuation::AttenuationModel, cone::Cone, positioning::Positioning,
    };
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
        s_group: &mut Option<&SoundGroup>,
        done_fence: Option<*mut sys::ma_fence>,
        sound: *mut sys::ma_sound,
    ) -> Result<()> {
        let s_group: *mut sys::ma_sound_group =
            s_group.map_or(core::ptr::null_mut(), |g| g.to_raw());
        let done_fence = done_fence.unwrap_or(core::ptr::null_mut());

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
        MaRawResult::resolve(res)
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_sound_init_from_file_w(
        engine: &Engine,
        path: &[u16],
        flags: SoundFlags,
        s_group: &mut Option<&SoundGroup>,
        done_fence: Option<*mut sys::ma_fence>,
        sound: *mut sys::ma_sound,
    ) -> Result<()> {
        let s_group: *mut sys::ma_sound_group =
            s_group.map_or(core::ptr::null_mut(), |g| g.to_raw());
        let done_fence = done_fence.unwrap_or(core::ptr::null_mut());

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
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_sound_init_copy(
        engine: &Engine,
        existing_sound: &Sound,
        flags: SoundFlags,
        s_group: Option<&mut SoundGroup>,
        new_sound: *mut sys::ma_sound,
    ) -> Result<()> {
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
        MaRawResult::resolve(res)
    }

    // TODO: Implement data sources
    #[inline]
    pub fn ma_sound_init_from_data_source(
        engine: &Engine,
        data_source: *mut sys::ma_data_source,
        flags: SoundFlags,
        s_group: &mut SoundGroup,
        sound: &Sound,
    ) -> Result<()> {
        let res = unsafe {
            sys::ma_sound_init_from_data_source(
                engine.to_raw(),
                data_source,
                flags.bits(),
                s_group.to_raw(),
                sound.to_raw(),
            )
        };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_sound_init_ex(
        engine: &Engine,
        config: &SoundBuilder,
        sound: *mut sys::ma_sound,
    ) -> Result<()> {
        let res = unsafe { sys::ma_sound_init_ex(engine.to_raw(), config.to_raw(), sound) };
        MaRawResult::resolve(res)
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

    // TODO: Implement DataSource and DataSourceRef
    #[inline]
    pub fn ma_sound_get_data_source(_sound: &mut Sound) -> Option<*mut sys::ma_data_source> {
        // let ptr = unsafe { sys::ma_sound_get_data_source(sound.assume_init_ptr()) };
        // NonNull::new(ptr).map(|nn| DataSourceRef::from_ptr(nn))
        todo!()
    }

    #[inline]
    pub fn ma_sound_start(sound: &mut Sound) -> Result<()> {
        let res = unsafe { sys::ma_sound_start(sound.to_raw()) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_sound_stop(sound: &mut Sound) -> Result<()> {
        let res = unsafe { sys::ma_sound_stop(sound.to_raw()) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_sound_stop_with_fade_in_pcm_frames(
        sound: &mut Sound,
        fade_frames: u64,
    ) -> Result<()> {
        let res =
            unsafe { sys::ma_sound_stop_with_fade_in_pcm_frames(sound.to_raw(), fade_frames) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_sound_stop_with_fade_in_milis(sound: &mut Sound, fade_milis: u64) -> Result<()> {
        let res =
            unsafe { sys::ma_sound_stop_with_fade_in_milliseconds(sound.to_raw(), fade_milis) };
        MaRawResult::resolve(res)
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
    pub fn ma_sound_get_pan_mode(sound: &Sound) -> Result<PanMode> {
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
    pub fn ma_sound_get_attenuation_model(sound: &Sound) -> Result<AttenuationModel> {
        let model = unsafe { sys::ma_sound_get_attenuation_model(sound.to_raw() as *const _) };
        model.try_into()
    }

    #[inline]
    pub fn ma_sound_set_positioning(sound: &mut Sound, positioning: Positioning) {
        unsafe { sys::ma_sound_set_positioning(sound.to_raw(), positioning.into()) }
    }

    #[inline]
    pub fn ma_sound_get_positioning(sound: &Sound) -> Result<Positioning> {
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
    pub fn ma_sound_seek_to_pcm_frame(sound: &mut Sound, frame_index: u64) -> Result<()> {
        let res = unsafe { sys::ma_sound_seek_to_pcm_frame(sound.to_raw(), frame_index) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_sound_seek_to_second(sound: &mut Sound, seek_point_seconds: f32) -> Result<()> {
        let res = unsafe { sys::ma_sound_seek_to_second(sound.to_raw(), seek_point_seconds) };
        MaRawResult::resolve(res)
    }

    // TODO Implement data_format type?
    #[inline]
    pub fn ma_sound_get_data_format(
        sound: &Sound,
        format: *mut sys::ma_format,
        channels: *mut sys::ma_uint32,
        sample_rate: *mut sys::ma_uint32,
        channel_map: *mut sys::ma_channel,
        channel_map_cap: usize,
    ) -> Result<()> {
        let res = unsafe {
            sys::ma_sound_get_data_format(
                sound.to_raw() as *const _,
                format,
                channels,
                sample_rate,
                channel_map,
                channel_map_cap,
            )
        };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_sound_get_cursor_in_pcm_frames(sound: &Sound) -> Result<u64> {
        let mut cursor: sys::ma_uint64 = 0;
        let res = unsafe {
            sys::ma_sound_get_cursor_in_pcm_frames(sound.to_raw() as *const _, &mut cursor)
        };
        MaRawResult::resolve(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_sound_get_length_in_pcm_frames(sound: &Sound) -> Result<u64> {
        let mut length: sys::ma_uint64 = 0;
        let res = unsafe {
            sys::ma_sound_get_length_in_pcm_frames(sound.to_raw() as *const _, &mut length)
        };
        MaRawResult::resolve(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_sound_get_cursor_in_seconds(sound: &Sound) -> Result<f32> {
        let mut cursor: f32 = 0.0;
        let res =
            unsafe { sys::ma_sound_get_cursor_in_seconds(sound.to_raw() as *const _, &mut cursor) };
        MaRawResult::resolve(res)?;
        Ok(cursor)
    }

    #[inline]
    pub fn ma_sound_get_length_in_seconds(sound: &Sound) -> Result<f32> {
        let mut length: f32 = 0.0;
        let res =
            unsafe { sys::ma_sound_get_length_in_seconds(sound.to_raw() as *const _, &mut length) };
        MaRawResult::resolve(res)?;
        Ok(length)
    }

    // TODO
    #[inline]
    pub fn ma_sound_set_end_callback(
        sound: &mut Sound,
        callback: sys::ma_sound_end_proc,
        user_data: *mut core::ffi::c_void,
    ) -> Result<()> {
        let res = unsafe { sys::ma_sound_set_end_callback(sound.to_raw(), callback, user_data) };
        MaRawResult::resolve(res)
    }
}

#[cfg(test)]
mod test {
    use crate::engine::{Engine, node_graph::nodes::NodeOps};

    #[test]
    fn sound_test_cast_to_node() {
        let engine = Engine::new_for_tests().unwrap();
        let sound = engine.new_sound().unwrap();
        let node_ref = sound.as_node();
        let state = node_ref.state();
        assert!(state.is_ok());
        let state = state.unwrap();
        println!("node state is: {:?}", state);
    }
}
