//! Directional attenuation cone for spatial audio emitters.

/// A `Cone` describes how sound emitted from a source is attenuated based on
/// direction relative to the listener.
///
/// - Inside the inner cone, sound is played at full volume.
/// - Between the inner and outer cone, volume is smoothly attenuated.
/// - Outside the outer cone, volume is multiplied by `outer_gain`.
///
/// Angles are expressed in **radians**.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Cone {
    /// Inner cone angle in radians.
    pub inner_angle_rad: f32,
    /// Outer cone angle in radians.
    pub outer_angle_rad: f32,
    /// Gain multiplier outside the outer cone.
    pub outer_gain: f32,
}

impl Cone {
    /// Creates a new cone using angles in **radians**.
    ///
    /// No validation is performed; values are passed through directly.
    #[inline]
    pub const fn new(inner_angle_rad: f32, outer_angle_rad: f32, outer_gain: f32) -> Self {
        Self {
            inner_angle_rad,
            outer_angle_rad,
            outer_gain,
        }
    }

    /// Creates a new cone using angles in **degrees**.
    #[inline]
    pub fn from_degrees(inner_deg: f32, outer_deg: f32, outer_gain: f32) -> Self {
        let d2r = core::f32::consts::PI / 180.0;
        Self {
            inner_angle_rad: inner_deg * d2r,
            outer_angle_rad: outer_deg * d2r,
            outer_gain,
        }
    }

    /// Returns a cone with no directional attenuation.
    ///
    /// This is equivalent to an omnidirectional emitter.
    #[inline]
    pub const fn omni() -> Self {
        Self {
            inner_angle_rad: core::f32::consts::PI * 2.0,
            outer_angle_rad: core::f32::consts::PI * 2.0,
            outer_gain: 1.0,
        }
    }
}

impl Default for Cone {
    /// Returns an omnidirectional cone.
    #[inline]
    fn default() -> Self {
        Self::omni()
    }
}
