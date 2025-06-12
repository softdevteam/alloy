#![allow(missing_docs)]

#[cfg(not(no_gc))]
#[allow(nonstandard_style)]
#[allow(missing_debug_implementations)]
pub mod api {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[cfg(not(no_gc))]
pub use api::*;

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
