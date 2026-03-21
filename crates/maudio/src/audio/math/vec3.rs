//! Type definition for a 3D vector
use maudio_sys::ffi as sys;

/// A 3D vector used for positions, directions, and velocities in spatial audio.
///
/// Coordinates follow the OpenGL coordinate system and are expressed in world space using a right-handed system:
/// - +X: right
/// - +Y: up
/// - -Z: forward (towards the listener by default)
///
/// The unit scale is arbitrary, but typically represents meters. Larger values
/// result in greater distances between sounds and the listener, affecting
/// attenuation and spatial perception.
///
/// This type is used with methods such as [`Sound::set_position`](crate::sound::Sound),
/// [`Sound::set_direction`](crate::sound::Sound), and [`Sound::set_velocity`](crate::sound::Sound).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

impl From<sys::ma_vec3f> for Vec3 {
    fn from(v: sys::ma_vec3f) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<Vec3> for sys::ma_vec3f {
    fn from(v: Vec3) -> Self {
        sys::ma_vec3f {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}
