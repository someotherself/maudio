//! Digital signal processing primitives.
//!
//! This module contains reusable DSP types that operate directly on PCM frames,
//! such as biquad, low-pass, high-pass, and band-pass filters.
//!
//! These types are independent of the engine and node graph. They can be used
//! from device callbacks, custom nodes, offline processing code, or any other
//! low-level audio pipeline.
pub mod biquad_filter;
pub mod bpf2_filter;
pub mod bpf_filter;
pub mod delay_filter;
pub mod hishelf2_filter;
pub mod hpf1_filter;
pub mod hpf2_filter;
pub mod hpf_filter;
pub mod loshelf2_filter;
pub mod lpf1_filter;
pub mod lpf2_filter;
pub mod lpf_filter;
pub mod notch2_filter;
pub mod peak2_filter;
