#![allow(missing_docs)]

#[cfg(not(no_gc))]
#[allow(nonstandard_style)]
#[allow(missing_debug_implementations)]
pub mod api {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

use core::ffi::c_void;
use core::ptr::NonNull;
use core::{cmp, ptr};

#[cfg(not(no_gc))]
pub use api::*;

use crate::alloc::{AllocError, Allocator, GlobalAlloc, Layout};

pub fn init(finalizer_thread: extern "C" fn()) {
    unsafe {
        api::GC_set_finalize_on_demand(1);
        api::GC_set_finalizer_notifier(Some(finalizer_thread));
        #[cfg(feature = "gc-disable")]
        api::GC_disable();
        metrics::init();
        // The final initialization must come last.
        api::GC_init();
    }
}

// Fast-path for low alignment values
pub const MIN_ALIGN: usize = 8;

#[derive(Debug)]
pub struct GcAllocator;

unsafe impl GlobalAlloc for GcAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        metrics::increment(1, metrics::Metric::AllocationsBox);
        unsafe { gc_malloc(layout) }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { gc_free(ptr, layout) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { gc_realloc(ptr, layout, new_size) }
    }
}

#[inline]
unsafe fn gc_malloc(layout: Layout) -> *mut u8 {
    if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
        unsafe { api::GC_malloc(layout.size()) as *mut u8 }
    } else {
        let mut out = ptr::null_mut();
        // posix_memalign requires that the alignment be a multiple of `sizeof(void*)`.
        // Since these are all powers of 2, we can just use max.
        unsafe {
            let align = layout.align().max(core::mem::size_of::<usize>());
            let ret = api::GC_posix_memalign(&mut out, align, layout.size());
            if ret != 0 { ptr::null_mut() } else { out as *mut u8 }
        }
    }
}

#[inline]
unsafe fn gc_realloc(ptr: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
    if old_layout.align() <= MIN_ALIGN && old_layout.align() <= new_size {
        unsafe { api::GC_realloc(ptr as *mut c_void, new_size) as *mut u8 }
    } else {
        unsafe {
            let new_layout = Layout::from_size_align_unchecked(new_size, old_layout.align());

            let new_ptr = gc_malloc(new_layout);
            if !new_ptr.is_null() {
                let size = cmp::min(old_layout.size(), new_size);
                ptr::copy_nonoverlapping(ptr, new_ptr, size);
                gc_free(ptr, old_layout);
            }
            new_ptr
        }
    }
}

#[inline]
unsafe fn gc_free(ptr: *mut u8, _: Layout) {
    unsafe {
        api::GC_free(ptr as *mut c_void);
    }
}

unsafe impl Allocator for GcAllocator {
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        metrics::increment(1, metrics::Metric::AllocationsGc);
        match layout.size() {
            0 => Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0)),
            size => unsafe {
                let ptr = gc_malloc(layout);
                let ptr = NonNull::new(ptr).ok_or(AllocError)?;
                Ok(NonNull::slice_from_raw_parts(ptr, size))
            },
        }
    }

    unsafe fn deallocate(&self, _: NonNull<u8>, _: Layout) {}
}

pub mod metrics {
    #[derive(Copy, Clone, Debug)]
    pub enum Metric {
        AllocationsArc,
        AllocationsGc,
        AllocationsRc,
        AllocationsBox,
        FinalizersRun,
        FinalizersElided,
        FinalizersRegistered,
    }

    trait MetricsImpl {
        fn init(&self) {}
        fn increment(&self, _amount: u64, _metric: Metric) {}
        fn capture(&self, _is_last: bool) {}
    }

    #[cfg(feature = "gc-metrics")]
    mod active {
        use core::sync::atomic::{AtomicU64, Ordering};

        use super::{Metric, MetricsImpl};

        pub(super) struct Metrics {
            finalizers_registered: AtomicU64,
            finalizers_elidable: AtomicU64,
            finalizers_completed: AtomicU64,
            barriers_visited: AtomicU64,
            allocated_gc: AtomicU64,
            allocated_arc: AtomicU64,
            allocated_rc: AtomicU64,
            allocated_boxed: AtomicU64,
        }

        impl Metrics {
            pub const fn new() -> Self {
                Self {
                    finalizers_registered: AtomicU64::new(0),
                    finalizers_elidable: AtomicU64::new(0),
                    finalizers_completed: AtomicU64::new(0),
                    barriers_visited: AtomicU64::new(0),
                    allocated_gc: AtomicU64::new(0),
                    allocated_arc: AtomicU64::new(0),
                    allocated_rc: AtomicU64::new(0),
                    allocated_boxed: AtomicU64::new(0),
                }
            }
        }

        pub extern "C" fn record_post_collection(event: crate::GC_EventType) {
            if event == crate::GC_EventType_GC_EVENT_END {
                super::METRICS.capture(false);
            }
        }

        impl MetricsImpl for Metrics {
            fn init(&self) {
                unsafe {
                    crate::GC_enable_benchmark_stats();
                    crate::GC_set_on_collection_event(Some(record_post_collection));
                }
            }

            fn increment(&self, amount: u64, metric: Metric) {
                match metric {
                    Metric::AllocationsArc => {
                        self.allocated_boxed.fetch_sub(amount, Ordering::Relaxed);
                        self.allocated_arc.fetch_add(amount, Ordering::Relaxed);
                    }
                    Metric::AllocationsRc => {
                        self.allocated_boxed.fetch_sub(amount, Ordering::Relaxed);
                        self.allocated_rc.fetch_add(amount, Ordering::Relaxed);
                    }
                    Metric::AllocationsBox => {
                        self.allocated_boxed.fetch_add(amount, Ordering::Relaxed);
                    }
                    Metric::AllocationsGc => {
                        self.allocated_gc.fetch_add(amount, Ordering::Relaxed);
                    }
                    Metric::FinalizersRun => {
                        self.finalizers_completed.fetch_add(amount, Ordering::Relaxed);
                    }
                    Metric::FinalizersElided => {
                        self.finalizers_completed.fetch_add(amount, Ordering::Relaxed);
                    }
                    Metric::FinalizersRegistered => {
                        self.finalizers_registered.fetch_add(amount, Ordering::Relaxed);
                    }
                }
            }

            fn capture(&self, is_last: bool) {
                // Must preserve this ordering as it's hardcoded inside BDWGC.
                // See src/bdwgc/misc.c:2812
                unsafe {
                    crate::GC_log_metrics(
                        self.finalizers_completed.load(Ordering::Relaxed),
                        self.finalizers_registered.load(Ordering::Relaxed),
                        self.allocated_gc.load(Ordering::Relaxed),
                        self.allocated_arc.load(Ordering::Relaxed),
                        self.allocated_rc.load(Ordering::Relaxed),
                        self.allocated_boxed.load(Ordering::Relaxed),
                        is_last as i32,
                    );
                }
            }
        }
    }

    #[cfg(not(feature = "gc-metrics"))]
    pub mod noop {
        use super::MetricsImpl;

        #[derive(Debug, Default)]
        pub struct Metrics;

        impl Metrics {
            pub const fn new() -> Self {
                Self
            }
        }

        impl MetricsImpl for Metrics {}
    }

    #[cfg(feature = "gc-metrics")]
    use active::Metrics;
    #[cfg(not(feature = "gc-metrics"))]
    use noop::Metrics;

    static METRICS: Metrics = Metrics::new();

    pub fn init() {
        METRICS.init();
    }

    pub fn record_final() {
        METRICS.capture(true);
    }

    pub fn increment(amount: u64, metric: Metric) {
        METRICS.increment(amount, metric);
    }
}
