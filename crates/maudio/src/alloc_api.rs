#![allow(dead_code)]
use std::sync::OnceLock;
use std::{alloc::Layout, ffi::c_void};

use maudio_sys::ffi as sys;

use crate::AllocationCallbacks;

#[cfg(feature = "use-global-allocator")]
pub(crate) static GLOBAL_ALLOC: OnceLock<AllocationCallbacks> = OnceLock::new();

// https://github.com/manenko/cassander/blob/9c5b0f1fa1deee31759a1e424496411dce802e49/src/allocator.rs#L181

const ALIGN: usize = 16;
const META_SIZE: usize = std::mem::size_of::<Metadata>();

const _: () = assert!(META_SIZE % ALIGN == 0);
const _: () = assert!(std::mem::align_of::<Metadata>() == ALIGN);

#[repr(C, align(16))]
struct Metadata {
    size: usize,
}

#[inline]
unsafe fn pack_ptr(base: *mut u8, size: usize) -> *mut c_void {
    let metadata = Metadata { size };
    (base as *mut Metadata).write(metadata);
    base.add(META_SIZE) as *mut c_void
}

unsafe extern "C" fn ma_malloc_cb(size: usize, _user_data: *mut c_void) -> *mut c_void {
    let Ok(layout) = Layout::from_size_align(size + META_SIZE, ALIGN) else {
        return std::ptr::null_mut();
    };
    let base_ptr = std::alloc::alloc(layout);
    if base_ptr.is_null() {
        return std::ptr::null_mut();
    };
    pack_ptr(base_ptr, size)
}

unsafe extern "C" fn ma_free_cb(ptr: *mut c_void, _user_data: *mut c_void) {
    if ptr.is_null() {
        return;
    };

    let base_ptr = (ptr as *mut u8).sub(META_SIZE);
    let size = (base_ptr as *mut Metadata).read().size;

    // Layout assumed valid as it had to be created for this allocation to exist
    let layout = Layout::from_size_align_unchecked(size + META_SIZE, ALIGN);
    std::alloc::dealloc(base_ptr, layout);
}

unsafe extern "C" fn ma_realloc_cb(
    ptr: *mut c_void,
    new_size: usize,
    _user_data: *mut c_void,
) -> *mut c_void {
    if ptr.is_null() {
        return ma_malloc_cb(new_size, _user_data);
    }

    if new_size == 0 {
        ma_free_cb(ptr, _user_data);
        return std::ptr::null_mut();
    }

    let base_ptr = (ptr as *mut u8).sub(META_SIZE);
    let old_size = (base_ptr as *mut Metadata).read().size;

    // Layout assumed valid as it had to be created for this allocation to exist
    let old_layout = Layout::from_size_align_unchecked(old_size + META_SIZE, ALIGN);

    let Ok(new_layout) = Layout::from_size_align(new_size + META_SIZE, ALIGN) else {
        return std::ptr::null_mut();
    };
    let new_base_ptr = std::alloc::alloc(new_layout);
    if new_base_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let new_ptr = new_base_ptr.add(META_SIZE);
    core::ptr::copy_nonoverlapping(ptr, new_ptr as *mut c_void, old_size.min(new_size));

    // do not leak the old allocation
    std::alloc::dealloc(base_ptr, old_layout);

    // We already know that new_base_ptr is not null
    pack_ptr(new_base_ptr, new_size)
}

pub(crate) fn ma_global_allocation_callbacks() -> AllocationCallbacks {
    let cb = sys::ma_allocation_callbacks {
        pUserData: core::ptr::null_mut(),
        onMalloc: Some(ma_malloc_cb),
        onRealloc: Some(ma_realloc_cb),
        onFree: Some(ma_free_cb),
    };
    AllocationCallbacks(cb)
}
