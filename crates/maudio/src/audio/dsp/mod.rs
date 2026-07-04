//! Digital signal processing primitives.
//!
//! This module contains reusable DSP types that operate directly on PCM frames,
//! such as biquad, low-pass, high-pass, and band-pass filters.
//!
//! These types are independent of the engine and node graph. They can be used
//! from device callbacks, custom nodes, offline processing code, or any other
//! low-level audio pipeline.
pub mod delay_effect;
pub mod fader;
pub mod filters;
pub mod spatializer;
pub mod stereo_panner;
pub mod volume_gainer;
