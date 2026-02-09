use crate::util::pcm_frames::private_pcm::PcmInterface;
use crate::{audio::formats::SampleBuffer, ErrorKinds, MaResult, MaudioError};

/// The native miniaudio signed 24 bit format represented as 3 bytes packed.
pub struct S24Packed {}

/// Signed 24 bit format, represented as i32 with extended sign.
pub struct S24 {}

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

pub(crate) trait PcmFormatInternal: PcmFormat {
    fn new_zeroed_internal(
        frames: u64,
        channels: u32,
    ) -> MaResult<SampleBuffer<Self::StorageUnit>> {
        <Self as PcmFormat>::__PcmFramesProvider::new_zeroed::<Self>(frames, channels)
    }

    fn truncate_to_frames_read_internal(
        storage: &mut SampleBuffer<Self::StorageUnit>,
        frames_read: u64,
    ) -> MaResult<()> {
        <Self as PcmFormat>::__PcmFramesProvider::truncate_to_frames_read::<Self>(
            storage,
            frames_read,
        )
    }

    fn storage_to_pcm_internal(
        storage: SampleBuffer<Self::StorageUnit>,
    ) -> MaResult<SampleBuffer<Self::PcmUnit>> {
        <Self as PcmFormat>::__PcmFramesProvider::storage_to_pcm::<Self>(storage)
    }
}

impl<T: PcmFormat + ?Sized> PcmFormatInternal for T {}

pub(crate) mod private_pcm {
    use crate::{
        audio::formats::SampleBuffer,
        util::pcm_frames::{get_len, PcmFormat, S24Packed, S24},
        ErrorKinds, MaResult, MaudioError,
    };

    pub trait PcmInterface<T: PcmFormat + ?Sized> {
        fn new_zeroed<F: PcmFormat + ?Sized>(
            frames: u64,
            channels: u32,
        ) -> MaResult<SampleBuffer<T::StorageUnit>>;
        fn truncate_to_frames_read<F: PcmFormat + ?Sized>(
            storage: &mut SampleBuffer<F::StorageUnit>,
            frames_read: u64,
        ) -> MaResult<()>;
        fn storage_to_pcm<F: PcmFormat + ?Sized>(
            storage: SampleBuffer<T::StorageUnit>,
        ) -> MaResult<SampleBuffer<T::PcmUnit>>;
        fn pcm_to_storage(pcm: SampleBuffer<T::PcmUnit>) -> MaResult<SampleBuffer<T::StorageUnit>>;
    }

    pub struct PcmU8Provider;
    pub struct PcmI16Provider;
    pub struct PcmI32Provider;
    pub struct PcmS24Provider;
    pub struct PcmS24PackedProvider;
    pub struct PcmF32Provider;

    impl PcmInterface<u8> for PcmU8Provider {
        fn new_zeroed<F: PcmFormat + ?Sized>(
            frames: u64,
            channels: u32,
        ) -> MaResult<SampleBuffer<u8>> {
            let len = get_len(frames, channels, F::STORAGE_UNITS_PER_SAMPLE)?;
            let data = vec![0u8; len];
            Ok(SampleBuffer::new(data, channels))
        }

        fn truncate_to_frames_read<F: PcmFormat + ?Sized>(
            storage: &mut SampleBuffer<F::StorageUnit>,
            frames_read: u64,
        ) -> MaResult<()> {
            let len = get_len(frames_read, storage.channels(), F::STORAGE_UNITS_PER_SAMPLE)?;
            storage.truncate(len);
            Ok(())
        }

        fn storage_to_pcm<F: PcmFormat + ?Sized>(
            storage: SampleBuffer<u8>,
        ) -> MaResult<SampleBuffer<u8>> {
            Ok(storage)
        }

        fn pcm_to_storage(pcm: SampleBuffer<u8>) -> MaResult<SampleBuffer<u8>> {
            Ok(pcm)
        }
    }

    impl PcmInterface<i16> for PcmI16Provider {
        fn new_zeroed<F: PcmFormat + ?Sized>(
            frames: u64,
            channels: u32,
        ) -> MaResult<SampleBuffer<i16>> {
            let len = get_len(frames, channels, F::STORAGE_UNITS_PER_SAMPLE)?;
            let data = vec![0i16; len];
            Ok(SampleBuffer::new(data, channels))
        }

        fn truncate_to_frames_read<F: PcmFormat + ?Sized>(
            storage: &mut SampleBuffer<F::StorageUnit>,
            frames_read: u64,
        ) -> MaResult<()> {
            let len = get_len(frames_read, storage.channels(), F::STORAGE_UNITS_PER_SAMPLE)?;
            storage.truncate(len);
            Ok(())
        }

        fn storage_to_pcm<F: PcmFormat + ?Sized>(
            storage: SampleBuffer<i16>,
        ) -> MaResult<SampleBuffer<i16>> {
            Ok(storage)
        }

        fn pcm_to_storage(pcm: SampleBuffer<i16>) -> MaResult<SampleBuffer<i16>> {
            Ok(pcm)
        }
    }

    impl PcmInterface<S24> for PcmS24Provider {
        fn new_zeroed<F: PcmFormat + ?Sized>(
            frames: u64,
            channels: u32,
        ) -> MaResult<SampleBuffer<u8>> {
            let len = get_len(frames, channels, F::STORAGE_UNITS_PER_SAMPLE)?;
            let data = vec![0u8; len];
            Ok(SampleBuffer::new(data, channels))
        }

        fn truncate_to_frames_read<F: PcmFormat + ?Sized>(
            storage: &mut SampleBuffer<F::StorageUnit>,
            frames_read: u64,
        ) -> MaResult<()> {
            let len = get_len(frames_read, storage.channels(), F::STORAGE_UNITS_PER_SAMPLE)?;
            storage.truncate(len);
            Ok(())
        }

        fn storage_to_pcm<F: PcmFormat + ?Sized>(
            storage: SampleBuffer<u8>,
        ) -> MaResult<SampleBuffer<i32>> {
            let total_items = storage.as_slice().len();

            debug_assert!(total_items % 3 == 0);
            if total_items % 3 != 0 {
                // return
                return Err(crate::MaudioError::new_ma_error(
                    ErrorKinds::InvalidPackedSampleSize {
                        bytes_per_sample: 3,
                        actual_len: total_items,
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

        fn pcm_to_storage(pcm: SampleBuffer<i32>) -> MaResult<SampleBuffer<u8>> {
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
                    return Err(crate::MaudioError::new_ma_error(ErrorKinds::S24OverFlow));
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

    impl PcmInterface<S24Packed> for PcmS24PackedProvider {
        fn new_zeroed<F: PcmFormat + ?Sized>(
            frames: u64,
            channels: u32,
        ) -> MaResult<SampleBuffer<u8>> {
            let len = get_len(frames, channels, F::STORAGE_UNITS_PER_SAMPLE)?;
            let data = vec![0u8; len];
            Ok(SampleBuffer::new(data, channels))
        }

        fn truncate_to_frames_read<F: PcmFormat + ?Sized>(
            storage: &mut SampleBuffer<F::StorageUnit>,
            frames_read: u64,
        ) -> MaResult<()> {
            let len = get_len(frames_read, storage.channels(), F::STORAGE_UNITS_PER_SAMPLE)?;
            storage.truncate(len);
            Ok(())
        }

        fn storage_to_pcm<F: PcmFormat + ?Sized>(
            storage: SampleBuffer<u8>,
        ) -> MaResult<SampleBuffer<u8>> {
            Ok(storage)
        }

        fn pcm_to_storage(pcm: SampleBuffer<u8>) -> MaResult<SampleBuffer<u8>> {
            Ok(pcm)
        }
    }

    impl PcmInterface<i32> for PcmI32Provider {
        fn new_zeroed<F: PcmFormat + ?Sized>(
            frames: u64,
            channels: u32,
        ) -> MaResult<SampleBuffer<i32>> {
            let len = get_len(frames, channels, F::STORAGE_UNITS_PER_SAMPLE)?;
            let data = vec![0i32; len];
            Ok(SampleBuffer::new(data, channels))
        }

        fn truncate_to_frames_read<F: PcmFormat + ?Sized>(
            storage: &mut SampleBuffer<F::StorageUnit>,
            frames_read: u64,
        ) -> MaResult<()> {
            let len = get_len(frames_read, storage.channels(), F::STORAGE_UNITS_PER_SAMPLE)?;
            storage.truncate(len);
            Ok(())
        }

        fn storage_to_pcm<F: PcmFormat + ?Sized>(
            storage: SampleBuffer<i32>,
        ) -> MaResult<SampleBuffer<i32>> {
            Ok(storage)
        }

        fn pcm_to_storage(pcm: SampleBuffer<i32>) -> MaResult<SampleBuffer<i32>> {
            Ok(pcm)
        }
    }

    impl PcmInterface<f32> for PcmF32Provider {
        fn new_zeroed<F: PcmFormat + ?Sized>(
            frames: u64,
            channels: u32,
        ) -> MaResult<SampleBuffer<f32>> {
            let len = get_len(frames, channels, F::STORAGE_UNITS_PER_SAMPLE)?;
            let data = vec![0f32; len];
            Ok(SampleBuffer::new(data, channels))
        }

        fn truncate_to_frames_read<F: PcmFormat + ?Sized>(
            storage: &mut SampleBuffer<F::StorageUnit>,
            frames_read: u64,
        ) -> MaResult<()> {
            let len = get_len(frames_read, storage.channels(), F::STORAGE_UNITS_PER_SAMPLE)?;
            storage.truncate(len);
            Ok(())
        }

        fn storage_to_pcm<F: PcmFormat + ?Sized>(
            storage: SampleBuffer<f32>,
        ) -> MaResult<SampleBuffer<f32>> {
            Ok(storage)
        }

        fn pcm_to_storage(pcm: SampleBuffer<f32>) -> MaResult<SampleBuffer<f32>> {
            Ok(pcm)
        }
    }
}

pub trait PcmFormat {
    type __PcmFramesProvider: private_pcm::PcmInterface<Self>;

    // Format of the input and output from/to the user
    /// The miniaudio samples format (u8, i16, i32, S24 or f32)
    type PcmUnit: Default + Copy;
    // Format used by miniaudio
    type StorageUnit: Default + Copy;
    const BYTES_PER_PCM_UNIT: usize;
    // How many units per frame (not bytes). Example: S24Packed: 3 units. S24: 1 unit
    const STORAGE_UNITS_PER_SAMPLE: usize;
}

impl PcmFormat for u8 {
    type __PcmFramesProvider = private_pcm::PcmU8Provider;

    type PcmUnit = u8;
    type StorageUnit = Self::PcmUnit;
    const BYTES_PER_PCM_UNIT: usize = size_of::<Self::PcmUnit>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 1;
}

impl PcmFormat for i16 {
    type __PcmFramesProvider = private_pcm::PcmI16Provider;

    type PcmUnit = i16;
    type StorageUnit = Self::PcmUnit;
    const BYTES_PER_PCM_UNIT: usize = size_of::<Self::PcmUnit>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 1;
}

impl PcmFormat for S24Packed {
    type __PcmFramesProvider = private_pcm::PcmS24PackedProvider;

    type PcmUnit = u8;
    type StorageUnit = Self::PcmUnit;
    const BYTES_PER_PCM_UNIT: usize = size_of::<Self::PcmUnit>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 3;
}

impl PcmFormat for S24 {
    type __PcmFramesProvider = private_pcm::PcmS24Provider;

    type PcmUnit = i32;
    type StorageUnit = u8;
    const BYTES_PER_PCM_UNIT: usize = size_of::<Self::PcmUnit>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 3;
}

impl PcmFormat for i32 {
    type __PcmFramesProvider = private_pcm::PcmI32Provider;

    type PcmUnit = i32;
    type StorageUnit = Self::PcmUnit;
    const BYTES_PER_PCM_UNIT: usize = size_of::<Self::PcmUnit>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 1;
}

impl PcmFormat for f32 {
    type __PcmFramesProvider = private_pcm::PcmF32Provider;
    type PcmUnit = f32;
    type StorageUnit = Self::PcmUnit;
    const BYTES_PER_PCM_UNIT: usize = size_of::<Self::PcmUnit>();
    const STORAGE_UNITS_PER_SAMPLE: usize = 1;
}
