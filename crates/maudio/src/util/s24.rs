use crate::{audio::formats::SampleBuffer, ErrorKinds, MaResult, MaudioError};

/// The native miniaudio signed 24 bit format represented as 3 bytes packed.
pub struct S24Packed {}

/// Signed 24 bit format, represented as i32 with extended sign.
pub struct S24 {}

mod sealed {
    use crate::util::s24::{S24Packed, S24};

    pub trait Sealed {}

    impl Sealed for u8 {}
    impl Sealed for i16 {}
    impl Sealed for i32 {}
    impl Sealed for S24Packed {}
    impl Sealed for S24 {}
    impl Sealed for f32 {}
}

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

pub trait PcmFormat: sealed::Sealed {
    // Format of the input and output from/to the user
    /// The miniaudio samples format (u8, i16, i32, S24 or f32)
    type PcmUnit: Default + Copy;
    // Format used by miniaudio
    type StorageUnit: Default + Copy;
    const BYTES_PER_SAMPLE: usize;
    const STORAGE_UNITS_PER_SAMPLE: usize;

    fn new_zeroed(frames: u64, channels: u32) -> MaResult<SampleBuffer<Self::StorageUnit>>;
    // Used when passing pcm data from miniaudio to the user (ex: read_pcm_frames)
    fn storage_to_pcm(
        storage: SampleBuffer<Self::StorageUnit>,
    ) -> MaResult<SampleBuffer<Self::PcmUnit>>;
    // Used when passing pcm data from the user to miniaudio (ex: into buffers)
    fn pcm_to_storage(
        pcm: SampleBuffer<Self::PcmUnit>,
    ) -> MaResult<SampleBuffer<Self::StorageUnit>>;
}

impl PcmFormat for u8 {
    type PcmUnit = u8;
    type StorageUnit = u8;
    const BYTES_PER_SAMPLE: usize = size_of::<u8>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 1;

    fn new_zeroed(frames: u64, channels: u32) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        let len = get_len(frames, channels, Self::STORAGE_UNITS_PER_SAMPLE)?;
        let data = vec![0u8; len];
        Ok(SampleBuffer::new(data, channels))
    }

    #[inline]
    fn storage_to_pcm(
        storage: SampleBuffer<Self::StorageUnit>,
    ) -> MaResult<SampleBuffer<Self::PcmUnit>> {
        Ok(storage)
    }

    #[inline]
    fn pcm_to_storage(
        pcm: SampleBuffer<Self::PcmUnit>,
    ) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        Ok(pcm)
    }
}

impl PcmFormat for i16 {
    type PcmUnit = i16;
    type StorageUnit = i16;
    const BYTES_PER_SAMPLE: usize = size_of::<i16>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 1;

    fn new_zeroed(frames: u64, channels: u32) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        let len = get_len(frames, channels, Self::STORAGE_UNITS_PER_SAMPLE)?;
        let data = vec![0i16; len];
        Ok(SampleBuffer::new(data, channels))
    }

    #[inline]
    fn storage_to_pcm(
        storage: SampleBuffer<Self::StorageUnit>,
    ) -> MaResult<SampleBuffer<Self::PcmUnit>> {
        Ok(storage)
    }

    #[inline]
    fn pcm_to_storage(
        pcm: SampleBuffer<Self::PcmUnit>,
    ) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        Ok(pcm)
    }
}

impl PcmFormat for S24Packed {
    type PcmUnit = u8;
    type StorageUnit = u8;
    const BYTES_PER_SAMPLE: usize = size_of::<u8>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 3;

    fn new_zeroed(frames: u64, channels: u32) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        let len = get_len(frames, channels, Self::STORAGE_UNITS_PER_SAMPLE)?;
        let data = vec![0u8; len];
        Ok(SampleBuffer::new(data, channels))
    }

    #[inline]
    fn storage_to_pcm(
        storage: SampleBuffer<Self::StorageUnit>,
    ) -> MaResult<SampleBuffer<Self::PcmUnit>> {
        Ok(storage)
    }

    #[inline]
    fn pcm_to_storage(
        pcm: SampleBuffer<Self::PcmUnit>,
    ) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        Ok(pcm)
    }
}

impl PcmFormat for S24 {
    type PcmUnit = i32;
    type StorageUnit = u8;
    const BYTES_PER_SAMPLE: usize = size_of::<i32>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 1;

    fn new_zeroed(frames: u64, channels: u32) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        let len = get_len(frames, channels, Self::STORAGE_UNITS_PER_SAMPLE)?;
        let data = vec![0u8; len];
        Ok(SampleBuffer::new(data, channels))
    }

    #[inline]
    fn storage_to_pcm(
        storage: SampleBuffer<Self::StorageUnit>,
    ) -> MaResult<SampleBuffer<Self::PcmUnit>> {
        let total_samples = storage.len_samples();

        debug_assert!(total_samples % 3 == 0);
        if total_samples % 3 != 0 {
            // return
            return Err(MaudioError::new_ma_error(
                ErrorKinds::InvalidPackedSampleSize {
                    bytes_per_sample: 3,
                    actual_len: total_samples,
                },
            ));
        }

        let data: Vec<i32> = storage
            .as_slice()
            .chunks_exact(3)
            .map(|c| {
                let v: i32 = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16);
                (v << 8) >> 8
            })
            .collect();
        Ok(SampleBuffer::new(data, storage.channels()))
    }

    #[inline]
    fn pcm_to_storage(
        pcm: SampleBuffer<Self::PcmUnit>,
    ) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        let total_samples = pcm.len_samples();
        let out_len = total_samples
            .checked_mul(3)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                op: "PcmSamples to S24Packed",
                lhs: total_samples as u64,
                rhs: 3,
            }))?;

        let mut data: Vec<u8> = Vec::with_capacity(out_len);

        for &sample in pcm.as_slice() {
            if sample > 0x7FFFFF {
                return Err(MaudioError::new_ma_error(ErrorKinds::S24OverFlow));
            }
            if sample < -0x800000 {
                return Err(MaudioError::new_ma_error(ErrorKinds::S24UnderFlow));
            }

            data.push(sample as u8);
            data.push((sample >> 8) as u8);
            data.push((sample >> 16) as u8);
        }

        Ok(SampleBuffer::new(data, pcm.channels()))
    }
}

impl PcmFormat for i32 {
    type PcmUnit = i32;
    type StorageUnit = i32;
    const BYTES_PER_SAMPLE: usize = size_of::<i32>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 1;

    fn new_zeroed(frames: u64, channels: u32) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        let len = get_len(frames, channels, Self::STORAGE_UNITS_PER_SAMPLE)?;
        let data = vec![0i32; len];
        Ok(SampleBuffer::new(data, channels))
    }

    #[inline]
    fn storage_to_pcm(
        storage: SampleBuffer<Self::StorageUnit>,
    ) -> MaResult<SampleBuffer<Self::PcmUnit>> {
        Ok(storage)
    }

    #[inline]
    fn pcm_to_storage(
        pcm: SampleBuffer<Self::PcmUnit>,
    ) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        Ok(pcm)
    }
}

impl PcmFormat for f32 {
    type PcmUnit = f32;
    type StorageUnit = f32;
    const BYTES_PER_SAMPLE: usize = size_of::<f32>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 1;

    fn new_zeroed(frames: u64, channels: u32) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        let len = get_len(frames, channels, Self::STORAGE_UNITS_PER_SAMPLE)?;
        let data = vec![0f32; len];
        Ok(SampleBuffer::new(data, channels))
    }

    #[inline]
    fn storage_to_pcm(
        storage: SampleBuffer<Self::StorageUnit>,
    ) -> MaResult<SampleBuffer<Self::PcmUnit>> {
        Ok(storage)
    }

    #[inline]
    fn pcm_to_storage(
        pcm: SampleBuffer<Self::PcmUnit>,
    ) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        Ok(pcm)
    }
}
