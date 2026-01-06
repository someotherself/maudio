use maudio_sys::ffi as sys;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
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
