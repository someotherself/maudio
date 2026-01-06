use std::{mem::MaybeUninit, pin::Pin};

use maudio_sys::ffi as sys;

use crate::{
    Result,
    audio::{
        dsp::pan::PanMode,
        math::vec3::Vec3,
        spatial::{attenuation::AttenuationModel, cone::Cone, positioning::Positioning},
    },
    engine::EngineRef,
};

pub struct SoundGroup {
    inner: Pin<Box<MaybeUninit<sys::ma_sound_group>>>,
    init: bool,
}

impl SoundGroup {
    pub(crate) fn new_uninit() -> Self {
        let inner = Box::pin(MaybeUninit::<sys::ma_sound_group>::uninit());
        Self { inner, init: false }
    }

    pub(crate) fn set_init(&mut self) {
        self.init = true
    }

    pub fn engine(&mut self) -> EngineRef<'_> {
        s_group_ffi::ma_sound_group_get_engine(self)
    }

    pub fn start(&mut self) -> Result<()> {
        s_group_ffi::ma_sound_group_start(self)
    }

    pub fn stop(&mut self) -> Result<()> {
        s_group_ffi::ma_sound_group_stop(self)
    }

    pub fn set_volume(&mut self, volume: f32) {
        s_group_ffi::ma_sound_group_set_volume(self, volume);
    }

    pub fn get_volume(&self) -> f32 {
        s_group_ffi::ma_sound_group_get_volume(self)
    }

    pub fn pan(&self) -> f32 {
        s_group_ffi::ma_sound_group_get_pan(self)
    }

    pub fn set_pan(&mut self, pan: f32) {
        s_group_ffi::ma_sound_group_set_pan(self, pan);
    }

    pub fn pan_mode(&self) -> Result<PanMode> {
        s_group_ffi::ma_sound_group_get_pan_mode(self)
    }

    pub fn set_pan_mode(&mut self, pan_mode: PanMode) {
        s_group_ffi::ma_sound_group_set_pan_mode(self, pan_mode);
    }

    pub fn pitch(&self) -> f32 {
        s_group_ffi::ma_sound_group_get_pitch(self)
    }

    pub fn set_pitch(&mut self, pitch: f32) {
        s_group_ffi::ma_sound_group_set_pitch(self, pitch);
    }

    pub fn spatialization(&self) -> bool {
        s_group_ffi::ma_sound_group_is_spatialization_enabled(self)
    }

    pub fn set_spatialization(&mut self, enabled: bool) {
        s_group_ffi::ma_sound_group_set_spatialization_enabled(self, enabled);
    }

    pub fn pinned_listener(&self) -> u32 {
        s_group_ffi::ma_sound_group_get_pinned_listener_index(self)
    }

    pub fn set_pinned_listener(&mut self, listener: u32) {
        s_group_ffi::ma_sound_group_set_pinned_listener_index(self, listener);
    }

    pub fn listener(&self) -> u32 {
        s_group_ffi::ma_sound_group_get_listener_index(self)
    }

    pub fn direction_to_listener(&self) -> Vec3 {
        s_group_ffi::ma_sound_group_get_direction_to_listener(self)
    }

    pub fn set_position(&mut self, vec: Vec3) {
        s_group_ffi::ma_sound_group_set_position(self, vec);
    }

    pub fn position(&self) -> Vec3 {
        s_group_ffi::ma_sound_group_get_position(self)
    }

    pub fn set_direction(&mut self, vec: Vec3) {
        s_group_ffi::ma_sound_group_set_direction(self, vec);
    }

    pub fn direction(&self) -> Vec3 {
        s_group_ffi::ma_sound_group_get_direction(self)
    }

    pub fn velocity(&self) -> Vec3 {
        s_group_ffi::ma_sound_group_get_velocity(self)
    }

    pub fn set_velocity(&mut self, vec: Vec3) {
        s_group_ffi::ma_sound_group_set_velocity(self, vec);
    }

    pub fn set_attenuation(&mut self, model: AttenuationModel) {
        s_group_ffi::ma_sound_group_set_attenuation_model(self, model);
    }

    pub fn attenuation(&self) -> Result<AttenuationModel> {
        s_group_ffi::ma_sound_group_get_attenuation_model(self)
    }

    pub fn positioning(&self) -> Result<Positioning> {
        s_group_ffi::ma_sound_group_get_positioning(self)
    }

    pub fn set_positioning(&mut self, positioning: Positioning) {
        s_group_ffi::ma_sound_group_set_positioning(self, positioning);
    }

    pub fn rolloff(&self) -> f32 {
        s_group_ffi::ma_sound_group_get_rolloff(self)
    }

    pub fn set_rolloff(&mut self, rolloff: f32) {
        s_group_ffi::ma_sound_group_set_rolloff(self, rolloff);
    }

    pub fn min_gain(&self) -> f32 {
        s_group_ffi::ma_sound_group_get_min_gain(self)
    }

    pub fn set_min_gain(&mut self, gain: f32) {
        s_group_ffi::ma_sound_group_set_min_gain(self, gain);
    }

    pub fn max_gain(&self) -> f32 {
        s_group_ffi::ma_sound_group_get_max_gain(self)
    }

    pub fn set_max_gain(&mut self, gain: f32) {
        s_group_ffi::ma_sound_group_set_max_gain(self, gain);
    }

    pub fn min_distance(&self) -> f32 {
        s_group_ffi::ma_sound_group_get_min_distance(self)
    }

    pub fn set_min_distance(&mut self, distance: f32) {
        s_group_ffi::ma_sound_group_set_min_distance(self, distance);
    }

    pub fn max_distance(&self) -> f32 {
        s_group_ffi::ma_sound_group_get_max_distance(self)
    }

    pub fn set_max_distance(&mut self, distance: f32) {
        s_group_ffi::ma_sound_group_set_max_distance(self, distance);
    }

    pub fn cone(&self) -> Cone {
        s_group_ffi::ma_sound_group_get_cone(self)
    }

    pub fn set_cone(&mut self, cone: Cone) {
        s_group_ffi::ma_sound_group_set_cone(self, cone);
    }

    pub fn doppler_factor(&self) -> f32 {
        s_group_ffi::ma_sound_group_get_doppler_factor(self)
    }

    pub fn set_doppler_factor(&mut self, factor: f32) {
        s_group_ffi::ma_sound_group_set_doppler_factor(self, factor);
    }

    pub fn directional_attenuation(&mut self) -> f32 {
        s_group_ffi::ma_sound_group_get_directional_attenuation_factor(self)
    }

    pub fn set_directional_attenuation(&mut self, factor: f32) {
        s_group_ffi::ma_sound_group_set_directional_attenuation_factor(self, factor);
    }

    pub fn set_fade_pcm(&mut self, vol_start: f32, vol_end: f32, fade_length_frames: u64) {
        s_group_ffi::ma_sound_group_set_fade_in_pcm_frames(
            self,
            vol_start,
            vol_end,
            fade_length_frames,
        );
    }

    pub fn set_fade_mili(&mut self, vol_start: f32, vol_end: f32, fade_length_mili: u64) {
        s_group_ffi::ma_sound_group_set_fade_in_milliseconds(
            self,
            vol_start,
            vol_end,
            fade_length_mili,
        );
    }

    pub fn current_fade_volume(&mut self) -> f32 {
        s_group_ffi::ma_sound_group_get_current_fade_volume(self)
    }

    pub fn set_start_time_pcm(&mut self, abs_time_frames: u64) {
        s_group_ffi::ma_sound_group_set_start_time_in_pcm_frames(self, abs_time_frames);
    }

    pub fn set_start_time_mili(&mut self, abs_time_mili: u64) {
        s_group_ffi::ma_sound_group_set_start_time_in_milliseconds(self, abs_time_mili);
    }

    pub fn set_stop_time_pcm(&mut self, abs_time_frames: u64) {
        s_group_ffi::ma_sound_group_set_stop_time_in_pcm_frames(self, abs_time_frames);
    }

    pub fn set_stop_time_mili(&mut self, abs_time_mili: u64) {
        s_group_ffi::ma_sound_group_set_stop_time_in_milliseconds(self, abs_time_mili);
    }

    pub fn playing(&self) -> bool {
        s_group_ffi::ma_sound_group_is_playing(self)
    }

    pub fn time_pcm(&mut self) -> u64 {
        s_group_ffi::ma_sound_group_get_time_in_pcm_frames(self)
    }
}

impl SoundGroup {
    /// Gets a pointer to an initialized `MaybeUninit<sys::ma_engine>`
    pub(crate) fn assume_init_ptr(&self) -> *const sys::ma_sound_group {
        debug_assert!(self.init, "Engine used before initialization.");
        self.inner.as_ptr()
    }

    /// Gets a pointer to an UNINITIALIZED `MaybeUninit<sys::ma_engine>`
    pub(crate) fn maybe_uninit_mut_ptr(&mut self) -> *mut sys::ma_sound_group {
        self.inner.as_mut_ptr()
    }

    /// Gets a pointer to an initialized `MaybeUninit<sys::ma_engine>`
    pub(crate) fn assume_init_mut_ptr(&mut self) -> *mut sys::ma_sound_group {
        debug_assert!(self.init, "Engine used before initialization.");
        unsafe { self.inner.as_mut().get_unchecked_mut().as_mut_ptr() }
    }
}

impl Drop for SoundGroup {
    fn drop(&mut self) {
        if !self.init {
            return;
        }
        s_group_ffi::ma_sound_group_uninit(self);
    }
}

pub struct SoundGroupConfig {
    inner: sys::ma_sound_group_config,
}

pub(crate) mod s_group_cfg_ffi {
    use maudio_sys::ffi as sys;

    use crate::{engine::Engine, sound::sound_group::SoundGroupConfig};

    pub fn ma_sound_group_config_init_2(engine: &mut Engine) -> SoundGroupConfig {
        let ptr = unsafe { sys::ma_sound_group_config_init_2(engine.assume_init_mut_ptr()) };
        SoundGroupConfig { inner: ptr }
    }
}

pub(crate) mod s_group_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        MaRawResult, Result,
        audio::{
            dsp::pan::PanMode,
            math::vec3::{self, Vec3},
            spatial::{attenuation::AttenuationModel, cone::Cone, positioning::Positioning},
        },
        engine::{Engine, EngineRef},
        sound::sound_group::{SoundGroup, SoundGroupConfig},
    };

    pub fn ma_sound_group_init_ex(
        engine: &mut Engine,
        config: SoundGroupConfig,
        s_group: &mut SoundGroup,
    ) -> Result<()> {
        let res = unsafe {
            sys::ma_sound_group_init_ex(
                engine.assume_init_mut_ptr(),
                &config.inner as *const _,
                s_group.maybe_uninit_mut_ptr(),
            )
        };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_sound_group_uninit(s_group: &mut SoundGroup) {
        unsafe { sys::ma_sound_group_uninit(s_group.assume_init_mut_ptr()) };
    }

    #[inline]
    pub fn ma_sound_group_get_engine<'a>(s_group: &SoundGroup) -> EngineRef<'a> {
        let ptr = unsafe { sys::ma_sound_group_get_engine(s_group.assume_init_ptr()) };
        EngineRef::from_ptr(ptr)
    }

    #[inline]
    pub fn ma_sound_group_start(s_group: &mut SoundGroup) -> Result<()> {
        let res = unsafe { sys::ma_sound_group_start(s_group.assume_init_mut_ptr()) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_sound_group_stop(s_group: &mut SoundGroup) -> Result<()> {
        let res = unsafe { sys::ma_sound_group_stop(s_group.assume_init_mut_ptr()) };
        MaRawResult::resolve(res)
    }

    #[inline]
    pub fn ma_sound_group_set_volume(s_group: &mut SoundGroup, volume: f32) {
        unsafe {
            sys::ma_sound_group_set_volume(s_group.assume_init_mut_ptr(), volume);
        }
    }

    #[inline]
    pub fn ma_sound_group_get_volume(s_group: &SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_volume(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_pan(s_group: &mut SoundGroup, pan: f32) {
        unsafe { sys::ma_sound_group_set_pan(s_group.assume_init_mut_ptr(), pan) };
    }

    #[inline]
    pub fn ma_sound_group_get_pan(s_group: &SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_pan(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_pan_mode(s_group: &mut SoundGroup, pan_mode: PanMode) {
        unsafe { sys::ma_sound_group_set_pan_mode(s_group.assume_init_mut_ptr(), pan_mode.into()) }
    }

    #[inline]
    pub fn ma_sound_group_get_pan_mode(s_group: &SoundGroup) -> Result<PanMode> {
        let mode = unsafe { sys::ma_sound_group_get_pan_mode(s_group.assume_init_ptr()) };
        mode.try_into()
    }

    #[inline]
    pub fn ma_sound_group_set_pitch(s_group: &mut SoundGroup, pitch: f32) {
        unsafe { sys::ma_sound_group_set_pitch(s_group.assume_init_mut_ptr(), pitch) }
    }

    #[inline]
    pub fn ma_sound_group_get_pitch(s_group: &SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_pitch(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_spatialization_enabled(s_group: &mut SoundGroup, enabled: bool) {
        let enabled = enabled as sys::ma_bool32;
        unsafe {
            sys::ma_sound_group_set_spatialization_enabled(s_group.assume_init_mut_ptr(), enabled)
        }
    }

    #[inline]
    pub fn ma_sound_group_is_spatialization_enabled(s_group: &SoundGroup) -> bool {
        let res =
            unsafe { sys::ma_sound_group_is_spatialization_enabled(s_group.assume_init_ptr()) };
        res == 1
    }

    #[inline]
    pub fn ma_sound_group_set_pinned_listener_index(s_group: &mut SoundGroup, listener_idx: u32) {
        println!("Setting listener to {listener_idx}");
        unsafe {
            sys::ma_sound_group_set_pinned_listener_index(
                s_group.assume_init_mut_ptr(),
                listener_idx,
            )
        }
    }

    #[inline]
    pub fn ma_sound_group_get_pinned_listener_index(s_group: &SoundGroup) -> u32 {
        unsafe { sys::ma_sound_group_get_pinned_listener_index(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_get_listener_index(s_group: &SoundGroup) -> u32 {
        unsafe { sys::ma_sound_group_get_listener_index(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_get_direction_to_listener(s_group: &SoundGroup) -> Vec3 {
        let vec =
            unsafe { sys::ma_sound_group_get_direction_to_listener(s_group.assume_init_ptr()) };
        vec.into()
    }

    #[inline]
    pub fn ma_sound_group_set_position(s_group: &mut SoundGroup, vec3: Vec3) {
        unsafe {
            sys::ma_sound_group_set_position(s_group.assume_init_mut_ptr(), vec3.x, vec3.y, vec3.z);
        }
    }

    #[inline]
    pub fn ma_sound_group_get_position(s_group: &SoundGroup) -> Vec3 {
        let vec = unsafe { sys::ma_sound_group_get_position(s_group.assume_init_ptr()) };
        vec.into()
    }

    #[inline]
    pub fn ma_sound_group_set_direction(s_group: &mut SoundGroup, vec3: Vec3) {
        unsafe {
            sys::ma_sound_group_set_direction(s_group.assume_init_mut_ptr(), vec3.x, vec3.y, vec3.z)
        }
    }

    #[inline]
    pub fn ma_sound_group_get_direction(s_group: &SoundGroup) -> Vec3 {
        let vec = unsafe { sys::ma_sound_group_get_direction(s_group.assume_init_ptr()) };
        vec.into()
    }

    #[inline]
    pub fn ma_sound_group_set_velocity(s_group: &mut SoundGroup, vec3: Vec3) {
        unsafe {
            sys::ma_sound_group_set_velocity(s_group.assume_init_mut_ptr(), vec3.x, vec3.y, vec3.z)
        }
    }

    #[inline]
    pub fn ma_sound_group_get_velocity(s_group: &SoundGroup) -> Vec3 {
        let vec = unsafe { sys::ma_sound_group_get_velocity(s_group.assume_init_ptr()) };
        vec.into()
    }

    #[inline]
    pub fn ma_sound_group_set_attenuation_model(s_group: &mut SoundGroup, model: AttenuationModel) {
        unsafe {
            sys::ma_sound_group_set_attenuation_model(s_group.assume_init_mut_ptr(), model.into())
        }
    }

    #[inline]
    pub fn ma_sound_group_get_attenuation_model(s_group: &SoundGroup) -> Result<AttenuationModel> {
        let model = unsafe { sys::ma_sound_group_get_attenuation_model(s_group.assume_init_ptr()) };
        model.try_into()
    }

    #[inline]
    pub fn ma_sound_group_set_positioning(s_group: &mut SoundGroup, positioning: Positioning) {
        unsafe {
            sys::ma_sound_group_set_positioning(s_group.assume_init_mut_ptr(), positioning.into())
        }
    }

    #[inline]
    pub fn ma_sound_group_get_positioning(s_group: &SoundGroup) -> Result<Positioning> {
        let pos = unsafe { sys::ma_sound_group_get_positioning(s_group.assume_init_ptr()) };
        pos.try_into()
    }

    #[inline]
    pub fn ma_sound_group_set_rolloff(s_group: &mut SoundGroup, rolloff: f32) {
        unsafe { sys::ma_sound_group_set_rolloff(s_group.assume_init_mut_ptr(), rolloff) }
    }

    #[inline]
    pub fn ma_sound_group_get_rolloff(s_group: &SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_rolloff(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_min_gain(s_group: &mut SoundGroup, min_gain: f32) {
        unsafe { sys::ma_sound_group_set_min_gain(s_group.assume_init_mut_ptr(), min_gain) }
    }

    #[inline]
    pub fn ma_sound_group_get_min_gain(s_group: &SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_min_gain(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_max_gain(s_group: &mut SoundGroup, max_gain: f32) {
        unsafe { sys::ma_sound_group_set_max_gain(s_group.assume_init_mut_ptr(), max_gain) }
    }

    #[inline]
    pub fn ma_sound_group_get_max_gain(s_group: &SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_max_gain(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_min_distance(s_group: &mut SoundGroup, min_distance: f32) {
        unsafe { sys::ma_sound_group_set_min_distance(s_group.assume_init_mut_ptr(), min_distance) }
    }

    #[inline]
    pub fn ma_sound_group_get_min_distance(s_group: &SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_min_distance(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_max_distance(s_group: &mut SoundGroup, max_distance: f32) {
        unsafe { sys::ma_sound_group_set_max_distance(s_group.assume_init_mut_ptr(), max_distance) }
    }

    #[inline]
    pub fn ma_sound_group_get_max_distance(s_group: &SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_max_distance(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_cone(
        s_group: &mut SoundGroup,
        cone: Cone
    ) {
        unsafe {
            sys::ma_sound_group_set_cone(
                s_group.assume_init_mut_ptr(),
                cone.inner_angle_rad,
                cone.outer_angle_rad,
                cone.outer_gain,
            );
        }
    }

    #[inline]
    pub fn ma_sound_group_get_cone(s_group: &SoundGroup) -> Cone {
        let mut inner = 0.0f32;
        let mut outer = 0.0f32;
        let mut gain = 1.0f32;

        unsafe {
            sys::ma_sound_group_get_cone(
                s_group.assume_init_ptr(),
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
    pub fn ma_sound_group_set_doppler_factor(s_group: &mut SoundGroup, doppler_factor: f32) {
        unsafe {
            sys::ma_sound_group_set_doppler_factor(s_group.assume_init_mut_ptr(), doppler_factor)
        }
    }

    #[inline]
    pub fn ma_sound_group_get_doppler_factor(s_group: &SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_doppler_factor(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_directional_attenuation_factor(
        s_group: &mut SoundGroup,
        dir_attenuation_factor: f32,
    ) {
        unsafe {
            sys::ma_sound_group_set_directional_attenuation_factor(
                s_group.assume_init_mut_ptr(),
                dir_attenuation_factor,
            );
        }
    }

    #[inline]
    pub fn ma_sound_group_get_directional_attenuation_factor(s_group: &SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_directional_attenuation_factor(s_group.assume_init_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_fade_in_pcm_frames(
        s_group: &mut SoundGroup,
        volume_start: f32,
        volume_end: f32,
        fade_length_frames: u64,
    ) {
        unsafe {
            sys::ma_sound_group_set_fade_in_pcm_frames(
                s_group.assume_init_mut_ptr(),
                volume_start,
                volume_end,
                fade_length_frames,
            )
        };
    }

    #[inline]
    pub fn ma_sound_group_set_fade_in_milliseconds(
        s_group: &mut SoundGroup,
        volume_start: f32,
        volume_end: f32,
        fade_length_mili: u64,
    ) {
        unsafe {
            sys::ma_sound_group_set_fade_in_milliseconds(
                s_group.assume_init_mut_ptr(),
                volume_start,
                volume_end,
                fade_length_mili,
            );
        }
    }

    #[inline]
    pub fn ma_sound_group_get_current_fade_volume(s_group: &mut SoundGroup) -> f32 {
        unsafe { sys::ma_sound_group_get_current_fade_volume(s_group.assume_init_mut_ptr()) }
    }

    #[inline]
    pub fn ma_sound_group_set_start_time_in_pcm_frames(
        s_group: &mut SoundGroup,
        abs_time_frames: u64,
    ) {
        unsafe {
            sys::ma_sound_group_set_start_time_in_pcm_frames(
                s_group.assume_init_mut_ptr(),
                abs_time_frames,
            );
        }
    }

    #[inline]
    pub fn ma_sound_group_set_start_time_in_milliseconds(
        s_group: &mut SoundGroup,
        abs_time_mili: u64,
    ) {
        unsafe {
            sys::ma_sound_group_set_start_time_in_milliseconds(
                s_group.assume_init_mut_ptr(),
                abs_time_mili,
            );
        }
    }

    #[inline]
    pub fn ma_sound_group_set_stop_time_in_pcm_frames(
        s_group: &mut SoundGroup,
        abs_time_frames: u64,
    ) {
        unsafe {
            sys::ma_sound_group_set_stop_time_in_pcm_frames(
                s_group.assume_init_mut_ptr(),
                abs_time_frames,
            );
        }
    }

    #[inline]
    pub fn ma_sound_group_set_stop_time_in_milliseconds(
        s_group: &mut SoundGroup,
        abs_time_mili: u64,
    ) {
        unsafe {
            sys::ma_sound_group_set_stop_time_in_milliseconds(
                s_group.assume_init_mut_ptr(),
                abs_time_mili,
            );
        }
    }

    #[inline]
    pub fn ma_sound_group_is_playing(s_group: &SoundGroup) -> bool {
        let res = unsafe { sys::ma_sound_group_is_playing(s_group.assume_init_ptr()) };
        res == 1
    }

    #[inline]
    pub fn ma_sound_group_get_time_in_pcm_frames(s_group: &SoundGroup) -> u64 {
        unsafe { sys::ma_sound_group_get_time_in_pcm_frames(s_group.assume_init_ptr()) }
    }
}

#[cfg(test)]
mod test {
    use crate::{audio::{dsp::pan::PanMode, math::vec3::Vec3, spatial::{attenuation::AttenuationModel, cone::Cone, positioning::Positioning}}, engine::Engine};

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    fn assert_approx_eq(a: f32, b: f32, eps: f32) {
        assert!(
            approx_eq(a, b, eps),
            "expected ~={b}, got {a} (eps={eps})"
        );
    }

    fn assert_vec3_eq(a: Vec3, b: Vec3, eps: f32) {
        // Adjust field names if your Vec3 differs.
        assert_approx_eq(a.x, b.x, eps);
        assert_approx_eq(a.y, b.y, eps);
        assert_approx_eq(a.z, b.z, eps);
    }

    #[test]
    // #[cfg(feature = "device-tests")]
    fn test_sound_group_basic() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();
        let engine_ref = s_group.engine();
        drop(engine_ref);
    }

    #[test]
    fn test_sound_group_start_stop_smoke() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        // These are just smoke tests; depending on backend/device, playing() may be false.
        s_group.start().unwrap();
        s_group.stop().unwrap();
    }

    #[test]
    fn test_sound_group_volume_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_volume(0.25);
        let v = s_group.get_volume();
        assert_approx_eq(v, 0.25, 1e-6);

        s_group.set_volume(1.0);
        let v = s_group.get_volume();
        assert_approx_eq(v, 1.0, 1e-6);
    }

    #[test]
    fn test_sound_group_pan_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_pan(-0.5);
        assert_approx_eq(s_group.pan(), -0.5, 1e-6);

        s_group.set_pan(0.75);
        assert_approx_eq(s_group.pan(), 0.75, 1e-6);
    }

    #[test]
    fn test_sound_group_pan_mode_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        // Use variants that exist in your PanMode enum.
        // Common miniaudio ones: PanMode::Balance, PanMode::Pan
        s_group.set_pan_mode(PanMode::Balance);
        assert_eq!(s_group.pan_mode().unwrap(), PanMode::Balance);

        s_group.set_pan_mode(PanMode::Pan);
        assert_eq!(s_group.pan_mode().unwrap(), PanMode::Pan);
    }

    #[test]
    fn test_sound_group_pitch_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_pitch(0.5);
        assert_approx_eq(s_group.pitch(), 0.5, 1e-6);

        s_group.set_pitch(1.25);
        assert_approx_eq(s_group.pitch(), 1.25, 1e-6);
    }

    #[test]
    fn test_sound_group_spatialization_toggle() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_spatialization(false);
        assert_eq!(s_group.spatialization(), false);

        s_group.set_spatialization(true);
        assert_eq!(s_group.spatialization(), true);
    }

    // #[test]
    // fn test_sound_group_pinned_listener_roundtrip() {
    //     let mut engine = Engine::new().unwrap();
    //     let mut s_group = engine.new_sound_group().unwrap();

    //     s_group.set_pinned_listener(0);
    //     assert_eq!(s_group.pinned_listener(), 0);

    //     s_group.set_pinned_listener(2);
    //     assert_eq!(s_group.pinned_listener(), 2);
    // }

    // #[test]
    // fn test_sound_group_pinned_listener_roundtrip() {
    //     let mut engine = Engine::new().unwrap();
    //     let mut s_group = engine.new_sound_group().unwrap();

    //     s_group.set_pinned_listener(0);
    //     assert_eq!(s_group.pinned_listener(), 0);

    //     let listener_count = engine.listener_count();
    //     if listener_count > 1 {
    //         s_group.set_pinned_listener(1);
    //         assert_eq!(s_group.pinned_listener(), 1);
    //     } else {
    //         // With only one listener, setting to 1 is invalid => should remain 0.
    //         s_group.set_pinned_listener(1);
    //         assert_eq!(s_group.pinned_listener(), 0);
    //     }
    // }

    #[test]
    fn test_sound_group_listener_index_smoke() {
        let mut engine = Engine::new().unwrap();
        let s_group = engine.new_sound_group().unwrap();

        // Just ensure call works and returns something deterministic-ish.
        let _idx = s_group.listener();
    }

    #[test]
    fn test_sound_group_position_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        let p = Vec3 { x: 1.0, y: 2.0, z: 3.0 };
        s_group.set_position(p);
        let got = s_group.position();
        assert_vec3_eq(got, p, 1e-6);
    }

    #[test]
    fn test_sound_group_direction_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        let d = Vec3 { x: 0.0, y: 0.0, z: 1.0 };
        s_group.set_direction(d);
        let got = s_group.direction();
        assert_vec3_eq(got, d, 1e-6);
    }

    #[test]
    fn test_sound_group_velocity_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        let v = Vec3 { x: -1.0, y: 0.5, z: 0.25 };
        s_group.set_velocity(v);
        let got = s_group.velocity();
        assert_vec3_eq(got, v, 1e-6);
    }

    #[test]
    fn test_sound_group_direction_to_listener_smoke() {
        let mut engine = Engine::new().unwrap();
        let s_group = engine.new_sound_group().unwrap();

        // Typically depends on listener position; just ensure it doesn't crash.
        let _dir = s_group.direction_to_listener();
    }

    #[test]
    fn test_sound_group_attenuation_model_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        // Use variants that exist in your AttenuationModel enum.
        s_group.set_attenuation(AttenuationModel::None);
        assert_eq!(s_group.attenuation().unwrap(), AttenuationModel::None);

        s_group.set_attenuation(AttenuationModel::Inverse);
        assert_eq!(s_group.attenuation().unwrap(), AttenuationModel::Inverse);
    }

    #[test]
    fn test_sound_group_positioning_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_positioning(Positioning::Absolute);
        assert_eq!(s_group.positioning().unwrap(), Positioning::Absolute);

        s_group.set_positioning(Positioning::Relative);
        assert_eq!(s_group.positioning().unwrap(), Positioning::Relative);
    }

    #[test]
    fn test_sound_group_rolloff_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_rolloff(0.75);
        assert_approx_eq(s_group.rolloff(), 0.75, 1e-6);
    }

    #[test]
    fn test_sound_group_min_max_gain_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_min_gain(0.1);
        assert_approx_eq(s_group.min_gain(), 0.1, 1e-6);

        s_group.set_max_gain(2.0);
        assert_approx_eq(s_group.max_gain(), 2.0, 1e-6);
    }

    #[test]
    fn test_sound_group_min_max_distance_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_min_distance(1.25);
        assert_approx_eq(s_group.min_distance(), 1.25, 1e-6);

        s_group.set_max_distance(50.0);
        assert_approx_eq(s_group.max_distance(), 50.0, 1e-6);
    }

    #[test]
    fn test_sound_group_cone_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        let c = Cone {
            inner_angle_rad: 0.5,
            outer_angle_rad: 1.0,
            outer_gain: 0.25,
        };

        s_group.set_cone(c);
        let got = s_group.cone();

        assert_approx_eq(got.inner_angle_rad, c.inner_angle_rad, 1e-6);
        assert_approx_eq(got.outer_angle_rad, c.outer_angle_rad, 1e-6);
        assert_approx_eq(got.outer_gain, c.outer_gain, 1e-6);
    }

    #[test]
    fn test_sound_group_doppler_factor_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_doppler_factor(1.5);
        assert_approx_eq(s_group.doppler_factor(), 1.5, 1e-6);
    }

    #[test]
    fn test_sound_group_directional_attenuation_roundtrip() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_directional_attenuation(0.6);
        assert_approx_eq(s_group.directional_attenuation(), 0.6, 1e-6);
    }


    #[test]
    fn test_sound_group_fade_api_smoke() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        // Not possible to reliably assert current_fade_volume() without running audio;
        // this just ensures the calls are wired correctly.
        s_group.set_fade_pcm(0.0, 1.0, 4800);
        let _v = s_group.current_fade_volume();

        s_group.set_fade_mili(1.0, 0.0, 250);
        let _v2 = s_group.current_fade_volume();
    }

    #[test]
    fn test_sound_group_start_stop_time_api_smoke() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        s_group.set_start_time_pcm(0);
        s_group.set_stop_time_pcm(0);

        s_group.set_start_time_mili(0);
        s_group.set_stop_time_mili(0);
    }

    #[test]
    fn test_sound_group_playing_and_time_pcm_smoke() {
        let mut engine = Engine::new().unwrap();
        let mut s_group = engine.new_sound_group().unwrap();

        let _is_playing = s_group.playing();
        let _t = s_group.time_pcm();
    }
}
