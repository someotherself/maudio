use std::{marker::PhantomData, mem::MaybeUninit};

use crate::{
    audio::{
        channels::Channel,
        math::vec3::Vec3,
        spatial::{attenuation::AttenuationModel, cone::Cone, positioning::Positioning},
    },
    pcm_frames::PcmFormat,
    Binding, MaResult,
};

use maudio_sys::ffi as sys;

pub struct Spatializer<F: PcmFormat> {
    inner: *mut sys::ma_spatializer,
    channels_in: u32,
    channels_out: u32,
    _format: PhantomData<F>,
}

impl<F: PcmFormat> Binding for Spatializer<F> {
    type Raw = *mut sys::ma_spatializer;

    /// !!! unimplemented !!!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Spatializer<F> {
    fn build(config: &sys::ma_spatializer_config) -> MaResult<Spatializer<F>> {
        let mut inner: MaybeUninit<sys::ma_spatializer> = MaybeUninit::uninit();
        spatializer_ffi::ma_spatializer_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));
        Ok(Spatializer {
            inner: inner_ptr,
            channels_in: config.channelsIn,
            channels_out: config.channelsOut,
            _format: PhantomData,
        })
    }

    pub fn process_pcm_frames(
        &mut self,
        listener: &mut Listener<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        spatializer_ffi::ma_spatializer_process_pcm_frames(self, listener, frames_out, frames_in)
    }

    pub fn set_master_volume(&mut self, volume: f32) -> MaResult<()> {
        spatializer_ffi::ma_spatializer_set_master_volume(self, volume)
    }

    pub fn master_volume(&self) -> MaResult<f32> {
        spatializer_ffi::ma_spatializer_get_master_volume(self)
    }

    pub fn input_channels(&self) -> u32 {
        spatializer_ffi::ma_spatializer_get_input_channels(self)
    }

    pub fn output_channels(&self) -> u32 {
        spatializer_ffi::ma_spatializer_get_output_channels(self)
    }

    pub fn attenuation_model(&self) -> MaResult<AttenuationModel> {
        spatializer_ffi::ma_spatializer_get_attenuation_model(self)
    }

    pub fn set_positioning(&mut self, positioning: Positioning) {
        spatializer_ffi::ma_spatializer_set_positioning(self, positioning);
    }

    pub fn positioning(&self) -> MaResult<Positioning> {
        spatializer_ffi::ma_spatializer_get_positioning(self)
    }

    pub fn set_rolloff(&mut self, rolloff: f32) {
        spatializer_ffi::ma_spatializer_set_rolloff(self, rolloff);
    }

    pub fn rolloff(&self) -> f32 {
        spatializer_ffi::ma_spatializer_get_rolloff(self)
    }

    pub fn set_min_gain(&mut self, min_gain: f32) {
        spatializer_ffi::ma_spatializer_set_min_gain(self, min_gain);
    }

    pub fn min_gain(&self) -> f32 {
        spatializer_ffi::ma_spatializer_get_min_gain(self)
    }

    pub fn set_max_gain(&mut self, max_gain: f32) {
        spatializer_ffi::ma_spatializer_set_max_gain(self, max_gain);
    }

    pub fn max_gain(&self) -> f32 {
        spatializer_ffi::ma_spatializer_get_max_gain(self)
    }

    pub fn set_min_distance(&mut self, min_distance: f32) {
        spatializer_ffi::ma_spatializer_set_min_distance(self, min_distance);
    }

    pub fn min_distance(&self) -> f32 {
        spatializer_ffi::ma_spatializer_get_min_distance(self)
    }

    pub fn set_max_distance(&mut self, max_distance: f32) {
        spatializer_ffi::ma_spatializer_set_max_distance(self, max_distance);
    }

    pub fn max_distance(&self) -> f32 {
        spatializer_ffi::ma_spatializer_get_max_distance(self)
    }

    pub fn set_cone(&mut self, cone: Cone) {
        spatializer_ffi::ma_spatializer_set_cone(self, cone);
    }

    pub fn cone(&self) -> Cone {
        spatializer_ffi::ma_spatializer_get_cone(self)
    }

    pub fn set_doppler_factor(&mut self, doppler_factor: f32) {
        spatializer_ffi::ma_spatializer_set_doppler_factor(self, doppler_factor);
    }

    pub fn doppler_factor(&self) -> f32 {
        spatializer_ffi::ma_spatializer_get_doppler_factor(self)
    }

    pub fn set_directional_attenuation_factor(&mut self, factor: f32) {
        spatializer_ffi::ma_spatializer_set_directional_attenuation_factor(self, factor);
    }

    pub fn directional_attenuation_factor(&self) -> f32 {
        spatializer_ffi::ma_spatializer_get_directional_attenuation_factor(self)
    }

    pub fn set_position(&mut self, position: Vec3) {
        spatializer_ffi::ma_spatializer_set_position(self, position);
    }

    pub fn position(&self) -> Vec3 {
        spatializer_ffi::ma_spatializer_get_position(self)
    }

    pub fn set_direction(&mut self, direction: Vec3) {
        spatializer_ffi::ma_spatializer_set_direction(self, direction);
    }

    pub fn direction(&self) -> Vec3 {
        spatializer_ffi::ma_spatializer_get_direction(self)
    }

    pub fn set_velocity(&mut self, velocity: Vec3) {
        spatializer_ffi::ma_spatializer_set_velocity(self, velocity);
    }

    pub fn velocity(&self) -> Vec3 {
        spatializer_ffi::ma_spatializer_get_velocity(self)
    }

    pub fn relative_position_and_direction(&self, listener: &mut Listener<F>) -> (Vec3, Vec3) {
        spatializer_ffi::ma_spatializer_get_relative_position_and_direction(self, listener)
    }
}

struct SpatializerBuilder {
    config: sys::ma_spatializer_config,
}

impl SpatializerBuilder {
    pub fn new(channels_in: u32, channels_out: u32) -> Self {
        let config = unsafe { sys::ma_spatializer_config_init(channels_in, channels_out) };
        Self { config }
    }

    pub fn build_f32(&self) -> MaResult<Spatializer<f32>> {
        Spatializer::<f32>::build(&self.config)
    }
}

mod spatializer_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::{
            dsp::spatializer::{Listener, Spatializer},
            math::vec3::Vec3,
            spatial::{attenuation::AttenuationModel, cone::Cone, positioning::Positioning},
        },
        engine::AllocationCallbacks,
        pcm_frames::PcmFormat,
        AsRawRef, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_spatializer_init(
        config: &sys::ma_spatializer_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        spatializer: *mut sys::ma_spatializer,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res = unsafe { sys::ma_spatializer_init(config as *const _, alloc_cb, spatializer) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_spatializer_uninit<F: PcmFormat>(spatializer: &mut Spatializer<F>) {
        unsafe {
            sys::ma_spatializer_uninit(spatializer.to_raw(), std::ptr::null_mut());
        };
    }

    #[inline]
    pub fn ma_spatializer_process_pcm_frames<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        listener: &mut Listener<F>,
        frames_out: &mut [F::StorageUnit],
        frames_in: &[F::StorageUnit],
    ) -> MaResult<()> {
        let channels_in = spatializer.channels_in as usize;
        let channels_out = listener.channels_out as usize;

        let frame_in = frames_in.len() / channels_in;
        let frame_out = frames_out.len() / channels_out;
        let frames_proc = frame_in.min(frame_out);
        let res = unsafe {
            sys::ma_spatializer_process_pcm_frames(
                spatializer.to_raw(),
                listener.to_raw(),
                frames_out.as_mut_ptr() as *mut _,
                frames_in.as_ptr() as *const _,
                frames_proc as u64,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_spatializer_set_master_volume<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        volume: f32,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_spatializer_set_master_volume(spatializer.to_raw(), volume) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_spatializer_get_master_volume<F: PcmFormat>(
        spatializer: &Spatializer<F>,
    ) -> MaResult<f32> {
        let mut volume = 0.0;
        let res =
            unsafe { sys::ma_spatializer_get_master_volume(spatializer.to_raw(), &mut volume) };
        MaudioError::check(res)?;
        Ok(volume)
    }

    #[inline]
    pub fn ma_spatializer_get_input_channels<F: PcmFormat>(spatializer: &Spatializer<F>) -> u32 {
        unsafe { sys::ma_spatializer_get_input_channels(spatializer.to_raw()) }
    }

    #[inline]
    pub fn ma_spatializer_get_output_channels<F: PcmFormat>(spatializer: &Spatializer<F>) -> u32 {
        unsafe { sys::ma_spatializer_get_output_channels(spatializer.to_raw()) }
    }

    #[inline]
    pub fn ma_spatializer_set_attenuation_model<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        model: AttenuationModel,
    ) {
        unsafe {
            sys::ma_spatializer_set_attenuation_model(spatializer.to_raw(), model.into());
        };
    }

    #[inline]
    pub fn ma_spatializer_get_attenuation_model<F: PcmFormat>(
        spatializer: &Spatializer<F>,
    ) -> MaResult<AttenuationModel> {
        let res = unsafe { sys::ma_spatializer_get_attenuation_model(spatializer.to_raw()) };
        res.try_into()
    }

    #[inline]
    pub fn ma_spatializer_set_positioning<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        positioning: Positioning,
    ) {
        unsafe {
            sys::ma_spatializer_set_positioning(spatializer.to_raw(), positioning.into());
        }
    }

    #[inline]
    pub fn ma_spatializer_get_positioning<F: PcmFormat>(
        spatializer: &Spatializer<F>,
    ) -> MaResult<Positioning> {
        let res = unsafe { sys::ma_spatializer_get_positioning(spatializer.to_raw()) };
        res.try_into()
    }

    #[inline]
    pub fn ma_spatializer_set_rolloff<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        rolloff: f32,
    ) {
        unsafe { sys::ma_spatializer_set_rolloff(spatializer.to_raw(), rolloff) };
    }

    #[inline]
    pub fn ma_spatializer_get_rolloff<F: PcmFormat>(spatializer: &Spatializer<F>) -> f32 {
        unsafe { sys::ma_spatializer_get_rolloff(spatializer.to_raw()) }
    }

    #[inline]
    pub fn ma_spatializer_set_min_gain<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        min_gain: f32,
    ) {
        unsafe { sys::ma_spatializer_set_min_gain(spatializer.to_raw(), min_gain) };
    }

    #[inline]
    pub fn ma_spatializer_get_min_gain<F: PcmFormat>(spatializer: &Spatializer<F>) -> f32 {
        unsafe { sys::ma_spatializer_get_min_gain(spatializer.to_raw()) }
    }

    #[inline]
    pub fn ma_spatializer_set_max_gain<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        max_gain: f32,
    ) {
        unsafe { sys::ma_spatializer_set_max_gain(spatializer.to_raw(), max_gain) };
    }

    #[inline]
    pub fn ma_spatializer_get_max_gain<F: PcmFormat>(spatializer: &Spatializer<F>) -> f32 {
        unsafe { sys::ma_spatializer_get_max_gain(spatializer.to_raw()) }
    }

    #[inline]
    pub fn ma_spatializer_set_min_distance<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        min_distance: f32,
    ) {
        unsafe {
            sys::ma_spatializer_set_min_distance(spatializer.to_raw(), min_distance);
        }
    }

    #[inline]
    pub fn ma_spatializer_get_min_distance<F: PcmFormat>(spatializer: &Spatializer<F>) -> f32 {
        unsafe { sys::ma_spatializer_get_min_distance(spatializer.to_raw()) }
    }

    #[inline]
    pub fn ma_spatializer_set_max_distance<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        max_distance: f32,
    ) {
        unsafe {
            sys::ma_spatializer_set_max_distance(spatializer.to_raw(), max_distance);
        }
    }

    #[inline]
    pub fn ma_spatializer_get_max_distance<F: PcmFormat>(spatializer: &Spatializer<F>) -> f32 {
        unsafe { sys::ma_spatializer_get_max_distance(spatializer.to_raw()) }
    }

    #[inline]
    pub fn ma_spatializer_get_cone<F: PcmFormat>(spatializer: &Spatializer<F>) -> Cone {
        let mut inner = 0.0f32;
        let mut outer = 0.0f32;
        let mut gain = 1.0f32;
        unsafe {
            sys::ma_spatializer_get_cone(spatializer.to_raw(), &mut inner, &mut outer, &mut gain);
        };
        Cone {
            inner_angle_rad: inner,
            outer_angle_rad: outer,
            outer_gain: gain,
        }
    }

    #[inline]
    pub fn ma_spatializer_set_cone<F: PcmFormat>(spatializer: &mut Spatializer<F>, cone: Cone) {
        unsafe {
            sys::ma_spatializer_set_cone(
                spatializer.to_raw(),
                cone.inner_angle_rad,
                cone.outer_angle_rad,
                cone.outer_gain,
            );
        }
    }

    #[inline]
    pub fn ma_spatializer_set_doppler_factor<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        doppler: f32,
    ) {
        unsafe {
            sys::ma_spatializer_set_doppler_factor(spatializer.to_raw(), doppler);
        }
    }

    #[inline]
    pub fn ma_spatializer_get_doppler_factor<F: PcmFormat>(spatializer: &Spatializer<F>) -> f32 {
        unsafe { sys::ma_spatializer_get_doppler_factor(spatializer.to_raw()) }
    }

    #[inline]
    pub fn ma_spatializer_set_directional_attenuation_factor<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        factor: f32,
    ) {
        unsafe {
            sys::ma_spatializer_set_directional_attenuation_factor(spatializer.to_raw(), factor);
        }
    }

    #[inline]
    pub fn ma_spatializer_get_directional_attenuation_factor<F: PcmFormat>(
        spatializer: &Spatializer<F>,
    ) -> f32 {
        unsafe { sys::ma_spatializer_get_directional_attenuation_factor(spatializer.to_raw()) }
    }

    #[inline]
    pub fn ma_spatializer_get_position<F: PcmFormat>(spatializer: &Spatializer<F>) -> Vec3 {
        let res = unsafe { sys::ma_spatializer_get_position(spatializer.to_raw()) };
        res.into()
    }

    #[inline]
    pub fn ma_spatializer_set_position<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        position: Vec3,
    ) {
        unsafe {
            sys::ma_spatializer_set_position(
                spatializer.to_raw(),
                position.x,
                position.y,
                position.z,
            );
        }
    }

    #[inline]
    pub fn ma_spatializer_get_direction<F: PcmFormat>(spatializer: &Spatializer<F>) -> Vec3 {
        let res = unsafe { sys::ma_spatializer_get_direction(spatializer.to_raw()) };
        res.into()
    }

    #[inline]
    pub fn ma_spatializer_set_direction<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        direction: Vec3,
    ) {
        unsafe {
            sys::ma_spatializer_set_direction(
                spatializer.to_raw(),
                direction.x,
                direction.y,
                direction.z,
            )
        };
    }

    #[inline]
    pub fn ma_spatializer_get_velocity<F: PcmFormat>(spatializer: &Spatializer<F>) -> Vec3 {
        let res = unsafe { sys::ma_spatializer_get_velocity(spatializer.to_raw()) };
        res.into()
    }

    #[inline]
    pub fn ma_spatializer_set_velocity<F: PcmFormat>(
        spatializer: &mut Spatializer<F>,
        velocity: Vec3,
    ) {
        unsafe {
            sys::ma_spatializer_set_velocity(
                spatializer.to_raw(),
                velocity.x,
                velocity.y,
                velocity.z,
            )
        };
    }

    #[inline]
    pub fn ma_spatializer_get_relative_position_and_direction<F: PcmFormat>(
        spatializer: &Spatializer<F>,
        listener: &mut Listener<F>,
    ) -> (Vec3, Vec3) {
        let mut relative_pos = sys::ma_vec3f {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let mut relative_dir = sys::ma_vec3f {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        unsafe {
            sys::ma_spatializer_get_relative_position_and_direction(
                spatializer.to_raw(),
                listener.to_raw(),
                &mut relative_pos,
                &mut relative_dir,
            )
        };
        (relative_pos.into(), relative_dir.into())
    }
}

impl<F: PcmFormat> Drop for Spatializer<F> {
    fn drop(&mut self) {
        spatializer_ffi::ma_spatializer_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

pub struct Listener<F: PcmFormat> {
    inner: *mut sys::ma_spatializer_listener,
    channels_out: u32,
    _format: PhantomData<F>,
}

impl<F: PcmFormat> Binding for Listener<F> {
    type Raw = *mut sys::ma_spatializer_listener;

    /// !!! unimplemented !!!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> Listener<F> {
    fn build(config: &sys::ma_spatializer_listener_config) -> MaResult<Listener<F>> {
        let channels_out = config.channelsOut;
        let mut inner: MaybeUninit<sys::ma_spatializer_listener> = MaybeUninit::uninit();
        sp_listener_ffi::ma_spatializer_listener_init(config, None, inner.as_mut_ptr())?;

        let inner_ptr = Box::into_raw(Box::new(unsafe { inner.assume_init() }));

        Ok(Listener {
            inner: inner_ptr,
            channels_out,
            _format: PhantomData,
        })
    }

    pub fn channel_map(&self) -> Vec<Channel> {
        sp_listener_ffi::ma_spatializer_listener_get_channel_map(self)
    }

    pub fn set_cone(&mut self, cone: Cone) {
        sp_listener_ffi::ma_spatializer_listener_set_cone(self, cone);
    }

    pub fn cone(&self) -> Cone {
        sp_listener_ffi::ma_spatializer_listener_get_cone(self)
    }

    pub fn set_position(&mut self, position: Vec3) {
        sp_listener_ffi::ma_spatializer_listener_set_position(self, position);
    }

    pub fn position(&self) -> Vec3 {
        sp_listener_ffi::ma_spatializer_listener_get_position(self)
    }

    pub fn set_direction(&mut self, direction: Vec3) {
        sp_listener_ffi::ma_spatializer_listener_set_direction(self, direction);
    }

    pub fn direction(&self) -> Vec3 {
        sp_listener_ffi::ma_spatializer_listener_get_direction(self)
    }

    pub fn set_velocity(&mut self, velocity: Vec3) {
        sp_listener_ffi::ma_spatializer_listener_set_velocity(self, velocity);
    }

    pub fn velocity(&self) -> Vec3 {
        sp_listener_ffi::ma_spatializer_listener_get_velocity(self)
    }

    pub fn set_speed_of_sound(&mut self, speed: f32) {
        sp_listener_ffi::ma_spatializer_listener_set_speed_of_sound(self, speed);
    }

    pub fn speed_of_sound(&self) -> f32 {
        sp_listener_ffi::ma_spatializer_listener_get_speed_of_sound(self)
    }

    pub fn set_world_up(&mut self, up: Vec3) {
        sp_listener_ffi::ma_spatializer_listener_set_world_up(self, up);
    }

    pub fn world_up(&self) -> Vec3 {
        sp_listener_ffi::ma_spatializer_listener_get_world_up(self)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        sp_listener_ffi::ma_spatializer_listener_set_enabled(self, enabled);
    }

    pub fn is_enabled(&self) -> bool {
        sp_listener_ffi::ma_spatializer_listener_is_enabled(self)
    }
}

pub struct ListenerBuilder {
    config: sys::ma_spatializer_listener_config,
}

impl ListenerBuilder {
    pub fn new(channels_out: u32) -> Self {
        let config = unsafe { sys::ma_spatializer_listener_config_init(channels_out) };
        Self { config }
    }

    pub fn build_f32(&self) -> MaResult<Listener<f32>> {
        Listener::<f32>::build(&self.config)
    }
}

mod sp_listener_ffi {
    use std::sync::Arc;

    use maudio_sys::ffi as sys;

    use crate::{
        audio::{
            channels::Channel, dsp::spatializer::Listener, math::vec3::Vec3, spatial::cone::Cone,
        },
        engine::AllocationCallbacks,
        pcm_frames::PcmFormat,
        AsRawRef, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_spatializer_listener_init(
        config: &sys::ma_spatializer_listener_config,
        alloc: Option<Arc<AllocationCallbacks>>,
        listener: *mut sys::ma_spatializer_listener,
    ) -> MaResult<()> {
        let alloc_cb: *const sys::ma_allocation_callbacks =
            alloc.clone().map_or(core::ptr::null(), |c| c.as_raw_ptr());
        let res =
            unsafe { sys::ma_spatializer_listener_init(config as *const _, alloc_cb, listener) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_spatializer_listener_uninit<F: PcmFormat>(listener: &mut Listener<F>) {
        unsafe {
            sys::ma_spatializer_listener_uninit(listener.to_raw(), std::ptr::null_mut());
        };
    }

    #[inline]
    pub fn ma_spatializer_listener_get_channel_map<F: PcmFormat>(
        listener: &Listener<F>,
    ) -> Vec<Channel> {
        let channels_out = listener.channels_out as usize;

        let res = unsafe { sys::ma_spatializer_listener_get_channel_map(listener.to_raw()) };
        if res.is_null() {
            return Vec::new();
        }
        let channel_map = unsafe { std::slice::from_raw_parts(res, channels_out) };
        channel_map.iter().copied().map(Channel::from_raw).collect()
    }

    #[inline]
    pub fn ma_spatializer_listener_set_cone<F: PcmFormat>(listener: &mut Listener<F>, cone: Cone) {
        unsafe {
            sys::ma_spatializer_listener_set_cone(
                listener.to_raw(),
                cone.inner_angle_rad,
                cone.outer_angle_rad,
                cone.outer_gain,
            );
        }
    }

    #[inline]
    pub fn ma_spatializer_listener_get_cone<F: PcmFormat>(listener: &Listener<F>) -> Cone {
        let mut inner = 0.0f32;
        let mut outer = 0.0f32;
        let mut gain = 1.0f32;
        unsafe {
            sys::ma_spatializer_listener_get_cone(
                listener.to_raw(),
                &mut inner,
                &mut outer,
                &mut gain,
            );
        };
        Cone {
            inner_angle_rad: inner,
            outer_angle_rad: outer,
            outer_gain: gain,
        }
    }

    #[inline]
    pub fn ma_spatializer_listener_set_position<F: PcmFormat>(
        listener: &mut Listener<F>,
        position: Vec3,
    ) {
        unsafe {
            sys::ma_spatializer_listener_set_position(
                listener.to_raw(),
                position.x,
                position.y,
                position.z,
            );
        }
    }

    #[inline]
    pub fn ma_spatializer_listener_get_position<F: PcmFormat>(listener: &Listener<F>) -> Vec3 {
        let res = unsafe { sys::ma_spatializer_listener_get_position(listener.to_raw()) };
        res.into()
    }

    #[inline]
    pub fn ma_spatializer_listener_get_direction<F: PcmFormat>(listener: &Listener<F>) -> Vec3 {
        let res = unsafe { sys::ma_spatializer_listener_get_direction(listener.to_raw()) };
        res.into()
    }

    #[inline]
    pub fn ma_spatializer_listener_set_direction<F: PcmFormat>(
        listener: &mut Listener<F>,
        direction: Vec3,
    ) {
        unsafe {
            sys::ma_spatializer_listener_set_direction(
                listener.to_raw(),
                direction.x,
                direction.y,
                direction.z,
            )
        };
    }

    #[inline]
    pub fn ma_spatializer_listener_set_velocity<F: PcmFormat>(
        listener: &mut Listener<F>,
        velocity: Vec3,
    ) {
        unsafe {
            sys::ma_spatializer_listener_set_velocity(
                listener.to_raw(),
                velocity.x,
                velocity.y,
                velocity.z,
            )
        };
    }

    #[inline]
    pub fn ma_spatializer_listener_get_velocity<F: PcmFormat>(listener: &Listener<F>) -> Vec3 {
        let res = unsafe { sys::ma_spatializer_listener_get_velocity(listener.to_raw()) };
        res.into()
    }

    #[inline]
    pub fn ma_spatializer_listener_set_speed_of_sound<F: PcmFormat>(
        listener: &mut Listener<F>,
        speed: f32,
    ) {
        unsafe {
            sys::ma_spatializer_listener_set_speed_of_sound(listener.to_raw(), speed);
        }
    }

    #[inline]
    pub fn ma_spatializer_listener_get_speed_of_sound<F: PcmFormat>(listener: &Listener<F>) -> f32 {
        unsafe { sys::ma_spatializer_listener_get_speed_of_sound(listener.to_raw()) }
    }

    #[inline]
    pub fn ma_spatializer_listener_set_world_up<F: PcmFormat>(
        listener: &mut Listener<F>,
        up: Vec3,
    ) {
        unsafe {
            sys::ma_spatializer_listener_set_world_up(listener.to_raw(), up.x, up.y, up.z);
        }
    }

    #[inline]
    pub fn ma_spatializer_listener_get_world_up<F: PcmFormat>(listener: &Listener<F>) -> Vec3 {
        let vec = unsafe { sys::ma_spatializer_listener_get_world_up(listener.to_raw()) };
        vec.into()
    }

    #[inline]
    pub fn ma_spatializer_listener_set_enabled<F: PcmFormat>(
        listener: &mut Listener<F>,
        enabled: bool,
    ) {
        unsafe {
            sys::ma_spatializer_listener_set_enabled(listener.to_raw(), enabled as u32);
        }
    }

    #[inline]
    pub fn ma_spatializer_listener_is_enabled<F: PcmFormat>(listener: &Listener<F>) -> bool {
        let res = unsafe { sys::ma_spatializer_listener_is_enabled(listener.to_raw()) };
        res == 1
    }
}

impl<F: PcmFormat> Drop for Listener<F> {
    fn drop(&mut self) {
        sp_listener_ffi::ma_spatializer_listener_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

#[cfg(test)]
mod tests {
    use crate::audio::channels::ChannelPosition;

    use super::*;
    fn assert_vec3_eq(actual: Vec3, expected: Vec3) {
        assert_f32_near(actual.x, expected.x);
        assert_f32_near(actual.y, expected.y);
        assert_f32_near(actual.z, expected.z);
    }

    fn assert_cone_eq(actual: Cone, expected: Cone) {
        assert_f32_near(actual.inner_angle_rad, expected.inner_angle_rad);
        assert_f32_near(actual.outer_angle_rad, expected.outer_angle_rad);
        assert_f32_near(actual.outer_gain, expected.outer_gain);
    }

    fn assert_f32_near(actual: f32, expected: f32) {
        const EPSILON: f32 = 0.00001;

        assert!(
            (actual - expected).abs() <= EPSILON,
            "expected {actual} to be approximately {expected}"
        );
    }

    #[test]
    fn spatializer_test_builds_with_channel_counts() -> MaResult<()> {
        let spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        assert_eq!(spatializer.channels_in, 1);
        assert_eq!(spatializer.channels_out, 2);

        Ok(())
    }

    #[test]
    fn spatializer_test_returns_input_and_output_channels() -> MaResult<()> {
        let spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        assert_eq!(spatializer.input_channels(), 1);
        assert_eq!(spatializer.output_channels(), 2);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_master_volume() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        spatializer.set_master_volume(0.25)?;

        assert_eq!(spatializer.master_volume()?, 0.25);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_positioning() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        spatializer.set_positioning(Positioning::Relative);

        assert_eq!(spatializer.positioning()?, Positioning::Relative);

        spatializer.set_positioning(Positioning::Absolute);

        assert_eq!(spatializer.positioning()?, Positioning::Absolute);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_rolloff() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        spatializer.set_rolloff(2.5);

        assert_eq!(spatializer.rolloff(), 2.5);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_min_gain() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        spatializer.set_min_gain(0.25);

        assert_eq!(spatializer.min_gain(), 0.25);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_max_gain() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        spatializer.set_max_gain(0.75);

        assert_eq!(spatializer.max_gain(), 0.75);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_min_distance() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        spatializer.set_min_distance(3.0);

        assert_eq!(spatializer.min_distance(), 3.0);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_max_distance() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        spatializer.set_max_distance(100.0);

        assert_eq!(spatializer.max_distance(), 100.0);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_cone() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        let cone = Cone {
            inner_angle_rad: 0.25,
            outer_angle_rad: 0.75,
            outer_gain: 0.5,
        };

        spatializer.set_cone(cone);

        assert_cone_eq(spatializer.cone(), cone);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_doppler_factor() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        spatializer.set_doppler_factor(1.5);

        assert_eq!(spatializer.doppler_factor(), 1.5);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_directional_attenuation_factor() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        spatializer.set_directional_attenuation_factor(0.75);

        assert_eq!(spatializer.directional_attenuation_factor(), 0.75);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_position() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        let position = Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };

        spatializer.set_position(position);

        assert_vec3_eq(spatializer.position(), position);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_direction() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        let direction = Vec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        };

        spatializer.set_direction(direction);

        assert_vec3_eq(spatializer.direction(), direction);

        Ok(())
    }

    #[test]
    fn spatializer_test_set_and_get_velocity() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        let velocity = Vec3 {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        };

        spatializer.set_velocity(velocity);

        assert_vec3_eq(spatializer.velocity(), velocity);

        Ok(())
    }

    #[test]
    fn spatializer_test_get_attenuation_model() -> MaResult<()> {
        let spatializer = SpatializerBuilder::new(1, 2).build_f32()?;

        assert_eq!(spatializer.attenuation_model()?, AttenuationModel::Inverse);

        Ok(())
    }

    #[test]
    fn spatializer_test_relative_position_and_direction_with_default_listener() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;
        spatializer.set_positioning(Positioning::Relative);
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        let (relative_position, relative_direction) =
            spatializer.relative_position_and_direction(&mut listener);

        assert_vec3_eq(
            relative_position,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        );

        assert_vec3_eq(
            relative_direction,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
        );

        Ok(())
    }

    #[test]
    fn spatializer_test_relative_position_and_direction_changes_with_listener() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        spatializer.set_position(Vec3 {
            x: 10.0,
            y: 0.0,
            z: 0.0,
        });

        listener.set_position(Vec3 {
            x: 4.0,
            y: 0.0,
            z: 0.0,
        });

        let (relative_position, _relative_direction) =
            spatializer.relative_position_and_direction(&mut listener);

        assert_vec3_eq(
            relative_position,
            Vec3 {
                x: 6.0,
                y: 0.0,
                z: 0.0,
            },
        );

        Ok(())
    }

    #[test]
    fn spatializer_test_process_pcm_frames_writes_output() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        spatializer.set_position(Vec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        });

        let frames_in = [1.0_f32, 1.0, 1.0, 1.0];
        let mut frames_out = [0.0_f32; 8];

        spatializer.process_pcm_frames(&mut listener, &mut frames_out, &frames_in)?;

        assert!(frames_out.iter().any(|sample| *sample != 0.0));

        Ok(())
    }

    #[test]
    fn spatializer_test_process_pcm_frames_respects_master_volume_zero() -> MaResult<()> {
        let mut spatializer = SpatializerBuilder::new(1, 2).build_f32()?;
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        spatializer.set_master_volume(0.0)?;

        let frames_in = [1.0_f32, 1.0, 1.0, 1.0];
        let mut frames_out = [1.0_f32; 8];

        spatializer.process_pcm_frames(&mut listener, &mut frames_out, &frames_in)?;

        assert!(frames_out.iter().all(|sample| *sample == 0.0));

        Ok(())
    }

    //
    // Listener tests
    //

    #[test]
    fn spatializer_listener_test_builds_with_stereo_output() -> MaResult<()> {
        let listener = ListenerBuilder::new(2).build_f32()?;

        assert_eq!(listener.channels_out, 2);

        Ok(())
    }

    #[test]
    fn spatializer_listener_test_returns_channel_map_for_output_channels() -> MaResult<()> {
        let listener = ListenerBuilder::new(2).build_f32()?;

        let channel_map = listener.channel_map();

        assert_eq!(channel_map.len(), 2);
        assert_eq!(channel_map[0], ChannelPosition::SideLeft.into());
        assert_eq!(channel_map[1], ChannelPosition::SideRight.into());

        Ok(())
    }

    #[test]
    fn spatializer_listener_test_fails_when_channels_out_is_zero() -> MaResult<()> {
        let res = ListenerBuilder::new(0).build_f32();

        assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn spatializer_listener_test_set_and_get_cone() -> MaResult<()> {
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        let cone = Cone {
            inner_angle_rad: 0.25,
            outer_angle_rad: 0.75,
            outer_gain: 0.5,
        };

        listener.set_cone(cone);

        assert_cone_eq(listener.cone(), cone);

        Ok(())
    }

    #[test]
    fn spatializer_listener_test_and_get_position() -> MaResult<()> {
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        let position = Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };

        listener.set_position(position);

        assert_vec3_eq(listener.position(), position);

        Ok(())
    }

    #[test]
    fn spatializer_listener_test_set_and_get_direction() -> MaResult<()> {
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        let direction = Vec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        };

        listener.set_direction(direction);

        assert_vec3_eq(listener.direction(), direction);

        Ok(())
    }

    #[test]
    fn spatializer_listener_test_set_and_get_velocity() -> MaResult<()> {
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        let velocity = Vec3 {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        };

        listener.set_velocity(velocity);

        assert_vec3_eq(listener.velocity(), velocity);

        Ok(())
    }

    #[test]
    fn spatializer_listener_test_set_and_get_speed_of_sound() -> MaResult<()> {
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        listener.set_speed_of_sound(123.45);

        assert_eq!(listener.speed_of_sound(), 123.45);

        Ok(())
    }

    #[test]
    fn spatializer_listener_test_set_and_get_world_up() -> MaResult<()> {
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        let up = Vec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        };

        listener.set_world_up(up);

        assert_vec3_eq(listener.world_up(), up);

        Ok(())
    }

    #[test]
    fn spatializer_listener_test_set_and_get_enabled() -> MaResult<()> {
        let mut listener = ListenerBuilder::new(2).build_f32()?;

        listener.set_enabled(false);
        assert!(!listener.is_enabled());

        listener.set_enabled(true);
        assert!(listener.is_enabled());

        Ok(())
    }
}
