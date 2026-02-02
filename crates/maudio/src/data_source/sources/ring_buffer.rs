use std::{
    cell::Cell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use maudio_sys::ffi as sys;

use crate::{engine::AllocationCallbacks, MaResult, MaudioError};

/// Creates a single-producer, single-consumer ring buffer.
///
/// The ring buffer operates on items whose size is defined at creation time,
/// and is compatible with all miniaudio-supported sample formats.
///
/// This is not a PCM ring buffer.
pub struct RingBuffer {}

pub(crate) struct RbInner {
    inner: *mut sys::ma_rb,
    alloc_cb: Option<Arc<AllocationCallbacks>>,
}

// Needed for Arc<RbInner> to be Send
unsafe impl Send for RbInner {}
unsafe impl Sync for RbInner {}

// public init functions and private abstractions for Send and Recv
impl RingBuffer {
    fn new_inner(size: usize) -> MaResult<Arc<RbInner>> {
        let inner = rb_ffi::rb_new_raw(size, 1, 0, None, None)?;
        Ok(Arc::new(RbInner {
            inner,
            alloc_cb: None,
        }))
    }

    fn new_inner_ex(size: usize, count: usize, stride: usize) -> MaResult<Arc<RbInner>> {
        let inner = rb_ffi::rb_new_raw(size, count, stride, None, None)?;
        Ok(Arc::new(RbInner {
            inner,
            alloc_cb: None,
        }))
    }

    pub fn new_u8(size: usize) -> MaResult<(RbSendU8, RbRecvU8)> {
        let inner = Self::new_inner(size)?;
        Ok((
            RbSendU8 {
                inner: inner.clone(),
                _not_sync: PhantomData,
            },
            RbRecvU8 {
                inner,
                _not_sync: PhantomData,
            },
        ))
    }

    pub fn new_i16(size: usize) -> MaResult<(RbSendI16, RbRecvI16)> {
        let inner = Self::new_inner(size)?;
        Ok((
            RbSendI16 {
                inner: inner.clone(),
                _not_sync: PhantomData,
            },
            RbRecvI16 {
                inner,
                _not_sync: PhantomData,
            },
        ))
    }

    pub fn new_i32(size: usize) -> MaResult<(RbSendI32, RbRecvI32)> {
        let inner = Self::new_inner(size)?;
        Ok((
            RbSendI32 {
                inner: inner.clone(),
                _not_sync: PhantomData,
            },
            RbRecvI32 {
                inner,
                _not_sync: PhantomData,
            },
        ))
    }

    pub fn new_s24(size: usize) -> MaResult<(RbSendS24, RbRecvS24)> {
        let inner = Self::new_inner(size)?;
        Ok((
            RbSendS24 {
                inner: inner.clone(),
                _not_sync: PhantomData,
            },
            RbRecvS24 {
                inner,
                _not_sync: PhantomData,
            },
        ))
    }

    pub fn new_f32(size: usize) -> MaResult<(RbSendF32, RbRecvF32)> {
        let inner = Self::new_inner(size)?;
        Ok((
            RbSendF32 {
                inner: inner.clone(),
                _not_sync: PhantomData,
            },
            RbRecvF32 {
                inner,
                _not_sync: PhantomData,
            },
        ))
    }

    fn acquire_read_internal<R: AsRbPtr + RbReadOwner, T>(
        rb: &mut R,
        desired_items: usize,
        item_size: u32,
    ) -> MaResult<RbReadGuard<'_, T>> {
        let desired_bytes = desired_items
            .checked_mul(item_size as usize)
            .ok_or(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS))?;
        let (ptr, avail_bytes) = rb_ffi::ma_rb_acquire_read(rb, desired_bytes)?;
        Ok(RbReadGuard {
            owner: rb,
            ptr,
            avail_bytes,
            committed: false,
            _pd: PhantomData,
        })
    }

    #[inline]
    fn available_read_internal<R: AsRbPtr + ?Sized>(rb: &R, item_size: u32) -> u32 {
        let bytes = rb_ffi::ma_rb_available_read(rb);
        debug_assert!(bytes % item_size == 0);
        bytes / item_size
    }

    fn read_internal<R>(rb: &mut R, dst: &mut [R::Item]) -> MaResult<usize>
    where
        R: AsRbPtr + RbRead + ?Sized,
        R::Item: Copy,
    {
        let g = rb.acquire_read(dst.len())?;
        let n = dst.len().min(g.available_items());

        dst[..n].copy_from_slice(&g[..n]);

        g.commit_items(n)?;
        Ok(n)
    }

    fn read_exact_internal<R>(rb: &mut R, dst: &mut [R::Item]) -> MaResult<()>
    where
        R: AsRbPtr + RbRead + ?Sized,
        R::Item: Copy,
    {
        let g = rb.acquire_read(dst.len())?;

        if g.available_items() < dst.len() {
            return Err(MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_OPERATION,
            ));
        }

        dst.copy_from_slice(&g[..dst.len()]);

        g.commit_items(dst.len())?;
        Ok(())
    }

    fn read_with_internal<R, F>(rb: &mut R, desired_items: usize, f: F) -> MaResult<usize>
    where
        R: AsRbPtr + RbRead + ?Sized,
        R::Item: Copy,
        F: FnOnce(&[R::Item]) -> usize,
    {
        let g = rb.acquire_read(desired_items)?;
        let avail = g.available_items();

        let items_read = f(g.as_slice());
        // The closure should not return a value greater than the available items
        debug_assert!(items_read <= avail);

        if items_read > avail {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        g.commit_items(items_read)?;
        Ok(items_read)
    }

    fn acquire_write_internal<R: AsRbPtr + RbWriteOwner, T>(
        rb: &mut R,
        desired_items: usize,
        item_size: u32,
    ) -> MaResult<RbWriteGuard<'_, T>> {
        let desired_bytes = desired_items
            .checked_mul(item_size as usize)
            .ok_or(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS))?;
        let (ptr, cap_bytes) = rb_ffi::ma_rb_acquire_write(rb, desired_bytes)?;

        Ok(RbWriteGuard {
            owner: rb,
            ptr,
            cap_bytes,
            committed: false,
            _pd: PhantomData,
        })
    }

    #[inline]
    fn available_write_internal<R: AsRbPtr + ?Sized>(rb: &R, item_size: u32) -> u32 {
        let bytes = rb_ffi::ma_rb_available_write(rb);
        debug_assert!(bytes % item_size == 0);
        bytes / item_size
    }

    fn write_internal<R>(rb: &mut R, src: &[R::Item]) -> MaResult<usize>
    where
        R: AsRbPtr + RbWrite + ?Sized,
        R::Item: Copy,
    {
        let mut g = rb.acquire_write(src.len())?;
        let dst = g.as_slice_mut();

        let n = dst.len().min(src.len());
        dst[..n].copy_from_slice(&src[..n]);

        g.commit_items(n)?;
        Ok(n)
    }

    fn write_exact_internal<R>(rb: &mut R, src: &[R::Item]) -> MaResult<()>
    where
        R: AsRbPtr + RbWrite + ?Sized,
        R::Item: Copy,
    {
        let mut g = rb.acquire_write(src.len())?;
        let dst = g.as_slice_mut();

        if dst.len() < src.len() {
            return Err(MaudioError::from_ma_result(
                sys::ma_result_MA_INVALID_OPERATION,
            ));
        }

        dst.copy_from_slice(src);
        g.commit_items(src.len())?;
        Ok(())
    }

    fn write_with_internal<R, F>(rb: &mut R, desired_items: usize, f: F) -> MaResult<usize>
    where
        R: AsRbPtr + RbWrite + ?Sized,
        R::Item: Copy,
        F: FnOnce(&mut [R::Item]) -> usize,
    {
        let mut g = rb.acquire_write(desired_items)?;
        let cap = g.capacity_items();

        let written = f(g.as_slice_mut());
        // The closure should not return a value greater than the length of the provided slice.
        debug_assert!(written <= cap);

        if written > cap {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        g.commit_items(written)?;
        Ok(written)
    }
}

/// Ring buffer writing handle for `u8` format
pub struct RbSendU8 {
    inner: Arc<RbInner>,
    _not_sync: PhantomData<Cell<()>>,
}

/// Ring buffer reading handle for `u8` format
pub struct RbRecvU8 {
    inner: Arc<RbInner>,
    _not_sync: PhantomData<Cell<()>>,
}

/// Ring buffer writing handle for `i16` format
pub struct RbSendI16 {
    inner: Arc<RbInner>,
    _not_sync: PhantomData<Cell<()>>,
}

/// Ring buffer reading handle for `i16` format
pub struct RbRecvI16 {
    inner: Arc<RbInner>,
    _not_sync: PhantomData<Cell<()>>,
}

/// Ring buffer writing handle for `i32` format
pub struct RbSendI32 {
    inner: Arc<RbInner>,
    _not_sync: PhantomData<Cell<()>>,
}

/// Ring buffer reading handle for `i32` format
pub struct RbRecvI32 {
    inner: Arc<RbInner>,
    _not_sync: PhantomData<Cell<()>>,
}

/// Ring buffer writing handle for `S24` format
pub struct RbSendS24 {
    inner: Arc<RbInner>,
    _not_sync: PhantomData<Cell<()>>,
}

/// Ring buffer reading handle for `S24` format
pub struct RbRecvS24 {
    inner: Arc<RbInner>,
    _not_sync: PhantomData<Cell<()>>,
}

/// Ring buffer writing handle for `f32` format
pub struct RbSendF32 {
    inner: Arc<RbInner>,
    _not_sync: PhantomData<Cell<()>>,
}

/// Ring buffer reading handle for `f32` format
pub struct RbRecvF32 {
    inner: Arc<RbInner>,
    _not_sync: PhantomData<Cell<()>>,
}

/// Guard providing temporary read access to a section of the ring buffer
pub struct RbReadGuard<'a, T> {
    owner: &'a mut dyn RbReadOwner,
    ptr: *const core::ffi::c_void,
    avail_bytes: usize,
    committed: bool,
    _pd: PhantomData<&'a [T]>, // T is guaranteed to be sized
}

impl<'a, T> RbReadGuard<'a, T> {
    pub fn available_items(&self) -> usize {
        // If there are bytes available, the pointer must be non-null
        debug_assert!(self.avail_bytes == 0 || !self.ptr.is_null());
        // Available byte count must be an exact multiple of T
        debug_assert_eq!(self.avail_bytes % core::mem::size_of::<T>(), 0);
        // Pointer must be aligned for T when data is present
        debug_assert!(
            self.avail_bytes == 0 || (self.ptr as usize) % core::mem::align_of::<T>() == 0
        );
        self.avail_bytes / core::mem::size_of::<T>()
    }

    pub fn as_slice(&self) -> &[T] {
        let n = self.available_items();
        // Non-zero slice length requires a valid pointer
        debug_assert!(n == 0 || !self.ptr.is_null());
        // Byte count must match whole T items
        debug_assert_eq!(self.avail_bytes % core::mem::size_of::<T>(), 0);
        // Pointer must satisfy T's alignment before forming &[T]
        debug_assert!(n == 0 || (self.ptr as usize) % core::mem::align_of::<T>() == 0);
        // SAFETY:
        // miniaudio ensure buffer is alligned to MA_SIMD_ALIGNMENT
        // When (and if) allowing users to pass in pre-alloc buffers,
        // alignment needs to be checked in release builds
        unsafe { core::slice::from_raw_parts(self.ptr as *const T, n) }
    }

    pub fn commit_items(mut self, items: usize) -> MaResult<()> {
        let bytes = items
            .checked_mul(core::mem::size_of::<T>())
            .ok_or(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS))?;
        debug_assert!(bytes <= self.avail_bytes);

        self.owner.commit_read_bytes(bytes)?;
        self.committed = true;
        Ok(())
    }
}

impl<'a, T> Deref for RbReadGuard<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> Drop for RbReadGuard<'_, T> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.owner.commit_read_bytes(0);
        }
    }
}

trait RbReadOwner {
    fn commit_read_bytes(&mut self, bytes: usize) -> MaResult<()>;
}

impl RbReadOwner for RbRecvU8 {
    fn commit_read_bytes(&mut self, bytes: usize) -> MaResult<()> {
        rb_ffi::ma_rb_commit_read(self, bytes)
    }
}

impl RbReadOwner for RbRecvI16 {
    fn commit_read_bytes(&mut self, bytes: usize) -> MaResult<()> {
        rb_ffi::ma_rb_commit_read(self, bytes)
    }
}

impl RbReadOwner for RbRecvI32 {
    fn commit_read_bytes(&mut self, bytes: usize) -> MaResult<()> {
        rb_ffi::ma_rb_commit_read(self, bytes)
    }
}

impl RbReadOwner for RbRecvS24 {
    fn commit_read_bytes(&mut self, bytes: usize) -> MaResult<()> {
        rb_ffi::ma_rb_commit_read(self, bytes)
    }
}

impl RbReadOwner for RbRecvF32 {
    fn commit_read_bytes(&mut self, bytes: usize) -> MaResult<()> {
        rb_ffi::ma_rb_commit_read(self, bytes)
    }
}

/// Guard providing temporary write access to a section of the ring buffer
pub struct RbWriteGuard<'a, T> {
    owner: &'a mut dyn RbWriteOwner,
    ptr: *mut core::ffi::c_void,
    cap_bytes: usize,
    committed: bool,
    _pd: PhantomData<&'a mut [T]>, // T is guaranteed to be sized
}

impl<'a, T> RbWriteGuard<'a, T> {
    pub fn capacity_items(&self) -> usize {
        // If there is writable capacity, the pointer must be non-null
        debug_assert!(self.cap_bytes == 0 || !self.ptr.is_null());
        // Capacity in bytes must represent whole T items
        debug_assert_eq!(self.cap_bytes % core::mem::size_of::<T>(), 0);
        // Pointer must be aligned for T when writable data is present
        debug_assert!(self.cap_bytes == 0 || (self.ptr as usize) % core::mem::align_of::<T>() == 0);
        self.cap_bytes / core::mem::size_of::<T>()
    }

    pub fn as_slice(&self) -> &[T] {
        let n = self.capacity_items();
        // Non-zero slice length requires a valid pointer
        debug_assert!(n == 0 || !self.ptr.is_null());
        // Byte capacity must match whole T items
        debug_assert_eq!(self.cap_bytes % core::mem::size_of::<T>(), 0);
        // Pointer must satisfy T's alignment before forming &[T]
        debug_assert!(n == 0 || (self.ptr as usize) % core::mem::align_of::<T>() == 0);
        // SAFETY:
        // miniaudio ensure buffer is alligned to MA_SIMD_ALIGNMENT
        // When (and if) allowing users to pass in pre-alloc buffers,
        // alignment needs to be checked in release builds
        unsafe { core::slice::from_raw_parts(self.ptr as *const T, n) }
    }

    pub fn as_slice_mut(&mut self) -> &mut [T] {
        let n = self.capacity_items();
        // Non-zero slice length requires a valid pointer
        debug_assert!(n == 0 || !self.ptr.is_null());
        // Byte capacity must match whole T items
        debug_assert_eq!(self.cap_bytes % core::mem::size_of::<T>(), 0);
        // Pointer must satisfy T's alignment before forming &mut [T]
        debug_assert!(n == 0 || (self.ptr as usize) % core::mem::align_of::<T>() == 0);
        // SAFETY:
        // miniaudio ensure buffer is alligned to MA_SIMD_ALIGNMENT
        // When (and if) allowing users to pass in pre-alloc buffers,
        // alignment needs to be checked in release builds
        unsafe { core::slice::from_raw_parts_mut(self.ptr as *mut T, n) }
    }

    pub fn commit_items(mut self, items: usize) -> MaResult<()> {
        let bytes = items
            .checked_mul(core::mem::size_of::<T>())
            .ok_or(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS))?;
        debug_assert!(bytes <= self.cap_bytes);

        self.owner.commit_write_bytes(bytes)?;
        self.committed = true;
        Ok(())
    }
}

impl<'a, T> Deref for RbWriteGuard<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a, T> DerefMut for RbWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_slice_mut()
    }
}

impl<T> Drop for RbWriteGuard<'_, T> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.owner.commit_write_bytes(0);
        }
    }
}

trait RbWriteOwner {
    fn commit_write_bytes(&mut self, bytes: usize) -> MaResult<()>;
}

impl RbWriteOwner for RbSendU8 {
    fn commit_write_bytes(&mut self, bytes: usize) -> MaResult<()> {
        rb_ffi::ma_rb_commit_write(self, bytes)
    }
}

impl RbWriteOwner for RbSendI16 {
    fn commit_write_bytes(&mut self, bytes: usize) -> MaResult<()> {
        rb_ffi::ma_rb_commit_write(self, bytes)
    }
}

impl RbWriteOwner for RbSendI32 {
    fn commit_write_bytes(&mut self, bytes: usize) -> MaResult<()> {
        rb_ffi::ma_rb_commit_write(self, bytes)
    }
}

impl RbWriteOwner for RbSendS24 {
    fn commit_write_bytes(&mut self, bytes: usize) -> MaResult<()> {
        rb_ffi::ma_rb_commit_write(self, bytes)
    }
}

impl RbWriteOwner for RbSendF32 {
    fn commit_write_bytes(&mut self, bytes: usize) -> MaResult<()> {
        rb_ffi::ma_rb_commit_write(self, bytes)
    }
}

// RbRead and RbWrite keep public API for Send and Recv, and separate read() and write() API
pub trait RbRead: AsRbPtr {
    /// One of the miniaudio formats: u8, i16, i32, s24 or f32
    type Item: Copy;
    const ITEM_SIZE: u32;

    /// Reads as many items as are available and will fit into `dst`.
    ///
    /// Returns the number of items successfully read.
    fn read(&mut self, dst: &mut [Self::Item]) -> MaResult<usize>;
    /// Acquires readable items and invokes `f` to consume them, returning the number of items read.
    fn read_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&[Self::Item]) -> usize;

    /// Attempts to read **exactly** `dst.len()` items into `dst` in a single operation.
    ///
    /// This method is **all-or-nothing**:
    /// - On success, exactly `dst.len()` items are read and committed.
    /// - On failure, **no items are consumed** (the read reservation is aborted).
    ///
    /// If more than `dst.len()` items are available, the excess remains in the buffer.
    ///
    /// Returns `MA_INVALID_OPERATION` if fewer than `dst.len()` items are available to read.
    fn read_exact(&mut self, dst: &mut [Self::Item]) -> MaResult<()>;
    /// Acquires read access to a region of the ring buffer.
    ///
    /// The returned guard provides access to up to `desired_items` items and
    /// must be committed to consume them.
    ///
    /// In most cases, using [`Self::read`] is simpler and preferred.
    fn acquire_read(&mut self, desired_items: usize) -> MaResult<RbReadGuard<'_, Self::Item>>;
    /// Returns the number of items currently available for reading.
    fn available_read(&self) -> u32;
    /// Returns the number of items currently available for writing.
    fn available_write(&self) -> u32;
    /// Returns the distance between the read and write pointers.
    fn pointer_distance(&self) -> i32;
    /// Returns the size, in bytes, of a single subbuffer.
    fn subbuffer_size(&self) -> usize;
    /// Returns the stride, in bytes, between consecutive subbuffers.
    fn subbuffer_stride(&self) -> usize;
    /// Returns the byte offset of the subbuffer at the given ring buffer index.
    fn subbuffer_offset(&self, rb_index: usize) -> usize;
}

impl RbRead for RbRecvU8 {
    type Item = u8;
    const ITEM_SIZE: u32 = core::mem::size_of::<Self::Item>() as u32;

    fn read(&mut self, dst: &mut [Self::Item]) -> MaResult<usize> {
        RingBuffer::read_internal(self, dst)
    }

    fn read_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&[Self::Item]) -> usize,
    {
        RingBuffer::read_with_internal(self, desired_items, f)
    }

    fn read_exact(&mut self, dst: &mut [Self::Item]) -> MaResult<()> {
        RingBuffer::read_exact_internal(self, dst)
    }

    fn acquire_read(&mut self, desired_items: usize) -> MaResult<RbReadGuard<'_, Self::Item>> {
        RingBuffer::acquire_read_internal(self, desired_items, Self::ITEM_SIZE)
    }

    fn available_read(&self) -> u32 {
        RingBuffer::available_read_internal(self, Self::ITEM_SIZE)
    }

    fn available_write(&self) -> u32 {
        RingBuffer::available_write_internal(self, Self::ITEM_SIZE)
    }

    fn pointer_distance(&self) -> i32 {
        rb_ffi::ma_rb_pointer_distance(self)
    }

    fn subbuffer_size(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_size(self)
    }

    fn subbuffer_stride(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_stride(self)
    }

    fn subbuffer_offset(&self, rb_index: usize) -> usize {
        rb_ffi::ma_rb_get_subbuffer_offset(self, rb_index)
    }
}

impl RbRead for RbRecvI16 {
    type Item = i16;
    const ITEM_SIZE: u32 = core::mem::size_of::<Self::Item>() as u32;

    fn read(&mut self, dst: &mut [Self::Item]) -> MaResult<usize> {
        RingBuffer::read_internal(self, dst)
    }

    fn read_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&[Self::Item]) -> usize,
    {
        RingBuffer::read_with_internal(self, desired_items, f)
    }

    fn read_exact(&mut self, dst: &mut [Self::Item]) -> MaResult<()> {
        RingBuffer::read_exact_internal(self, dst)
    }

    fn acquire_read(&mut self, desired_items: usize) -> MaResult<RbReadGuard<'_, Self::Item>> {
        RingBuffer::acquire_read_internal(self, desired_items, Self::ITEM_SIZE)
    }

    fn available_read(&self) -> u32 {
        RingBuffer::available_read_internal(self, Self::ITEM_SIZE)
    }

    fn available_write(&self) -> u32 {
        RingBuffer::available_write_internal(self, Self::ITEM_SIZE)
    }

    fn pointer_distance(&self) -> i32 {
        rb_ffi::ma_rb_pointer_distance(self)
    }

    fn subbuffer_size(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_size(self)
    }

    fn subbuffer_stride(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_stride(self)
    }

    fn subbuffer_offset(&self, rb_index: usize) -> usize {
        rb_ffi::ma_rb_get_subbuffer_offset(self, rb_index)
    }
}

impl RbRead for RbRecvI32 {
    type Item = i32;
    const ITEM_SIZE: u32 = core::mem::size_of::<Self::Item>() as u32;

    fn read(&mut self, dst: &mut [Self::Item]) -> MaResult<usize> {
        RingBuffer::read_internal(self, dst)
    }

    fn read_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&[Self::Item]) -> usize,
    {
        RingBuffer::read_with_internal(self, desired_items, f)
    }

    fn read_exact(&mut self, dst: &mut [Self::Item]) -> MaResult<()> {
        RingBuffer::read_exact_internal(self, dst)
    }

    fn acquire_read(&mut self, desired_items: usize) -> MaResult<RbReadGuard<'_, Self::Item>> {
        RingBuffer::acquire_read_internal(self, desired_items, Self::ITEM_SIZE)
    }

    fn available_read(&self) -> u32 {
        RingBuffer::available_read_internal(self, Self::ITEM_SIZE)
    }

    fn available_write(&self) -> u32 {
        RingBuffer::available_write_internal(self, Self::ITEM_SIZE)
    }

    fn pointer_distance(&self) -> i32 {
        rb_ffi::ma_rb_pointer_distance(self)
    }

    fn subbuffer_size(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_size(self)
    }

    fn subbuffer_stride(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_stride(self)
    }

    fn subbuffer_offset(&self, rb_index: usize) -> usize {
        rb_ffi::ma_rb_get_subbuffer_offset(self, rb_index)
    }
}

impl RbRead for RbRecvS24 {
    type Item = [u8; 3];
    const ITEM_SIZE: u32 = core::mem::size_of::<Self::Item>() as u32;

    fn read(&mut self, dst: &mut [Self::Item]) -> MaResult<usize> {
        RingBuffer::read_internal(self, dst)
    }

    fn read_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&[Self::Item]) -> usize,
    {
        RingBuffer::read_with_internal(self, desired_items, f)
    }

    fn read_exact(&mut self, dst: &mut [Self::Item]) -> MaResult<()> {
        RingBuffer::read_exact_internal(self, dst)
    }

    fn acquire_read(&mut self, desired_items: usize) -> MaResult<RbReadGuard<'_, Self::Item>> {
        RingBuffer::acquire_read_internal(self, desired_items, Self::ITEM_SIZE)
    }

    fn available_read(&self) -> u32 {
        RingBuffer::available_read_internal(self, Self::ITEM_SIZE)
    }

    fn available_write(&self) -> u32 {
        RingBuffer::available_write_internal(self, Self::ITEM_SIZE)
    }

    fn pointer_distance(&self) -> i32 {
        rb_ffi::ma_rb_pointer_distance(self)
    }

    fn subbuffer_size(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_size(self)
    }

    fn subbuffer_stride(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_stride(self)
    }

    fn subbuffer_offset(&self, rb_index: usize) -> usize {
        rb_ffi::ma_rb_get_subbuffer_offset(self, rb_index)
    }
}

impl RbRead for RbRecvF32 {
    type Item = f32;
    const ITEM_SIZE: u32 = core::mem::size_of::<Self::Item>() as u32;

    fn read(&mut self, dst: &mut [Self::Item]) -> MaResult<usize> {
        RingBuffer::read_internal(self, dst)
    }

    fn read_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&[Self::Item]) -> usize,
    {
        RingBuffer::read_with_internal(self, desired_items, f)
    }

    fn read_exact(&mut self, dst: &mut [Self::Item]) -> MaResult<()> {
        RingBuffer::read_exact_internal(self, dst)
    }

    fn acquire_read(&mut self, desired_items: usize) -> MaResult<RbReadGuard<'_, Self::Item>> {
        RingBuffer::acquire_read_internal(self, desired_items, Self::ITEM_SIZE)
    }

    fn available_read(&self) -> u32 {
        RingBuffer::available_read_internal(self, Self::ITEM_SIZE)
    }

    fn available_write(&self) -> u32 {
        RingBuffer::available_write_internal(self, Self::ITEM_SIZE)
    }

    fn pointer_distance(&self) -> i32 {
        rb_ffi::ma_rb_pointer_distance(self)
    }

    fn subbuffer_size(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_size(self)
    }

    fn subbuffer_stride(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_stride(self)
    }

    fn subbuffer_offset(&self, rb_index: usize) -> usize {
        rb_ffi::ma_rb_get_subbuffer_offset(self, rb_index)
    }
}

// RbRead and RbWrite keep public API for Send and Recv, and separate read() and write() API
pub trait RbWrite: AsRbPtr {
    /// One of the miniaudio formats: u8, i16, i32, s24 or f32
    type Item: Copy;
    const ITEM_SIZE: u32;

    /// Writes as many items from `src` as will fit into the ring buffer.
    ///
    /// Returns the number of items successfully written.
    fn write(&mut self, src: &[Self::Item]) -> MaResult<usize>;

    /// Acquires writable space and invokes `f` to fill it, returning the number of items written.
    fn write_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&mut [Self::Item]) -> usize;
    /// Attempts to write **all** items from `src` in a single operation.
    ///
    /// This method is **all-or-nothing**: on success, all items are written and committed.
    /// On failure, **no items are written** (the write reservation is aborted).
    ///
    /// Returns `MA_INVALID_OPERATION` if there is insufficient space available to write `src.len()` items.
    fn write_exact(&mut self, src: &[Self::Item]) -> MaResult<()>;
    /// Acquires writable access to a region of the ring buffer.
    ///
    /// The returned guard provides access to up to `desired_items` items and
    /// must be committed to make the write visible.
    ///
    /// For most cases, using [`Self::write`] is safer and prefered.
    fn acquire_write(&mut self, desired_items: usize) -> MaResult<RbWriteGuard<'_, Self::Item>>;
    /// Returns the number of items currently available for reading.
    fn available_read(&self) -> u32;
    /// Returns the number of items currently available for writing.
    fn available_write(&self) -> u32;
    /// Returns the distance between the read and write pointers.
    fn pointer_distance(&self) -> i32;
    /// Returns the size, in bytes, of a single subbuffer.
    fn subbuffer_size(&self) -> usize;
    /// Returns the stride, in bytes, between consecutive subbuffers.
    fn subbuffer_stride(&self) -> usize;
    /// Returns the byte offset of the subbuffer at the given ring buffer index.
    fn subbuffer_offset(&self, rb_index: usize) -> usize;
}

impl RbWrite for RbSendU8 {
    type Item = u8;
    const ITEM_SIZE: u32 = core::mem::size_of::<Self::Item>() as u32;

    fn write(&mut self, src: &[Self::Item]) -> MaResult<usize> {
        RingBuffer::write_internal(self, src)
    }

    fn write_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&mut [Self::Item]) -> usize,
    {
        RingBuffer::write_with_internal(self, desired_items, f)
    }

    fn write_exact(&mut self, src: &[Self::Item]) -> MaResult<()> {
        RingBuffer::write_exact_internal(self, src)
    }

    fn acquire_write(&mut self, desired_items: usize) -> MaResult<RbWriteGuard<'_, Self::Item>> {
        RingBuffer::acquire_write_internal(self, desired_items, Self::ITEM_SIZE)
    }

    fn available_read(&self) -> u32 {
        RingBuffer::available_read_internal(self, Self::ITEM_SIZE)
    }

    fn available_write(&self) -> u32 {
        RingBuffer::available_write_internal(self, Self::ITEM_SIZE)
    }

    fn pointer_distance(&self) -> i32 {
        rb_ffi::ma_rb_pointer_distance(self)
    }

    fn subbuffer_size(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_size(self)
    }

    fn subbuffer_stride(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_stride(self)
    }

    fn subbuffer_offset(&self, rb_index: usize) -> usize {
        rb_ffi::ma_rb_get_subbuffer_offset(self, rb_index)
    }
}

impl RbWrite for RbSendI16 {
    type Item = i16;
    const ITEM_SIZE: u32 = core::mem::size_of::<Self::Item>() as u32;

    fn write(&mut self, src: &[Self::Item]) -> MaResult<usize> {
        RingBuffer::write_internal(self, src)
    }

    fn write_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&mut [Self::Item]) -> usize,
    {
        RingBuffer::write_with_internal(self, desired_items, f)
    }

    fn write_exact(&mut self, src: &[Self::Item]) -> MaResult<()> {
        RingBuffer::write_exact_internal(self, src)
    }

    fn acquire_write(&mut self, desired_items: usize) -> MaResult<RbWriteGuard<'_, Self::Item>> {
        RingBuffer::acquire_write_internal(self, desired_items, Self::ITEM_SIZE)
    }

    fn available_read(&self) -> u32 {
        RingBuffer::available_read_internal(self, Self::ITEM_SIZE)
    }

    fn available_write(&self) -> u32 {
        RingBuffer::available_write_internal(self, Self::ITEM_SIZE)
    }

    fn pointer_distance(&self) -> i32 {
        rb_ffi::ma_rb_pointer_distance(self)
    }

    fn subbuffer_size(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_size(self)
    }

    fn subbuffer_stride(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_stride(self)
    }

    fn subbuffer_offset(&self, rb_index: usize) -> usize {
        rb_ffi::ma_rb_get_subbuffer_offset(self, rb_index)
    }
}

impl RbWrite for RbSendI32 {
    type Item = i32;
    const ITEM_SIZE: u32 = core::mem::size_of::<Self::Item>() as u32;

    fn write(&mut self, src: &[Self::Item]) -> MaResult<usize> {
        RingBuffer::write_internal(self, src)
    }

    fn write_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&mut [Self::Item]) -> usize,
    {
        RingBuffer::write_with_internal(self, desired_items, f)
    }

    fn write_exact(&mut self, src: &[Self::Item]) -> MaResult<()> {
        RingBuffer::write_exact_internal(self, src)
    }

    fn acquire_write(&mut self, desired_items: usize) -> MaResult<RbWriteGuard<'_, Self::Item>> {
        RingBuffer::acquire_write_internal(self, desired_items, Self::ITEM_SIZE)
    }

    fn available_read(&self) -> u32 {
        RingBuffer::available_read_internal(self, Self::ITEM_SIZE)
    }

    fn available_write(&self) -> u32 {
        RingBuffer::available_write_internal(self, Self::ITEM_SIZE)
    }

    fn pointer_distance(&self) -> i32 {
        rb_ffi::ma_rb_pointer_distance(self)
    }

    fn subbuffer_size(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_size(self)
    }

    fn subbuffer_stride(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_stride(self)
    }

    fn subbuffer_offset(&self, rb_index: usize) -> usize {
        rb_ffi::ma_rb_get_subbuffer_offset(self, rb_index)
    }
}

impl RbWrite for RbSendS24 {
    type Item = [u8; 3];
    const ITEM_SIZE: u32 = core::mem::size_of::<Self::Item>() as u32;

    fn write(&mut self, src: &[Self::Item]) -> MaResult<usize> {
        RingBuffer::write_internal(self, src)
    }

    fn write_exact(&mut self, src: &[Self::Item]) -> MaResult<()> {
        RingBuffer::write_exact_internal(self, src)
    }

    fn write_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&mut [Self::Item]) -> usize,
    {
        RingBuffer::write_with_internal(self, desired_items, f)
    }

    fn acquire_write(&mut self, desired_items: usize) -> MaResult<RbWriteGuard<'_, Self::Item>> {
        RingBuffer::acquire_write_internal(self, desired_items, Self::ITEM_SIZE)
    }

    fn available_read(&self) -> u32 {
        RingBuffer::available_read_internal(self, Self::ITEM_SIZE)
    }

    fn available_write(&self) -> u32 {
        RingBuffer::available_write_internal(self, Self::ITEM_SIZE)
    }

    fn pointer_distance(&self) -> i32 {
        rb_ffi::ma_rb_pointer_distance(self)
    }

    fn subbuffer_size(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_size(self)
    }

    fn subbuffer_stride(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_stride(self)
    }

    fn subbuffer_offset(&self, rb_index: usize) -> usize {
        rb_ffi::ma_rb_get_subbuffer_offset(self, rb_index)
    }
}

impl RbWrite for RbSendF32 {
    type Item = f32;
    const ITEM_SIZE: u32 = core::mem::size_of::<Self::Item>() as u32;

    fn write(&mut self, src: &[Self::Item]) -> MaResult<usize> {
        RingBuffer::write_internal(self, src)
    }

    fn write_with<F>(&mut self, desired_items: usize, f: F) -> MaResult<usize>
    where
        F: FnOnce(&mut [Self::Item]) -> usize,
    {
        RingBuffer::write_with_internal(self, desired_items, f)
    }

    fn write_exact(&mut self, src: &[Self::Item]) -> MaResult<()> {
        RingBuffer::write_exact_internal(self, src)
    }

    fn acquire_write(&mut self, desired_items: usize) -> MaResult<RbWriteGuard<'_, Self::Item>> {
        RingBuffer::acquire_write_internal(self, desired_items, Self::ITEM_SIZE)
    }

    fn available_read(&self) -> u32 {
        RingBuffer::available_read_internal(self, Self::ITEM_SIZE)
    }

    fn available_write(&self) -> u32 {
        RingBuffer::available_write_internal(self, Self::ITEM_SIZE)
    }

    fn pointer_distance(&self) -> i32 {
        rb_ffi::ma_rb_pointer_distance(self)
    }

    fn subbuffer_size(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_size(self)
    }

    fn subbuffer_stride(&self) -> usize {
        rb_ffi::ma_rb_get_subbuffer_stride(self)
    }

    fn subbuffer_offset(&self, rb_index: usize) -> usize {
        rb_ffi::ma_rb_get_subbuffer_offset(self, rb_index)
    }
}

mod private_rb {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait RbPtrProvider<T: ?Sized> {
        fn as_rb_ptr(t: &T) -> *mut sys::ma_rb;
    }

    pub struct RbSendProviderU8;
    pub struct RbrecvProviderU8;
    pub struct RbSendProviderI16;
    pub struct RbrecvProviderI16;
    pub struct RbSendProviderI32;
    pub struct RbrecvProviderI32;
    pub struct RbSendProviderS24;
    pub struct RbrecvProviderS24;
    pub struct RbSendProviderF32;
    pub struct RbrecvProviderF32;

    impl RbPtrProvider<RbSendU8> for RbSendProviderU8 {
        fn as_rb_ptr(t: &RbSendU8) -> *mut sys::ma_rb {
            t.inner.inner
        }
    }

    impl RbPtrProvider<RbRecvU8> for RbrecvProviderU8 {
        fn as_rb_ptr(t: &RbRecvU8) -> *mut sys::ma_rb {
            t.inner.inner
        }
    }

    impl RbPtrProvider<RbSendI16> for RbSendProviderI16 {
        fn as_rb_ptr(t: &RbSendI16) -> *mut sys::ma_rb {
            t.inner.inner
        }
    }

    impl RbPtrProvider<RbRecvI16> for RbrecvProviderI16 {
        fn as_rb_ptr(t: &RbRecvI16) -> *mut sys::ma_rb {
            t.inner.inner
        }
    }

    impl RbPtrProvider<RbSendI32> for RbSendProviderI32 {
        fn as_rb_ptr(t: &RbSendI32) -> *mut sys::ma_rb {
            t.inner.inner
        }
    }

    impl RbPtrProvider<RbRecvI32> for RbrecvProviderI32 {
        fn as_rb_ptr(t: &RbRecvI32) -> *mut sys::ma_rb {
            t.inner.inner
        }
    }

    impl RbPtrProvider<RbSendS24> for RbSendProviderS24 {
        fn as_rb_ptr(t: &RbSendS24) -> *mut sys::ma_rb {
            t.inner.inner
        }
    }

    impl RbPtrProvider<RbRecvS24> for RbrecvProviderS24 {
        fn as_rb_ptr(t: &RbRecvS24) -> *mut sys::ma_rb {
            t.inner.inner
        }
    }

    impl RbPtrProvider<RbSendF32> for RbSendProviderF32 {
        fn as_rb_ptr(t: &RbSendF32) -> *mut sys::ma_rb {
            t.inner.inner
        }
    }

    impl RbPtrProvider<RbRecvF32> for RbrecvProviderF32 {
        fn as_rb_ptr(t: &RbRecvF32) -> *mut sys::ma_rb {
            t.inner.inner
        }
    }

    pub fn rb_ptr<T: AsRbPtr + ?Sized>(t: &T) -> *mut sys::ma_rb {
        <T as AsRbPtr>::__PtrProvider::as_rb_ptr(t)
    }
}

#[doc(hidden)]
pub trait AsRbPtr {
    type __PtrProvider: private_rb::RbPtrProvider<Self>;
}

#[doc(hidden)]
impl AsRbPtr for RbSendU8 {
    type __PtrProvider = private_rb::RbSendProviderU8;
}

#[doc(hidden)]
impl AsRbPtr for RbRecvU8 {
    type __PtrProvider = private_rb::RbrecvProviderU8;
}

#[doc(hidden)]
impl AsRbPtr for RbSendI16 {
    type __PtrProvider = private_rb::RbSendProviderI16;
}

#[doc(hidden)]
impl AsRbPtr for RbRecvI16 {
    type __PtrProvider = private_rb::RbrecvProviderI16;
}

#[doc(hidden)]
impl AsRbPtr for RbSendI32 {
    type __PtrProvider = private_rb::RbSendProviderI32;
}

#[doc(hidden)]
impl AsRbPtr for RbRecvI32 {
    type __PtrProvider = private_rb::RbrecvProviderI32;
}

#[doc(hidden)]
impl AsRbPtr for RbSendS24 {
    type __PtrProvider = private_rb::RbSendProviderS24;
}

#[doc(hidden)]
impl AsRbPtr for RbRecvS24 {
    type __PtrProvider = private_rb::RbrecvProviderS24;
}

#[doc(hidden)]
impl AsRbPtr for RbSendF32 {
    type __PtrProvider = private_rb::RbSendProviderF32;
}

#[doc(hidden)]
impl AsRbPtr for RbRecvF32 {
    type __PtrProvider = private_rb::RbrecvProviderF32;
}

pub(crate) mod rb_ffi {
    use std::{mem::MaybeUninit, sync::Arc};

    use maudio_sys::ffi as sys;

    use crate::{
        data_source::sources::ring_buffer::{private_rb, AsRbPtr, RbInner},
        engine::AllocationCallbacks,
        MaRawResult, MaResult,
    };

    pub fn rb_new_raw(
        size_bytes: usize,
        count: usize,
        stride: usize,
        pre_alloc: Option<&mut [u8]>,
        alloc_cb: Option<Arc<AllocationCallbacks>>,
    ) -> MaResult<*mut sys::ma_rb> {
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_rb>> = Box::new(MaybeUninit::uninit());

        let (pre_alloc, alloc_cb) = match pre_alloc {
            Some(buf) => {
                if buf.len() < size_bytes {
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
                    alloc_cb.map_or(core::ptr::null(), |c| &c.inner as *const _);
                (core::ptr::null_mut(), alloc_cb)
            }
        };

        ma_rb_init_ex(
            size_bytes,
            count,
            stride,
            pre_alloc,
            alloc_cb,
            mem.as_mut_ptr(),
        )?;

        Ok(Box::into_raw(mem) as *mut sys::ma_rb)
    }

    // _ex version is used instead.
    #[inline]
    pub fn ma_rb_init(
        size: usize,
        pre_alloc: *mut core::ffi::c_void,
        alloc_cb: *const sys::ma_allocation_callbacks,
        rb: *mut sys::ma_rb,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_rb_init(size, pre_alloc, alloc_cb, rb) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_rb_init_ex(
        size: usize,
        count: usize,
        stride_bytes: usize,
        pre_alloc: *mut core::ffi::c_void,
        alloc_cb: *const sys::ma_allocation_callbacks,
        rb: *mut sys::ma_rb,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_rb_init_ex(size, count, stride_bytes, pre_alloc, alloc_cb, rb) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_rb_uninit(rb: &mut RbInner) {
        unsafe {
            sys::ma_rb_uninit(rb.inner);
        }
    }

    #[inline]
    pub fn ma_rb_reset(rb: &mut RbInner) {
        unsafe {
            sys::ma_rb_reset(rb.inner);
        }
    }
    // format agnostic
    #[inline]
    pub fn ma_rb_acquire_read<R: AsRbPtr + ?Sized>(
        rb: &mut R,
        desired_bytes: usize,
    ) -> MaResult<(*mut core::ffi::c_void, usize)> {
        let mut size = desired_bytes;
        let mut buf: *mut core::ffi::c_void = std::ptr::null_mut();
        let res = unsafe { sys::ma_rb_acquire_read(private_rb::rb_ptr(rb), &mut size, &mut buf) };
        MaRawResult::check(res)?;
        Ok((buf, size))
    }

    // advance the rb after we read from the buffer
    #[inline]
    pub fn ma_rb_commit_read<R: AsRbPtr + ?Sized>(rb: &mut R, bytes: usize) -> MaResult<()> {
        let res = unsafe { sys::ma_rb_commit_read(private_rb::rb_ptr(rb), bytes) };
        MaRawResult::check(res)
    }

    // format agnostic
    #[inline]
    pub fn ma_rb_acquire_write<R: AsRbPtr + ?Sized>(
        rb: &mut R,
        desired_bytes: usize,
    ) -> MaResult<(*mut core::ffi::c_void, usize)> {
        let mut size = desired_bytes;
        let mut buf: *mut core::ffi::c_void = std::ptr::null_mut();
        let res = unsafe { sys::ma_rb_acquire_write(private_rb::rb_ptr(rb), &mut size, &mut buf) };
        MaRawResult::check(res)?;
        Ok((buf, size))
    }

    #[inline]
    pub fn ma_rb_commit_write<R: AsRbPtr + ?Sized>(rb: &mut R, bytes: usize) -> MaResult<()> {
        let res = unsafe { sys::ma_rb_commit_write(private_rb::rb_ptr(rb), bytes) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_rb_seek_read<R: AsRbPtr + ?Sized>(rb: &mut R, off_bytes: usize) -> MaResult<()> {
        let res = unsafe { sys::ma_rb_seek_read(private_rb::rb_ptr(rb), off_bytes) };
        MaRawResult::check(res)
    }

    #[inline]
    pub fn ma_rb_seek_write<R: AsRbPtr + ?Sized>(rb: &mut R, off_bytes: usize) -> MaResult<()> {
        let res = unsafe { sys::ma_rb_seek_write(private_rb::rb_ptr(rb), off_bytes) };
        MaRawResult::check(res)
    }

    // Returns the distance between the write pointer and the read pointer.
    // Should never be negative for a correct program.
    // Will return the number of bytes that can be read before the read
    // pointer hits the write pointer.
    #[inline]
    pub fn ma_rb_pointer_distance<R: AsRbPtr + ?Sized>(rb: &R) -> i32 {
        unsafe { sys::ma_rb_pointer_distance(private_rb::rb_ptr(rb)) }
    }

    #[inline]
    pub fn ma_rb_available_read<R: AsRbPtr + ?Sized>(rb: &R) -> u32 {
        unsafe { sys::ma_rb_available_read(private_rb::rb_ptr(rb)) }
    }

    #[inline]
    pub fn ma_rb_available_write<R: AsRbPtr + ?Sized>(rb: &R) -> u32 {
        unsafe { sys::ma_rb_available_write(private_rb::rb_ptr(rb)) }
    }

    #[inline]
    pub fn ma_rb_get_subbuffer_size<R: AsRbPtr + ?Sized>(rb: &R) -> usize {
        unsafe { sys::ma_rb_get_subbuffer_size(private_rb::rb_ptr(rb)) }
    }

    #[inline]
    pub fn ma_rb_get_subbuffer_stride<R: AsRbPtr + ?Sized>(rb: &R) -> usize {
        unsafe { sys::ma_rb_get_subbuffer_stride(private_rb::rb_ptr(rb)) }
    }

    #[inline]
    pub fn ma_rb_get_subbuffer_offset<R: AsRbPtr + ?Sized>(rb: &R, rb_index: usize) -> usize {
        unsafe { sys::ma_rb_get_subbuffer_offset(private_rb::rb_ptr(rb), rb_index) }
    }

    // #[inline]
    // fn ma_rb_get_subbuffer_ptr(rb: &mut RingBuffer, rb_index: usize) {
    //     let buf: *mut core::ffi::c_void = std::ptr::null_mut();
    //     unsafe { sys::ma_rb_get_subbuffer_ptr(rb.to_raw(), rb_index, buf) };
    // }
}

impl Drop for RbInner {
    fn drop(&mut self) {
        rb_ffi::ma_rb_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

#[cfg(test)]
mod test {
    use crate::data_source::sources::ring_buffer::{RbRead, RbWrite, RingBuffer};

    #[test]
    fn test_ring_buffer_basic_init_u8() {
        let (mut send, mut recv) = RingBuffer::new_u8(128).unwrap();
        let data: [u8; 5] = [1, 2, 3, 6, 10];
        let written = send.write(&data).unwrap();
        assert_eq!(written, 5);

        let avail = recv.available_read();
        assert_eq!(avail, 5);

        let mut out = Vec::with_capacity(5);
        out.resize_with(5, || 0u8);
        let mut out = vec![0u8; 5];
        let items_read = recv.read(&mut out).unwrap();
        assert_eq!(written, items_read);
        assert_eq!(out, data);
    }
}
