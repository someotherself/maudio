//! A single producer, single consumer ring buffer for PCM frames.
//!
//! This module provides [`PcmRingBuffer`] for constructing a typed send/receive
//! pair: [`PcmRbSend<F>`] and [`PcmRbRecv<F>`]. The pair shares storage internally and is intended
//! for **one producer thread** (writer) and **one consumer thread** (reader).
//!
//! - [`PcmRbSend<F>`] writes frames/samples
//! - [`PcmRbRecv<F>`] reads frames/samples
//!
//! The endpoints are **not `Sync`** (by design). You can move each endpoint to a different
//! thread, but you must not share an endpoint between threads.
//!
//! Construction is format-specific (`new_i16`, `new_f32`, …)
//!
//! Special note for 24-bit audio:
//! - [`S24Packed`] represents miniaudio’s 24-bit **packed 3-byte** interleaved samples.
//! - [`S24`] is a convenience type for “24-bit stored in i32”.
//!
//! ## Example: SPSC usage pattern (one owner per endpoint)
//!
//! ```no_run
//! # use maudio::data_source::sources::pcm_ring_buffer::PcmRingBuffer;
//! # fn main() -> maudio::MaResult<()> {
//! let (mut tx, mut rx) = PcmRingBuffer::new_i16(2048, 1)?;
//!
//! // Move tx into producer thread, rx into consumer thread.
//! std::thread::scope(|s| {
//!     s.spawn(move || {
//!         let input = [0i16; 256];
//!         let _ = tx.write(&input);
//!     });
//!     s.spawn(move || {
//!         let mut output = [0i16; 256];
//!         let _ = rx.read(&mut output);
//!     });
//! });
//! # Ok(()) }
//! ```
use std::{
    cell::Cell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    data_source::sources::pcm_ring_buffer::private_pcm_db::{
        PcmRbPtrImplementation, PcmRbRecvProvider, PcmRbSendProvider,
    },
    engine::AllocationCallbacks,
    pcm_frames::{PcmFormat, PcmFormatInternal, S24Packed, S24},
    MaResult,
};

/// Type for creating a typed single-producer / single-consumer PCM ring buffer.
///
/// The buffer stores **interleaved** PCM samples:
/// - mono: `MMMM...`
/// - stereo: `LRLRLR...`
/// - N channels: `C0 C1 ... C(N-1) C0 C1 ...`
///
/// `size_frames` is the capacity in **frames**. One frame contains `channels` samples.
/// Total sample capacity is `size_frames * channels`.
///
/// # Contruction
/// Use the constructor matching your sample type:
/// - [`PcmRingBuffer::new_u8`]
/// - [`PcmRingBuffer::new_i16`]
/// - [`PcmRingBuffer::new_s24_packed`]
/// - [`PcmRingBuffer::new_s24`]
/// - [`PcmRingBuffer::new_i32`]
/// - [`PcmRingBuffer::new_f32`]
pub struct PcmRingBuffer {}

pub(crate) struct PcmRbInner {
    inner: *mut sys::ma_pcm_rb,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
}

// Required for Arc<PcmRbInner> to be Send
unsafe impl Send for PcmRbInner {}
unsafe impl Sync for PcmRbInner {}

/// Send (write) endpoint of a single-producer PCM ring buffer.
pub struct PcmRbSend<F: PcmFormat> {
    inner: Arc<PcmRbInner>,
    channels: usize,
    _not_sync: PhantomData<Cell<()>>,
    _marker: PhantomData<F>,
}

/// Receive (read) endpoint of a single-consumer PCM ring buffer.
pub struct PcmRbRecv<F: PcmFormat> {
    inner: Arc<PcmRbInner>,
    channels: usize,
    _not_sync: PhantomData<Cell<()>>,
    _marker: PhantomData<F>,
}

trait RbReadOwner {
    fn commit_read(&mut self, frames: u32) -> MaResult<()>;
}

impl<T: PcmFormat> RbReadOwner for PcmRbRecv<T> {
    fn commit_read(&mut self, frames: u32) -> MaResult<()> {
        pcm_rb_ffi::ma_pcm_rb_commit_read(self, frames)
    }
}

impl<F: PcmFormat> PcmRbSend<F> {
    /// Writes as many PCM samples from `src` as the buffer has capacity for.
    ///
    /// For interleaved audio writes are rounded down to whole frames.
    ///
    /// Returns the number of frames written.
    pub fn write(&mut self, src: &[F::PcmUnit]) -> MaResult<usize> {
        PcmRingBuffer::write_internal::<F>(self, src)
    }

    /// Writes as many of the `desired_frames` as the buffer has capacity for.
    ///
    /// The closure must return the number of `PCM FRAMES` written
    ///
    /// For interleaved audio writes are rounded down to whole frames.
    ///
    /// Returns the number of frames written.
    pub fn write_with<C>(&mut self, desired_frames: usize, f: C) -> MaResult<usize>
    where
        C: FnOnce(&mut [F::PcmUnit]) -> usize,
    {
        PcmRingBuffer::write_with_internal(self, desired_frames, f)
    }

    fn acquire_write(
        &mut self,
        desired_frames: u32,
    ) -> MaResult<RbWriteGuard<'_, F::StorageUnit, F>> {
        PcmRingBuffer::acquire_write_internal::<F>(self, desired_frames)
    }

    /// Advances the read pointer by `offset_frames`.
    pub fn seek_write(&mut self, offset_frames: u32) -> MaResult<()> {
        pcm_rb_ffi::ma_pcm_rb_seek_write(self, offset_frames)
    }

    /// Returns the distance between the read and write pointers.
    pub fn pointer_distance(&self) -> i32 {
        pcm_rb_ffi::ma_pcm_rb_pointer_distance(self)
    }

    /// Returns the number of frames available for reading.
    pub fn available_read(&self) -> u32 {
        pcm_rb_ffi::ma_pcm_rb_available_read(self)
    }

    /// Returns the number of frames available for writing.
    pub fn available_write(&self) -> u32 {
        pcm_rb_ffi::ma_pcm_rb_available_write(self)
    }

    /// Returns the total capacity of the buffer in frames.
    pub fn buffer_size(&self) -> u32 {
        pcm_rb_ffi::ma_pcm_rb_get_subbuffer_size(self)
    }

    /// Returns the PCM format of the buffer.
    pub fn format(&self) -> MaResult<Format> {
        pcm_rb_ffi::ma_pcm_rb_get_format(self)
    }

    /// Returns the number of channels.
    pub fn channels(&self) -> u32 {
        pcm_rb_ffi::ma_pcm_rb_get_channels(self)
    }

    /// Returns the configured sample rate.
    pub fn sample_rate(&self) -> MaResult<SampleRate> {
        pcm_rb_ffi::ma_pcm_rb_get_sample_rate(self)
    }

    /// Sets the sample rate metadata.
    pub fn set_sample_rate(&mut self, sample_rate: SampleRate) {
        pcm_rb_ffi::ma_pcm_rb_set_sample_rate(self, sample_rate);
    }
}

impl<F: PcmFormat> PcmRbRecv<F> {
    pub fn read(&mut self, dst: &mut [F::PcmUnit]) -> MaResult<usize> {
        PcmRingBuffer::read_internal(self, dst)
    }

    /// Reads as many of the `desired_frames` as are currently available.
    ///
    /// The closure must return the number of `PCM FRAMES` consumed.
    ///
    /// For interleaved audio reads are rounded down to whole frames.
    ///
    /// Returns the number of frames read.
    pub fn read_with<C>(&mut self, desired_frames: usize, f: C) -> MaResult<usize>
    where
        C: FnOnce(&[F::PcmUnit]) -> usize,
    {
        PcmRingBuffer::read_with_internal(self, desired_frames, f)
    }

    fn acquire_read(
        &mut self,
        desired_frames: u32,
    ) -> MaResult<RbReadGuard<'_, F::StorageUnit, F>> {
        PcmRingBuffer::acquire_read_internal::<F>(self, desired_frames)
    }

    /// Advances the write pointer by `offset_frames`.
    pub fn seek_read(&mut self, offset_frames: u32) -> MaResult<()> {
        pcm_rb_ffi::ma_pcm_rb_seek_read(self, offset_frames)
    }

    /// Returns the distance between the read and write pointers.
    pub fn pointer_distance(&self) -> i32 {
        pcm_rb_ffi::ma_pcm_rb_pointer_distance(self)
    }

    /// Returns the number of frames available for reading.
    pub fn available_read(&self) -> u32 {
        pcm_rb_ffi::ma_pcm_rb_available_read(self)
    }

    /// Returns the number of frames available for writing
    pub fn available_write(&self) -> u32 {
        pcm_rb_ffi::ma_pcm_rb_available_write(self)
    }

    /// Returns the total capacity of the buffer in frames.
    pub fn buffer_size(&self) -> u32 {
        pcm_rb_ffi::ma_pcm_rb_get_subbuffer_size(self)
    }

    /// Returns the PCM format of the buffer.
    pub fn format(&self) -> MaResult<Format> {
        pcm_rb_ffi::ma_pcm_rb_get_format(self)
    }

    /// Returns the number of channels.
    pub fn channels(&self) -> u32 {
        pcm_rb_ffi::ma_pcm_rb_get_channels(self)
    }

    /// Returns the configured sample rate.
    pub fn sample_rate(&self) -> MaResult<SampleRate> {
        pcm_rb_ffi::ma_pcm_rb_get_sample_rate(self)
    }

    /// Sets the sample rate metadata.
    pub fn set_sample_rate(&mut self, sample_rate: SampleRate) {
        pcm_rb_ffi::ma_pcm_rb_set_sample_rate(self, sample_rate);
    }
}

impl PcmRingBuffer {
    /// Constructs a ring-buffer for interleaved audio frames.
    pub fn new_u8(size_frames: u32, channels: u32) -> MaResult<(PcmRbSend<u8>, PcmRbRecv<u8>)> {
        let inner = Self::new_inner_ex(size_frames, Format::U8, channels, 1, 0)?;
        Self::init::<u8>(inner, channels)
    }

    /// Constructs a ring-buffer for interleaved audio frames.
    pub fn new_i16(size_frames: u32, channels: u32) -> MaResult<(PcmRbSend<i16>, PcmRbRecv<i16>)> {
        let inner = Self::new_inner_ex(size_frames, Format::S16, channels, 1, 0)?;
        Self::init::<i16>(inner, channels)
    }

    /// Constructs a ring-buffer for interleaved audio frames.
    pub fn new_s24_packed(
        size_frames: u32,
        channels: u32,
    ) -> MaResult<(PcmRbSend<S24Packed>, PcmRbRecv<S24Packed>)> {
        let inner = Self::new_inner_ex(size_frames, Format::S24, channels, 1, 0)?;
        Self::init::<S24Packed>(inner, channels)
    }

    /// Constructs a ring-buffer for interleaved audio frames.
    pub fn new_s24(size_frames: u32, channels: u32) -> MaResult<(PcmRbSend<S24>, PcmRbRecv<S24>)> {
        let inner = Self::new_inner_ex(size_frames, Format::S24, channels, 1, 0)?;
        Self::init::<S24>(inner, channels)
    }

    /// Constructs a ring-buffer for interleaved audio frames.
    pub fn new_i32(size_frames: u32, channels: u32) -> MaResult<(PcmRbSend<i32>, PcmRbRecv<i32>)> {
        let inner = Self::new_inner_ex(size_frames, Format::S24, channels, 1, 0)?;
        Self::init::<i32>(inner, channels)
    }

    /// Constructs a ring-buffer for interleaved audio frames.
    pub fn new_f32(size_frames: u32, channels: u32) -> MaResult<(PcmRbSend<f32>, PcmRbRecv<f32>)> {
        let inner = Self::new_inner_ex(size_frames, Format::F32, channels, 1, 0)?;
        Self::init::<f32>(inner, channels)
    }

    fn init<T: PcmFormat>(
        inner: Arc<PcmRbInner>,
        channels: u32,
    ) -> MaResult<(PcmRbSend<T>, PcmRbRecv<T>)> {
        Ok((
            PcmRbSend {
                inner: inner.clone(),
                channels: channels as usize,
                _not_sync: PhantomData,
                _marker: PhantomData,
            },
            PcmRbRecv {
                inner,
                channels: channels as usize,
                _not_sync: PhantomData,
                _marker: PhantomData,
            },
        ))
    }

    fn acquire_read_internal<F: PcmFormat>(
        rb: &mut PcmRbRecv<F>,
        desired_frames: u32,
    ) -> MaResult<RbReadGuard<'_, F::StorageUnit, F>> {
        let (ptr, avail_frames) = pcm_rb_ffi::ma_pcm_rb_acquire_read(rb, desired_frames)?;
        let channels = rb.channels;

        Ok(RbReadGuard {
            owner: rb,
            ptr,
            channels,
            avail_frames,
            committed: false,
            _raw_type: PhantomData,
            _pcm_format: PhantomData,
        })
    }

    fn read_internal<F>(rb: &mut PcmRbRecv<F>, dst: &mut [F::PcmUnit]) -> MaResult<usize>
    where
        F: PcmFormat,
    {
        // Must adjust frames count for S24 format and round down to a multiple of VEC_STORE_UNITS_PER_FRAME
        // The potential truncates are intentional
        let desired_frames = dst.len() / rb.channels / F::VEC_PCM_UNITS_PER_FRAME;
        let channels = rb.channels;
        let g = rb.acquire_read(desired_frames as u32)?;
        let available = g.available_frames() as usize;
        let src = g.as_slice(); // builds the slice with length of `available` (adjusted to vec items)

        let n = F::read_from_storage_internal(src, dst, available, channels)?;
        g.commit_frames(n as u32)?; // byte capacity of the rb fits inside an i32

        Ok(n)
    }

    fn read_with_internal<F, C>(
        rb: &mut PcmRbRecv<F>,
        desired_frames: usize,
        f: C,
    ) -> MaResult<usize>
    where
        F: PcmFormat,
        C: FnOnce(&[F::PcmUnit]) -> usize,
    {
        let channels = rb.channels;
        let g = rb.acquire_read(desired_frames as u32)?;
        let available = g.available_frames() as usize;
        let src = g.as_slice();

        let n = F::read_with_from_storage_internal(src, available, f, channels)?;
        g.commit_frames(n as u32)?; // byte capacity of the rb fits inside an i32
        Ok(n)
    }

    fn acquire_write_internal<F: PcmFormat>(
        rb: &mut PcmRbSend<F>,
        desired_frames: u32,
    ) -> MaResult<RbWriteGuard<'_, F::StorageUnit, F>> {
        let (ptr, cap_frames) = pcm_rb_ffi::ma_pcm_rb_acquire_write(rb, desired_frames)?;
        let channels = rb.channels;

        Ok(RbWriteGuard {
            owner: rb,
            ptr,
            cap_frames,
            channels,
            committed: false,
            _raw_type: PhantomData,
            _pcm_format: PhantomData,
        })
    }

    fn write_internal<F>(rb: &mut PcmRbSend<F>, src: &[F::PcmUnit]) -> MaResult<usize>
    where
        F: PcmFormat,
    {
        // Must adjust frames count to channels and S24 format and round down to a multiple
        // The potential truncates are intentional
        let desired_frames = src.len() / rb.channels / F::VEC_STORE_UNITS_PER_FRAME;
        let channels = rb.channels;
        let mut g: RbWriteGuard<'_, <F as PcmFormat>::StorageUnit, F> =
            rb.acquire_write(desired_frames as u32)?;
        let capacity = g.capacity_frames() as usize;
        // as_slice_mut builds the slice with length of `capacity_frames` (adjusted to vec items and channels)
        let dst = g.as_slice_mut();

        let n = F::write_to_storage_internal(dst, src, capacity, channels)?;
        g.commit_frames(n as u32)?; // byte capacity of the rb fits inside an i32
        Ok(n)
    }

    // dst uses StorageUnit -> This is the format type for miniaudio
    // param for C uses PcmUnit -> We cannot always write to miniaudio directly.
    //                          -> If StorageUnit != PcmUnit, we write to a tmp Vec<PcmUnit>
    //                          -> Then convert it to Vec<StorageUnit> and write to miniaudio
    fn write_with_internal<F, C>(
        rb: &mut PcmRbSend<F>,
        desired_frames: usize,
        f: C,
    ) -> MaResult<usize>
    where
        F: PcmFormat,
        C: FnOnce(&mut [F::PcmUnit]) -> usize,
    {
        let channels = rb.channels;
        let mut g = rb.acquire_write(desired_frames as u32)?;
        let capacity = g.capacity_frames() as usize;
        let len = capacity.min(desired_frames);
        // as_slice_mut builds the slice with length of `capacity_frames` (adjusted to vec items and channels)
        let dst = g.as_slice_mut();
        let written = F::write_with_to_storage_internal(dst, len, f, channels)?;
        g.commit_frames(written as u32)?; // byte capacity of the rb fits inside an i32
        Ok(written)
    }

    fn new_inner_ex(
        size_frames: u32,
        format: Format,
        channels: u32,
        count: u32,
        stride: u32,
    ) -> MaResult<Arc<PcmRbInner>> {
        let inner =
            pcm_rb_ffi::new_raw_ex(format, channels, size_frames, count, stride, None, None)?;
        Ok(Arc::new(PcmRbInner {
            inner,
            alloc_cb: None,
        }))
    }
}

/// Guard providing temporary read access to a section of the ring buffer
struct RbReadGuard<'a, T, F: PcmFormat> {
    owner: &'a mut dyn RbReadOwner,
    ptr: *mut core::ffi::c_void,
    avail_frames: u32,
    channels: usize,
    committed: bool,
    _raw_type: PhantomData<&'a mut [T]>, // T is guaranteed to be sized
    _pcm_format: PhantomData<F>,
}

impl<'a, T, F: PcmFormat> RbReadGuard<'a, T, F> {
    pub fn available_frames(&self) -> u32 {
        self.avail_frames
    }

    pub fn as_slice(&self) -> &[T] {
        // acquire_read returns the number of frames available.
        // It returns a byte pointer but we need to adjust for the number of vec items we need
        // rb storage is capped to i32 so this mul should be pretty safe
        let n = self.available_frames() as usize * F::VEC_STORE_UNITS_PER_FRAME * self.channels;
        // Non-zero slice length requires a valid pointer
        debug_assert!(n == 0 || !self.ptr.is_null());
        // Pointer must satisfy T's alignment before forming &[T]
        debug_assert!(n == 0 || (self.ptr as usize) % core::mem::align_of::<T>() == 0);
        // SAFETY:
        // miniaudio ensure buffer is alligned to MA_SIMD_ALIGNMENT
        // When (and if) allowing users to pass in pre-alloc buffers,
        // alignment needs to be checked in release builds
        unsafe { core::slice::from_raw_parts(self.ptr as *const T, n) }
    }

    pub fn commit_frames(mut self, frames: u32) -> MaResult<()> {
        self.owner.commit_read(frames)?;
        self.committed = true;
        Ok(())
    }
}

impl<'a, T, F: PcmFormat> Deref for RbReadGuard<'a, T, F> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, F: PcmFormat> Drop for RbReadGuard<'_, T, F> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.owner.commit_read(0);
        }
    }
}

trait RbWriteOwner {
    fn commit_write(&mut self, frames: u32) -> MaResult<()>;
}

impl<T: PcmFormat> RbWriteOwner for PcmRbSend<T> {
    fn commit_write(&mut self, frames: u32) -> MaResult<()> {
        pcm_rb_ffi::ma_pcm_rb_commit_write(self, frames)
    }
}

/// Guard providing temporary write access to a section of the ring buffer
struct RbWriteGuard<'a, T, F: PcmFormat> {
    owner: &'a mut dyn RbWriteOwner,
    ptr: *mut core::ffi::c_void,
    cap_frames: u32,
    channels: usize,
    committed: bool,
    _raw_type: PhantomData<&'a mut [T]>, // T is guaranteed to be sized
    _pcm_format: PhantomData<F>,
}

impl<'a, T, F: PcmFormat> RbWriteGuard<'a, T, F> {
    pub fn capacity_frames(&self) -> u32 {
        self.cap_frames
    }

    pub fn as_slice(&self) -> &[T] {
        // acquire_read returns the number of frames available.
        // It returns a byte pointer but we need to adjust for the number of vec items we need
        // rb storage is capped to i32 so this mul should be pretty safe
        let n = self.capacity_frames() as usize * F::VEC_STORE_UNITS_PER_FRAME * self.channels;
        // Non-zero slice length requires a valid pointer
        debug_assert!(n == 0 || !self.ptr.is_null());
        // Pointer must satisfy T's alignment before forming &[T]
        debug_assert!(n == 0 || (self.ptr as usize) % core::mem::align_of::<T>() == 0);
        // SAFETY:
        // miniaudio ensure buffer is alligned to MA_SIMD_ALIGNMENT
        // When (and if) allowing users to pass in pre-alloc buffers,
        // alignment needs to be checked in release builds
        unsafe { core::slice::from_raw_parts(self.ptr as *const T, n) }
    }

    pub fn as_slice_mut(&mut self) -> &mut [T] {
        // acquire_read returns the number of frames available.
        // It returns a byte pointer but we need to adjust for the number of vec items we need.
        // rb storage is capped to i32 so this mul should be pretty safe
        let n = self.capacity_frames() as usize * F::VEC_STORE_UNITS_PER_FRAME * self.channels;
        // Non-zero slice length requires a valid pointer
        debug_assert!(n == 0 || !self.ptr.is_null());
        // Byte capacity must match whole T items
        debug_assert_eq!(n % core::mem::size_of::<T>(), 0);
        // Pointer must satisfy T's alignment before forming &mut [T]
        debug_assert!(n == 0 || (self.ptr as usize) % core::mem::align_of::<T>() == 0);
        // SAFETY:
        // miniaudio ensure buffer is alligned to MA_SIMD_ALIGNMENT
        // When (and if) allowing users to pass in pre-alloc buffers,
        // alignment needs to be checked in release builds
        unsafe { core::slice::from_raw_parts_mut(self.ptr as *mut T, n) }
    }

    pub fn commit_frames(mut self, frames: u32) -> MaResult<()> {
        self.owner.commit_write(frames)?;
        self.committed = true;
        Ok(())
    }
}

impl<'a, T, F: PcmFormat> Deref for RbWriteGuard<'a, T, F> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a, T, F: PcmFormat> DerefMut for RbWriteGuard<'a, T, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_slice_mut()
    }
}

impl<T, F: PcmFormat> Drop for RbWriteGuard<'_, T, F> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.owner.commit_write(0);
        }
    }
}

mod private_pcm_db {
    use super::*;

    use maudio_sys::ffi as sys;

    pub trait PcmRbPtrImplementation<T: ?Sized> {
        fn as_pcm_rb_ptr(t: &T) -> *mut sys::ma_pcm_rb;
    }

    pub struct PcmRbSendProvider;
    pub struct PcmRbRecvProvider;

    impl<T: PcmFormat> PcmRbPtrImplementation<PcmRbRecv<T>> for PcmRbRecvProvider {
        fn as_pcm_rb_ptr(t: &PcmRbRecv<T>) -> *mut sys::ma_pcm_rb {
            t.inner.inner
        }
    }

    impl<T: PcmFormat> PcmRbPtrImplementation<PcmRbSend<T>> for PcmRbSendProvider {
        fn as_pcm_rb_ptr(t: &PcmRbSend<T>) -> *mut sys::ma_pcm_rb {
            t.inner.inner
        }
    }

    pub fn pcm_rb_ptr<T: AsPcmRbPtr>(t: &T) -> *mut sys::ma_pcm_rb {
        <T as AsPcmRbPtr>::__PtrProvider::as_pcm_rb_ptr(t)
    }
}

pub trait AsPcmRbPtr {
    type __PtrProvider: PcmRbPtrImplementation<Self>;
}

impl<T: PcmFormat> AsPcmRbPtr for PcmRbRecv<T> {
    type __PtrProvider = PcmRbRecvProvider;
}

impl<T: PcmFormat> AsPcmRbPtr for PcmRbSend<T> {
    type __PtrProvider = PcmRbSendProvider;
}

mod pcm_rb_ffi {
    use std::{mem::MaybeUninit, sync::Arc};

    use maudio_sys::ffi as sys;

    use crate::{
        audio::{formats::Format, sample_rate::SampleRate},
        data_source::sources::pcm_ring_buffer::{private_pcm_db, AsPcmRbPtr, PcmRbInner},
        engine::AllocationCallbacks,
        AsRawRef, MaResult, MaudioError,
    };

    pub fn new_raw_ex(
        format: Format,
        channels: u32,
        size_frames: u32,
        count: u32,
        stride: u32,
        pre_alloc: Option<&mut [u8]>,
        alloc_cb: Option<Arc<AllocationCallbacks>>,
    ) -> MaResult<*mut sys::ma_pcm_rb> {
        let mut mem: Box<MaybeUninit<sys::ma_pcm_rb>> = Box::new(MaybeUninit::uninit());

        let (pre_alloc, alloc_cb) = match pre_alloc {
            Some(buf) => {
                if buf.len() < size_frames as usize {
                    return Err(crate::MaudioError::from_ma_result(
                        sys::ma_result_MA_INVALID_ARGS,
                    ));
                }
                (
                    buf.as_mut_ptr() as *mut core::ffi::c_void,
                    core::ptr::null(),
                )
            }
            None => {
                let alloc_cb: *const sys::ma_allocation_callbacks =
                    alloc_cb.map_or(core::ptr::null(), |c| c.as_raw_ptr());
                (core::ptr::null_mut(), alloc_cb)
            }
        };

        ma_pcm_rb_init_ex(
            format.into(),
            channels,
            size_frames,
            count,
            stride,
            pre_alloc,
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        Ok(Box::into_raw(mem) as *mut sys::ma_pcm_rb)
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn ma_pcm_rb_init_ex(
        format: sys::ma_format,
        channels: u32,
        subbuffer_size_frames: u32,
        subbuffer_count: u32,
        subbuffer_stride_frames: u32,
        pre_alloc: *mut core::ffi::c_void,
        alloc_cb: *const sys::ma_allocation_callbacks,
        rb: *mut sys::ma_pcm_rb,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_pcm_rb_init_ex(
                format,
                channels,
                subbuffer_size_frames,
                subbuffer_count,
                subbuffer_stride_frames,
                pre_alloc,
                alloc_cb,
                rb,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_pcm_rb_init(
        format: sys::ma_format,
        channels: u32,
        subbuffer_size_frames: u32,
        pre_alloc: *mut core::ffi::c_void,
        alloc_cb: *const sys::ma_allocation_callbacks,
        rb: *mut sys::ma_pcm_rb,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_pcm_rb_init(
                format,
                channels,
                subbuffer_size_frames,
                pre_alloc,
                alloc_cb,
                rb,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_pcm_rb_uninit(rb: &mut PcmRbInner) {
        unsafe {
            sys::ma_pcm_rb_uninit(rb.inner);
        }
    }

    pub fn ma_pcm_rb_reset<R: AsPcmRbPtr>(rb: &mut R) {
        unsafe {
            sys::ma_pcm_rb_reset(private_pcm_db::pcm_rb_ptr(rb));
        }
    }

    #[inline]
    pub fn ma_pcm_rb_acquire_read<R: AsPcmRbPtr>(
        rb: &mut R,
        desired_frames: u32,
    ) -> MaResult<(*mut core::ffi::c_void, u32)> {
        let mut size = desired_frames;
        let mut buf: *mut core::ffi::c_void = std::ptr::null_mut();
        let res = unsafe {
            sys::ma_pcm_rb_acquire_read(private_pcm_db::pcm_rb_ptr(rb), &mut size, &mut buf)
        };
        MaudioError::check(res)?;
        Ok((buf, size))
    }

    #[inline]
    pub fn ma_pcm_rb_commit_read<R: AsPcmRbPtr>(rb: &mut R, frames: u32) -> MaResult<()> {
        let res = unsafe { sys::ma_pcm_rb_commit_read(private_pcm_db::pcm_rb_ptr(rb), frames) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_pcm_rb_acquire_write<R: AsPcmRbPtr>(
        rb: &mut R,
        desired_frames: u32,
    ) -> MaResult<(*mut core::ffi::c_void, u32)> {
        let mut size = desired_frames;
        let mut buf: *mut core::ffi::c_void = std::ptr::null_mut();
        let res = unsafe {
            sys::ma_pcm_rb_acquire_write(private_pcm_db::pcm_rb_ptr(rb), &mut size, &mut buf)
        };
        MaudioError::check(res)?;
        Ok((buf, size))
    }

    #[inline]
    pub fn ma_pcm_rb_commit_write<R: AsPcmRbPtr>(rb: &mut R, frames: u32) -> MaResult<()> {
        let res = unsafe { sys::ma_pcm_rb_commit_write(private_pcm_db::pcm_rb_ptr(rb), frames) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_pcm_rb_seek_read<R: AsPcmRbPtr>(rb: &mut R, offset: u32) -> MaResult<()> {
        let res = unsafe { sys::ma_pcm_rb_seek_read(private_pcm_db::pcm_rb_ptr(rb), offset) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_pcm_rb_seek_write<R: AsPcmRbPtr>(rb: &mut R, offset: u32) -> MaResult<()> {
        let res = unsafe { sys::ma_pcm_rb_seek_write(private_pcm_db::pcm_rb_ptr(rb), offset) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_pcm_rb_pointer_distance<R: AsPcmRbPtr>(rb: &R) -> i32 {
        unsafe { sys::ma_pcm_rb_pointer_distance(private_pcm_db::pcm_rb_ptr(rb)) }
    }

    #[inline]
    pub fn ma_pcm_rb_available_read<R: AsPcmRbPtr>(rb: &R) -> u32 {
        unsafe { sys::ma_pcm_rb_available_read(private_pcm_db::pcm_rb_ptr(rb)) }
    }

    #[inline]
    pub fn ma_pcm_rb_available_write<R: AsPcmRbPtr>(rb: &R) -> u32 {
        unsafe { sys::ma_pcm_rb_available_write(private_pcm_db::pcm_rb_ptr(rb)) }
    }

    // deinterleaved buffers not implemented yet. Create new trait for this method
    #[inline]
    pub fn ma_pcm_rb_get_subbuffer_size<R: AsPcmRbPtr>(rb: &R) -> u32 {
        unsafe { sys::ma_pcm_rb_get_subbuffer_size(private_pcm_db::pcm_rb_ptr(rb)) }
    }

    // deinterleaved buffers not implemented yet. Create new trait for this method
    #[inline]
    pub fn ma_pcm_rb_get_subbuffer_stride<R: AsPcmRbPtr>(rb: &R) -> u32 {
        unsafe { sys::ma_pcm_rb_get_subbuffer_stride(private_pcm_db::pcm_rb_ptr(rb)) }
    }

    // deinterleaved buffers not implemented yet. Create new trait for this method
    #[inline]
    pub fn ma_pcm_rb_get_subbuffer_offset<R: AsPcmRbPtr>(rb: &R, index: u32) -> u32 {
        unsafe { sys::ma_pcm_rb_get_subbuffer_offset(private_pcm_db::pcm_rb_ptr(rb), index) }
    }

    // deinterleaved buffers not implemented yet. Create new trait for this method
    #[inline]
    pub fn ma_pcm_rb_get_subbuffer_ptr<R: AsPcmRbPtr>(
        rb: &mut R,
        index: u32,
    ) -> *mut core::ffi::c_void {
        let buf = core::ptr::null_mut();
        unsafe { sys::ma_pcm_rb_get_subbuffer_ptr(private_pcm_db::pcm_rb_ptr(rb), index, buf) };
        buf
    }

    #[inline]
    pub fn ma_pcm_rb_get_format<R: AsPcmRbPtr>(rb: &R) -> MaResult<Format> {
        let res = unsafe { sys::ma_pcm_rb_get_format(private_pcm_db::pcm_rb_ptr(rb)) };
        res.try_into()
    }

    #[inline]
    pub fn ma_pcm_rb_get_channels<R: AsPcmRbPtr>(rb: &R) -> u32 {
        unsafe { sys::ma_pcm_rb_get_channels(private_pcm_db::pcm_rb_ptr(rb)) }
    }

    #[inline]
    pub fn ma_pcm_rb_get_sample_rate<R: AsPcmRbPtr>(rb: &R) -> MaResult<SampleRate> {
        let res = unsafe { sys::ma_pcm_rb_get_sample_rate(private_pcm_db::pcm_rb_ptr(rb)) };
        res.try_into()
    }

    #[inline]
    pub fn ma_pcm_rb_set_sample_rate<R: AsPcmRbPtr>(rb: &mut R, sample_rate: SampleRate) {
        unsafe {
            sys::ma_pcm_rb_set_sample_rate(private_pcm_db::pcm_rb_ptr(rb), sample_rate.into());
        }
    }
}

impl Drop for PcmRbInner {
    fn drop(&mut self) {
        pcm_rb_ffi::ma_pcm_rb_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}
