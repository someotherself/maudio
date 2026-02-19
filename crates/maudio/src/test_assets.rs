pub mod decoded_data {
    #[inline]
    fn assert_valid(channels: u32, frames: usize) {
        assert!(channels > 0, "channels must be > 0");
        assert!(frames > 0, "frames must be > 0");
    }

    /// Interleaved u8 (commonly used for unsigned 8-bit PCM).
    pub fn asset_interleaved_u8(channels: u32, frames: usize, base: u8) -> Vec<u8> {
        assert_valid(channels, frames);
        let ch = channels as usize;
        let mut out = Vec::with_capacity(frames * ch);
        for f in 0..frames {
            for c in 0..ch {
                let v = (base as u32 + (f as u32).wrapping_mul(7) + c as u32) & 0xFF;
                out.push(v as u8);
            }
        }
        out
    }

    /// Interleaved i16.
    pub fn asset_interleaved_i16(channels: u32, frames: usize, base: i16) -> Vec<i16> {
        assert_valid(channels, frames);
        let ch = channels as usize;
        let mut out = Vec::with_capacity(frames * ch);
        for f in 0..frames {
            for c in 0..ch {
                let v = base as i32 + (f as i32) * 1000 + (c as i32);
                out.push(v.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
            }
        }
        out
    }

    /// Interleaved i32 (full-range 32-bit PCM).
    pub fn asset_interleaved_i32(channels: u32, frames: usize, base: i32) -> Vec<i32> {
        assert_valid(channels, frames);
        let ch = channels as usize;
        let mut out = Vec::with_capacity(frames * ch);
        for f in 0..frames {
            for c in 0..ch {
                let v = base
                    .wrapping_add((f as i32).wrapping_mul(1000))
                    .wrapping_add(c as i32);
                out.push(v);
            }
        }
        out
    }

    /// Interleaved s24 stored in i32 ("S24 (i32)" container).
    ///
    /// Values are kept within the signed 24-bit range: [-2^23, 2^23-1].
    /// Convention: stored in the low 24 bits (sign-extended), which is a common representation.
    pub fn asset_interleaved_s24_i32(channels: u32, frames: usize, base: i32) -> Vec<i32> {
        assert_valid(channels, frames);
        let ch = channels as usize;
        let mut out = Vec::with_capacity(frames * ch);

        const S24_MIN: i32 = -(1 << 23);
        const S24_MAX: i32 = (1 << 23) - 1;

        for f in 0..frames {
            for c in 0..ch {
                let v = base
                    .wrapping_add((f as i32).wrapping_mul(1000))
                    .wrapping_add(c as i32);
                out.push(v.clamp(S24_MIN, S24_MAX));
            }
        }
        out
    }

    /// Interleaved packed s24: 3 bytes per sample, little-endian.
    ///
    /// Each sample is signed 24-bit in two's complement, stored as:
    /// [least significant byte, mid byte, most significant byte]
    pub fn asset_interleaved_s24_packed_le(channels: u32, frames: usize, base: i32) -> Vec<u8> {
        assert_valid(channels, frames);
        let ch = channels as usize;

        const S24_MIN: i32 = -(1 << 23);
        const S24_MAX: i32 = (1 << 23) - 1;

        let mut out = Vec::with_capacity(frames * ch * 3);

        for f in 0..frames {
            for c in 0..ch {
                let v = base
                    .wrapping_add((f as i32).wrapping_mul(1000))
                    .wrapping_add(c as i32);
                let s24 = v.clamp(S24_MIN, S24_MAX);

                // Two's complement signed 24-bit, keep low 24 bits.
                let u = s24 & 0x00FF_FFFF;

                out.push((u & 0xFF) as u8);
                out.push(((u >> 8) & 0xFF) as u8);
                out.push(((u >> 16) & 0xFF) as u8);
            }
        }

        out
    }

    /// Interleaved f32 in [-1, 1] approx.
    pub fn asset_interleaved_f32(channels: u32, frames: usize, base: f32) -> Vec<f32> {
        assert_valid(channels, frames);
        let ch = channels as usize;
        let mut out = Vec::with_capacity(frames * ch);
        for f in 0..frames {
            for c in 0..ch {
                let v = base + (f as f32) * 0.01 + (c as f32) * 0.001;
                // keep it sensible for audio tests
                out.push(v.clamp(-1.0, 1.0));
            }
        }
        out
    }
}

// Create a temporary (wav) file
pub mod temp_file {
    use std::time::{SystemTime, UNIX_EPOCH};

    pub(crate) struct TempFileGuard {
        path: std::path::PathBuf,
    }

    pub(crate) fn unique_tmp_path(ext: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("miniaudio_decoder_test_{nanos}.{ext}"));
        p
    }

    impl TempFileGuard {
        pub(crate) fn new(path: std::path::PathBuf) -> Self {
            Self { path }
        }

        pub(crate) fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TempFileGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Build a minimal PCM 16-bit little-endian WAV file.
pub(crate) fn wav_i16_le(channels: u16, sample_rate: u32, samples_interleaved: &[i16]) -> Vec<u8> {
    assert!(channels > 0);
    assert_eq!(samples_interleaved.len() % channels as usize, 0);

    let bits_per_sample: u16 = 16;
    let block_align: u16 = channels * (bits_per_sample / 8);
    let byte_rate: u32 = sample_rate * block_align as u32;
    let data_bytes_len: u32 = (samples_interleaved.len() * 2) as u32;

    let riff_chunk_size: u32 = 4 + (8 + 16) + (8 + data_bytes_len);

    let mut out = Vec::with_capacity((8 + riff_chunk_size) as usize);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_chunk_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = 1 (PCM)
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes_len.to_le_bytes());

    for s in samples_interleaved {
        out.extend_from_slice(&s.to_le_bytes());
    }

    out
}
