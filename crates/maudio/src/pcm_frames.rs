//! PCM format abstraction and utilities.
use crate::pcm_frames::private_pcm::PcmInterface;
use crate::{ErrorKinds, MaResult, MaudioError};

/// The native miniaudio signed 24 bit format represented as 3 bytes packed.
#[derive(Clone, Copy)]
pub struct S24Packed {}

/// Signed 24 bit format, represented as i32 with extended sign.
#[derive(Clone, Copy)]
pub struct S24 {}

/// Handles interleaved frames only. Not used
fn get_len(frames: u64, channels: u32, storage_units: usize) -> MaResult<usize> {
    let len = frames
        .checked_mul(channels as u64)
        .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
            op: "frames * channels",
            lhs: frames,
            rhs: channels as u64,
        }))?;

    let len = len
        .checked_mul(storage_units as u64)
        .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
            op: "samples * storage units per sample",
            lhs: len,
            rhs: storage_units as u64,
        }))?;

    let len: usize = len.try_into().map_err(|_| {
        MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
            op: "len u64 -> usize",
            lhs: len,
            rhs: usize::MAX as u64,
        })
    })?;

    Ok(len)
}

/// Handles interleaved frames only.
fn pcm_i32_to_u8(src: &[i32], frames: usize, channels: usize) -> MaResult<Vec<u8>> {
    // lenght read from src needs to be adjusted to the number of channels
    let len = frames
        .checked_mul(channels)
        .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
            op: "PcmSamples * channels",
            lhs: frames as u64,
            rhs: channels as u64,
        }))?;

    // length of out buffer is frames * channels * 3
    let out_len =
        len.checked_mul(3)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                op: "PcmSamples * channels to S24Packed",
                lhs: len as u64,
                rhs: 3,
            }))?;

    let mut data: Vec<u8> = Vec::with_capacity(out_len);

    for &sample in &src[..len] {
        if sample > 0x7FFFFF {
            return Err(crate::MaudioError::new_ma_error(ErrorKinds::S24OverFlow));
        }
        if sample < -0x800000 {
            return Err(MaudioError::new_ma_error(ErrorKinds::S24UnderFlow));
        }

        data.push(sample as u8);
        data.push((sample >> 8) as u8);
        data.push((sample >> 16) as u8);
    }
    Ok(data)
}

pub(crate) trait PcmFormatInternal: PcmFormat {
    // Used after a read_from_pcm
    fn storage_to_pcm_internal(storage: Vec<Self::StorageUnit>) -> MaResult<Vec<Self::PcmUnit>> {
        <Self as PcmFormat>::__PcmFramesProvider::storage_to_pcm(storage)
    }

    /// Available capacity is the number of frames we can read.
    /// It must be guaranteed that the frames can fit in both src and dst
    ///
    /// Handles interleaved frames only
    fn write_to_storage_internal(
        dst: &mut [Self::StorageUnit],
        src: &[Self::PcmUnit],
        avail_capacity: usize,
        channels: usize,
    ) -> MaResult<usize> {
        <Self as PcmFormat>::__PcmFramesProvider::write_to_storage(
            dst,
            src,
            avail_capacity,
            channels,
        )
    }

    /// Used during a `write_with` function for the PcmRingBuffer
    /// len == amount of frames we will copy from src to dst (capacity.min(desired_items))
    ///
    /// Handles interleaved frames only
    fn write_with_to_storage_internal<C>(
        dst: &mut [Self::StorageUnit],
        len: usize,
        f: C,
        channels: usize,
    ) -> MaResult<usize>
    where
        C: FnOnce(&mut [Self::PcmUnit]) -> usize,
    {
        <Self as PcmFormat>::__PcmFramesProvider::write_with_to_storage(dst, len, f, channels)
    }

    // The implementation for each format will have to convert between StorageUnit and PcmUnit types
    // len == amount of frames we should try to read
    /// Used during a `read` function for the PcmRingBuffer
    ///
    /// Handles interleaved frames only
    fn read_from_storage_internal(
        src: &[Self::StorageUnit],
        dst: &mut [Self::PcmUnit],
        len: usize,
        channels: usize,
    ) -> MaResult<usize> {
        <Self as PcmFormat>::__PcmFramesProvider::read_from_storage(src, dst, len, channels)
    }

    fn read_with_from_storage_internal<C>(
        src: &[Self::StorageUnit],
        len: usize,
        f: C,
        channels: usize,
    ) -> MaResult<usize>
    where
        C: FnOnce(&[Self::PcmUnit]) -> usize,
    {
        <Self as PcmFormat>::__PcmFramesProvider::read_with_from_storage(src, len, f, channels)
    }
}

impl<T: PcmFormat> PcmFormatInternal for T {}

pub(crate) mod private_pcm {
    use crate::{
        pcm_frames::{pcm_i32_to_u8, PcmFormat, S24Packed, S24},
        ErrorKinds, MaResult, MaudioError,
    };

    pub trait PcmInterface<T: PcmFormat + ?Sized> {
        fn storage_to_pcm(storage: Vec<T::StorageUnit>) -> MaResult<Vec<T::PcmUnit>>;
        fn write_to_storage(
            dst: &mut [T::StorageUnit],
            src: &[T::PcmUnit],
            avail_capacity: usize,
            channels: usize,
        ) -> MaResult<usize>;
        fn write_with_to_storage<C>(
            dst: &mut [T::StorageUnit],
            avail_capacity: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&mut [T::PcmUnit]) -> usize;
        fn read_from_storage(
            src: &[T::StorageUnit],
            dst: &mut [T::PcmUnit],
            avail: usize,
            channels: usize,
        ) -> MaResult<usize>;
        fn read_with_from_storage<C>(
            src: &[T::StorageUnit],
            avail: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&[T::PcmUnit]) -> usize;
    }

    pub struct PcmU8Provider;
    pub struct PcmI16Provider;
    pub struct PcmI32Provider;
    pub struct PcmS24Provider;
    pub struct PcmS24PackedProvider;
    pub struct PcmF32Provider;

    impl PcmInterface<u8> for PcmU8Provider {
        fn storage_to_pcm(storage: Vec<u8>) -> MaResult<Vec<u8>> {
            Ok(storage)
        }

        fn write_to_storage(
            dst: &mut [u8],
            src: &[u8],
            avail_capacity: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = avail_capacity
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail_capacity as u64,
                    rhs: channels as u64,
                }))?;
            dst[..len].copy_from_slice(&src[..len]);
            // We return the number of frames we read
            Ok(avail_capacity)
        }

        fn write_with_to_storage<C>(
            dst: &mut [u8],
            cap_frames: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&mut [u8]) -> usize,
        {
            let len = cap_frames
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: cap_frames as u64,
                    rhs: channels as u64,
                }))?;

            // written must be the number of frames (documented to the user)
            let written = f(&mut dst[..len]);

            debug_assert!(written <= cap_frames);
            if written > cap_frames {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: cap_frames,
                        written,
                    },
                ));
            }

            Ok(written)
        }

        fn read_from_storage(
            src: &[u8],
            dst: &mut [u8],
            avail_capacity: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = avail_capacity
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail_capacity as u64,
                    rhs: channels as u64,
                }))?;
            dst[..len].copy_from_slice(&src[..len]);

            // We return the number of frames we read
            Ok(avail_capacity)
        }

        fn read_with_from_storage<C>(
            src: &[u8],
            avail: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&[u8]) -> usize,
        {
            let len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            let frames_read = f(&src[..len]);

            debug_assert!(frames_read <= avail);
            if frames_read > avail {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: avail,
                        written: frames_read,
                    },
                ));
            }

            Ok(frames_read)
        }
    }

    impl PcmInterface<i16> for PcmI16Provider {
        fn storage_to_pcm(storage: Vec<i16>) -> MaResult<Vec<i16>> {
            Ok(storage)
        }

        fn write_to_storage(
            dst: &mut [i16],
            src: &[i16],
            avail_capacity: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = avail_capacity
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail_capacity as u64,
                    rhs: channels as u64,
                }))?;

            dst[..len].copy_from_slice(&src[..len]);
            // We return the number of frames we read
            Ok(avail_capacity)
        }

        fn write_with_to_storage<C>(
            dst: &mut [i16],
            cap_frames: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&mut [i16]) -> usize,
        {
            let len = cap_frames
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: cap_frames as u64,
                    rhs: channels as u64,
                }))?;

            // written must be the number of frames (documented to the user)
            let written = f(&mut dst[..len]);

            debug_assert!(written <= cap_frames);
            if written > cap_frames {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: cap_frames,
                        written,
                    },
                ));
            }

            Ok(written)
        }

        fn read_from_storage(
            src: &[i16],
            dst: &mut [i16],
            avail: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            dst[..len].copy_from_slice(&src[..len]);
            // We return the number of frames we read
            Ok(avail)
        }

        fn read_with_from_storage<C>(
            src: &[i16],
            avail: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&[i16]) -> usize,
        {
            let len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            let frames_read = f(&src[..len]);

            debug_assert!(frames_read <= avail);
            if frames_read > avail {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: avail,
                        written: frames_read,
                    },
                ));
            }

            Ok(frames_read)
        }
    }

    impl PcmInterface<S24> for PcmS24Provider {
        fn storage_to_pcm(storage: Vec<u8>) -> MaResult<Vec<i32>> {
            let total_items = storage.as_slice().len();

            debug_assert!(total_items % 3 == 0);
            if total_items % 3 != 0 {
                return Err(crate::MaudioError::new_ma_error(
                    ErrorKinds::InvalidPackedSampleSize {
                        bytes_per_sample: 3,
                        actual_len: total_items,
                    },
                ));
            }

            // TODO: Remove the iterator and allocate all at once
            let data: Vec<i32> = storage
                .as_slice()
                .chunks_exact(3)
                .map(|c| {
                    let v: i32 = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16);
                    (v << 8) >> 8
                })
                .collect();
            Ok(data)
        }

        fn write_to_storage(
            dst: &mut [u8],
            src: &[i32],
            avail_capacity: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let data = pcm_i32_to_u8(src, avail_capacity, channels)?;
            let len = data.len();
            dst[..len].copy_from_slice(&data[..len]);
            Ok(len)
        }

        fn write_with_to_storage<C>(
            dst: &mut [u8],
            avail_capacity: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&mut [i32]) -> usize,
        {
            // Create a temporary storage and write the i32 into it
            let tmp_len = avail_capacity
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail_capacity as u64,
                    rhs: channels as u64,
                }))?;

            let mut tmp: Vec<i32> = vec![];
            tmp.resize_with(tmp_len, || 0i32);

            let written = f(&mut tmp[..tmp_len]);

            debug_assert!(written <= avail_capacity);
            if written > avail_capacity {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: avail_capacity,
                        written,
                    },
                ));
            }

            // Truncate the tmp storage to the frames actually written
            let written_len = written
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: written as u64,
                    rhs: channels as u64,
                }))?;

            tmp.truncate(written_len);

            // Convert them to S24 packed and write at the same time
            for (i, &sample) in tmp.iter().enumerate() {
                if sample > 0x7FFFFF {
                    return Err(crate::MaudioError::new_ma_error(ErrorKinds::S24OverFlow));
                }
                if sample < -0x800000 {
                    return Err(MaudioError::new_ma_error(ErrorKinds::S24UnderFlow));
                }

                let o = i.checked_mul(3).ok_or(MaudioError::new_ma_error(
                    ErrorKinds::IntegerOverflow {
                        op: "S24Packed byte offset (sample_index * 3)",
                        lhs: i as u64,
                        rhs: 3,
                    },
                ))?;
                dst[o] = sample as u8;
                dst[o + 1] = (sample >> 8) as u8;
                dst[o + 2] = (sample >> 16) as u8;
            }

            Ok(written)
        }

        fn read_from_storage(
            src: &[u8],
            dst: &mut [i32],
            avail: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "read: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            let total_bytes = len.checked_mul(3).ok_or(MaudioError::new_ma_error(
                ErrorKinds::IntegerOverflow {
                    op: "Frames available adjusted to bytes (S24Packed)",
                    lhs: len as u64,
                    rhs: 3,
                },
            ))?;

            for (i, c) in src[..total_bytes].chunks_exact(3).enumerate() {
                let v: i32 = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16);
                let v = (v << 8) >> 8;
                dst[i] = v;
            }
            Ok(avail)
        }

        fn read_with_from_storage<C>(
            src: &[u8],
            avail: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&[i32]) -> usize,
        {
            // Create temp storage and read into it first
            // tmp_len is the length of the Vec<i32
            let tmp_len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "read: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            // total_bytes is the len of the [u8] slice we read from
            let total_bytes = tmp_len.checked_mul(3).ok_or(MaudioError::new_ma_error(
                ErrorKinds::IntegerOverflow {
                    op: "read: frames to S24Packed",
                    lhs: tmp_len as u64,
                    rhs: 3,
                },
            ))?;

            // Convert and write into the tmp storage
            let mut tmp: Vec<i32> = Vec::with_capacity(tmp_len);
            for c in src[..total_bytes].chunks_exact(3) {
                let v: i32 = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16);
                let v = (v << 8) >> 8;
                tmp.push(v);
            }

            // User should return number of frames
            let frames_read = f(&tmp[..tmp_len]);

            debug_assert!(frames_read <= avail);
            if frames_read > avail {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::ReadExceedsAvailability {
                        available: avail,
                        read: frames_read,
                    },
                ));
            }

            Ok(frames_read)
        }
    }

    impl PcmInterface<S24Packed> for PcmS24PackedProvider {
        fn storage_to_pcm(storage: Vec<u8>) -> MaResult<Vec<u8>> {
            Ok(storage)
        }

        fn write_to_storage(
            dst: &mut [u8],
            src: &[u8],
            cap_frames: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = cap_frames
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: cap_frames as u64,
                    rhs: channels as u64,
                }))?;

            let max_cap = len.checked_mul(3).ok_or(MaudioError::new_ma_error(
                ErrorKinds::IntegerOverflow {
                    op: "write: frames to S24Packed",
                    lhs: len as u64,
                    rhs: 3,
                },
            ))?;

            dst[..max_cap].copy_from_slice(&src[..max_cap]);
            Ok(cap_frames)
        }

        /// Truncate the ammount of bytes written to the nearest multiple of 3
        fn write_with_to_storage<C>(
            dst: &mut [u8],
            cap_frames: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&mut [u8]) -> usize,
        {
            let len = cap_frames
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: cap_frames as u64,
                    rhs: channels as u64,
                }))?;

            let max_cap = len.checked_mul(3).ok_or(MaudioError::new_ma_error(
                ErrorKinds::IntegerOverflow {
                    op: "write: frames to S24Packed",
                    lhs: len as u64,
                    rhs: 3,
                },
            ))?;

            // written must be the number of frames (documented to the user)
            let frames_written = f(&mut dst[..max_cap]);

            debug_assert!(frames_written <= cap_frames);
            if frames_written > cap_frames {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: cap_frames,
                        written: frames_written,
                    },
                ));
            }

            Ok(frames_written)
        }

        fn read_from_storage(
            src: &[u8],
            dst: &mut [u8],
            avail: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            let max_cap = len.checked_mul(3).ok_or(MaudioError::new_ma_error(
                ErrorKinds::IntegerOverflow {
                    op: "write: frames to S24Packed",
                    lhs: len as u64,
                    rhs: 3,
                },
            ))?;

            dst[..max_cap].copy_from_slice(&src[..max_cap]);
            Ok(avail)
        }

        fn read_with_from_storage<C>(
            src: &[u8],
            avail: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&[u8]) -> usize,
        {
            let len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            let max_cap = len.checked_mul(3).ok_or(MaudioError::new_ma_error(
                ErrorKinds::IntegerOverflow {
                    op: "write: frames to S24Packed",
                    lhs: len as u64,
                    rhs: 3,
                },
            ))?;

            let frames_read = f(&src[..max_cap]);

            debug_assert!(frames_read <= avail);
            if frames_read > avail {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: avail,
                        written: frames_read,
                    },
                ));
            }

            Ok(frames_read)
        }
    }

    impl PcmInterface<i32> for PcmI32Provider {
        fn storage_to_pcm(storage: Vec<i32>) -> MaResult<Vec<i32>> {
            Ok(storage)
        }

        fn write_to_storage(
            dst: &mut [i32],
            src: &[i32],
            avail_capacity: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = avail_capacity
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail_capacity as u64,
                    rhs: channels as u64,
                }))?;

            dst[..len].copy_from_slice(&src[..len]);
            Ok(avail_capacity)
        }

        fn write_with_to_storage<C>(
            dst: &mut [i32],
            cap_frames: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&mut [i32]) -> usize,
        {
            let len = cap_frames
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: cap_frames as u64,
                    rhs: channels as u64,
                }))?;

            // written must be the number of frames (documented to the user)
            let written = f(&mut dst[..len]);

            debug_assert!(written <= cap_frames);
            if written > cap_frames {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: cap_frames,
                        written,
                    },
                ));
            }

            Ok(written)
        }

        fn read_from_storage(
            src: &[i32],
            dst: &mut [i32],
            avail: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            dst[..len].copy_from_slice(&src[..len]);
            Ok(avail)
        }

        fn read_with_from_storage<C>(
            src: &[i32],
            avail: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&[i32]) -> usize,
        {
            let len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            let frames_read = f(&src[..len]);

            debug_assert!(frames_read <= avail);
            if frames_read > avail {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: avail,
                        written: frames_read,
                    },
                ));
            }

            Ok(frames_read)
        }
    }

    impl PcmInterface<f32> for PcmF32Provider {
        fn storage_to_pcm(storage: Vec<f32>) -> MaResult<Vec<f32>> {
            Ok(storage)
        }

        fn write_to_storage(
            dst: &mut [f32],
            src: &[f32],
            avail_capacity: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = avail_capacity
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail_capacity as u64,
                    rhs: channels as u64,
                }))?;

            dst[..len].copy_from_slice(&src[..len]);
            // We return the number of frames we read
            Ok(avail_capacity)
        }

        fn write_with_to_storage<C>(
            dst: &mut [f32],
            cap_frames: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&mut [f32]) -> usize,
        {
            let len = cap_frames
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: cap_frames as u64,
                    rhs: channels as u64,
                }))?;

            // written must be the number of frames (documented to the user)
            let written = f(&mut dst[..len]);

            debug_assert!(written <= cap_frames);
            if written > cap_frames {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: cap_frames,
                        written,
                    },
                ));
            }

            Ok(written)
        }

        fn read_from_storage(
            src: &[f32],
            dst: &mut [f32],
            avail: usize,
            channels: usize,
        ) -> MaResult<usize> {
            let len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            dst[..len].copy_from_slice(&src[..len]);
            // We return the number of frames we read
            Ok(avail)
        }

        fn read_with_from_storage<C>(
            src: &[f32],
            avail: usize,
            f: C,
            channels: usize,
        ) -> MaResult<usize>
        where
            C: FnOnce(&[f32]) -> usize,
        {
            let len = avail
                .checked_mul(channels)
                .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                    op: "write: frames * channels",
                    lhs: avail as u64,
                    rhs: channels as u64,
                }))?;

            let frames_read = f(&src[..len]);

            debug_assert!(frames_read <= avail);
            if frames_read > avail {
                return Err(MaudioError::new_ma_error(
                    ErrorKinds::WriteExceedsCapacity {
                        capacity: avail,
                        written: frames_read,
                    },
                ));
            }

            Ok(frames_read)
        }
    }
}

/// PCM sample format marker used throughout the crate.
///
/// `PcmFormat` ties a Rust-facing sample type (what you read/write) to the
/// underlying representation expected by miniaudio (what is stored internally).
/// Most APIs in this crate are generic over `F: PcmFormat` so they can handle
/// both “direct” formats (`u8`, `i16`, `i32`, `f32`) and special cases such as
/// 24-bit PCM where the user-facing type and storage layout may differ.
///
/// In most cases you don’t implement this trait yourself—just choose an existing
/// format type (e.g. `i16`, `f32`, [`S24`], [`S24Packed`]) when constructing buffers.
///
/// The associated items on this trait are primarily used internally to:
/// - define the user-facing sample unit (`PcmUnit`) vs the stored unit (`StorageUnit`);
/// - describe how many units make up a single interleaved PCM frame;
/// - select the internal conversion logic when a format requires it.
pub trait PcmFormat {
    type __PcmFramesProvider: private_pcm::PcmInterface<Self>;

    /// The sample format used by the user. Not a whole frame.
    type PcmUnit: Default + Copy;
    /// Sample format used by miniaudio. Not a whole frame.
    type StorageUnit: Default + Copy;
    /// Number of `StorageUnit` items per channel sample in a buffer.
    ///
    /// Examples: `S24Packed = 3`, `S24 = 1`, `u8 = 1`.
    const VEC_STORE_UNITS_PER_FRAME: usize;
    /// Number of `PcmUnit` items per channel sample in a buffer.
    ///
    /// Examples: `S24Packed = 1`, `S24 = 3`, `u8 = 1`.
    const VEC_PCM_UNITS_PER_FRAME: usize;
    /// Used for simple logic only, when we don't want to call the PcmInterface trait
    const DIRECT_READ: bool;
}

impl PcmFormat for u8 {
    type __PcmFramesProvider = private_pcm::PcmU8Provider;

    type PcmUnit = u8;
    type StorageUnit = Self::PcmUnit;
    const VEC_STORE_UNITS_PER_FRAME: usize = 1;
    const VEC_PCM_UNITS_PER_FRAME: usize = 1;
    const DIRECT_READ: bool = true;
}

impl PcmFormat for i16 {
    type __PcmFramesProvider = private_pcm::PcmI16Provider;

    type PcmUnit = i16;
    type StorageUnit = Self::PcmUnit;
    const VEC_STORE_UNITS_PER_FRAME: usize = 1;
    const VEC_PCM_UNITS_PER_FRAME: usize = 1;
    const DIRECT_READ: bool = true;
}

impl PcmFormat for S24Packed {
    type __PcmFramesProvider = private_pcm::PcmS24PackedProvider;

    type PcmUnit = u8;
    type StorageUnit = Self::PcmUnit;
    const VEC_STORE_UNITS_PER_FRAME: usize = 3;
    const VEC_PCM_UNITS_PER_FRAME: usize = 3;
    const DIRECT_READ: bool = true;
}

impl PcmFormat for S24 {
    type __PcmFramesProvider = private_pcm::PcmS24Provider;

    type PcmUnit = i32;
    type StorageUnit = u8;
    const VEC_STORE_UNITS_PER_FRAME: usize = 3;
    const VEC_PCM_UNITS_PER_FRAME: usize = 1;
    const DIRECT_READ: bool = false;
}

impl PcmFormat for i32 {
    type __PcmFramesProvider = private_pcm::PcmI32Provider;

    type PcmUnit = i32;
    type StorageUnit = Self::PcmUnit;
    const VEC_STORE_UNITS_PER_FRAME: usize = 1;
    const VEC_PCM_UNITS_PER_FRAME: usize = 1;
    const DIRECT_READ: bool = true;
}

impl PcmFormat for f32 {
    type __PcmFramesProvider = private_pcm::PcmF32Provider;
    type PcmUnit = f32;
    type StorageUnit = Self::PcmUnit;
    const VEC_STORE_UNITS_PER_FRAME: usize = 1;
    const VEC_PCM_UNITS_PER_FRAME: usize = 1;
    const DIRECT_READ: bool = true;
}
