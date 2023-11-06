#![allow(missing_docs)]
#![allow(unused_imports)]

#[cfg(not(test))]
#[cfg(not(no_global_oom_handling))]
use crate::boxed::Box;
#[cfg(test)]
#[cfg(not(no_global_oom_handling))]
use std::boxed::Box;

use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    any::Any,
    borrow,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    marker::{FinalizerSafe, PhantomData, Unsize},
    mem::MaybeUninit,
    ops::{CoerceUnsized, Deref, DispatchFromDyn, Receiver},
    ptr::{drop_in_place, null_mut, NonNull},
};

#[cfg(profile_gc)]
use core::sync::atomic::{self, AtomicU64};

#[cfg(not(no_global_oom_handling))]
use core::gc::ReferenceFree;

#[derive(Debug)]
pub struct GcAllocator;

unsafe impl GlobalAlloc for GcAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { boehm::GC_malloc(layout.size()) as *mut u8 }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _: Layout) {
        unsafe {
            boehm::GC_free(ptr);
        }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, _: Layout, new_size: usize) -> *mut u8 {
        unsafe { boehm::GC_realloc(ptr, new_size) as *mut u8 }
    }
}

unsafe impl Allocator for GcAllocator {
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe {
            let ptr = boehm::GC_malloc(layout.size()) as *mut u8;
            let ptr = NonNull::new_unchecked(ptr);
            Ok(NonNull::slice_from_raw_parts(ptr, layout.size()))
        }
    }

    unsafe fn deallocate(&self, _: NonNull<u8>, _: Layout) {}
}

impl GcAllocator {
    pub fn force_gc() {
        unsafe { boehm::GC_gcollect() }
    }
}

pub fn init() {
    unsafe { boehm::GC_init() }
}

/// Returns true if thread was successfully registered.
pub unsafe fn register_thread(stack_base: *mut u8) -> bool {
    unsafe { boehm::GC_register_my_thread(stack_base) == 0 }
}

/// Returns true if thread was successfully unregistered.
pub unsafe fn unregister_thread() -> bool {
    unsafe { boehm::GC_unregister_my_thread() == 0 }
}

pub fn suppress_warnings() {
    unsafe { boehm::GC_set_warn_proc(&boehm::GC_ignore_warn_proc as *const _ as *mut u8) };
}

pub fn thread_registered() -> bool {
    unsafe { boehm::GC_thread_is_registered() != 0 }
}
