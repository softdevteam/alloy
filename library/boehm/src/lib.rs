#![no_std]
#![feature(allocator_api)]
#![feature(nonnull_slice_from_raw_parts)]

use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    ptr::NonNull,
};

mod boehm;

pub struct GcAllocator;

unsafe impl GlobalAlloc for GcAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        return boehm::GC_malloc_uncollectable(layout.size()) as *mut u8;
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _: Layout) {
        boehm::GC_free(ptr);
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, _: Layout, new_size: usize) -> *mut u8 {
        boehm::GC_realloc(ptr, new_size) as *mut u8
    }

    #[inline]
    unsafe fn alloc_precise(
        &self,
        _layout: Layout,
        _bitmap: usize,
        _bitmap_size: usize,
    ) -> *mut u8 {
        unimplemented!("Boehm does not provide an uncollectable version of this call")
    }

    #[inline]
    fn alloc_conservative(&self, layout: Layout) -> *mut u8 {
        unsafe { boehm::GC_malloc_uncollectable(layout.size()) as *mut u8 }
    }

    #[inline]
    unsafe fn alloc_untraceable(&self, layout: Layout) -> *mut u8 {
        boehm::GC_malloc_atomic_uncollectable(layout.size()) as *mut u8
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

    #[inline]
    fn alloc_untraceable(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe {
            let ptr = boehm::GC_malloc_atomic(layout.size()) as *mut u8;
            let ptr = NonNull::new_unchecked(ptr);
            Ok(NonNull::slice_from_raw_parts(ptr, layout.size()))
        }
    }

    #[inline]
    fn alloc_conservative(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe {
            let ptr = boehm::GC_malloc(layout.size()) as *mut u8;
            let ptr = NonNull::new_unchecked(ptr);
            Ok(NonNull::slice_from_raw_parts(ptr, layout.size()))
        }
    }

    #[inline]
    fn alloc_precise(
        &self,
        layout: Layout,
        bitmap: usize,
        bitmap_size: usize,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe {
            let gc_descr = boehm::GC_make_descriptor(&bitmap as *const usize, bitmap_size);
            let ptr = boehm::GC_malloc_explicitly_typed(layout.size(), gc_descr);
            let ptr = NonNull::new_unchecked(ptr);
            Ok(NonNull::slice_from_raw_parts(ptr, layout.size()))
        }
    }
}

impl GcAllocator {
    pub fn force_gc() {
        unsafe { boehm::GC_gcollect() }
    }

    pub unsafe fn register_finalizer(
        &self,
        obj: *mut u8,
        finalizer: Option<unsafe extern "C" fn(*mut u8, *mut u8)>,
        client_data: *mut u8,
        old_finalizer: *mut extern "C" fn(*mut u8, *mut u8),
        old_client_data: *mut *mut u8,
    ) {
        boehm::GC_register_finalizer_no_order(
            obj,
            finalizer,
            client_data,
            old_finalizer,
            old_client_data,
        )
    }

    pub fn unregister_finalizer(&self, gcbox: *mut u8) {
        unsafe {
            boehm::GC_register_finalizer(
                gcbox,
                None,
                ::core::ptr::null_mut(),
                ::core::ptr::null_mut(),
                ::core::ptr::null_mut(),
            );
        }
    }

    pub fn get_stats() -> GcStats {
        let mut ps = boehm::ProfileStats::default();
        unsafe {
            boehm::GC_get_prof_stats(
                &mut ps as *mut boehm::ProfileStats,
                core::mem::size_of::<boehm::ProfileStats>(),
            );
        }
        let total_gc_time = unsafe { boehm::GC_get_full_gc_total_time() };

        GcStats {
            total_gc_time,
            num_collections: ps.gc_no,
            total_freed: ps.bytes_reclaimed_since_gc,
            total_alloced: ps.bytes_allocd_since_gc,
        }
    }

    pub fn init() {
        unsafe { boehm::GC_init() }
    }

    /// Returns true if thread was successfully registered.
    pub unsafe fn register_thread(stack_base: *mut u8) -> bool {
        boehm::GC_register_my_thread(stack_base) == 0
    }

    /// Returns true if thread was successfully unregistered.
    pub unsafe fn unregister_thread() -> bool {
        boehm::GC_unregister_my_thread() == 0
    }

    pub fn thread_registered() -> bool {
        unsafe { boehm::GC_thread_is_registered() != 0 }
    }

    pub fn allow_register_threads() {
        unsafe { boehm::GC_allow_register_threads() }
    }
}

#[derive(Debug)]
pub struct GcStats {
    total_gc_time: usize, // In milliseconds.
    num_collections: usize,
    total_freed: usize,   // In bytes
    total_alloced: usize, // In bytes
}
