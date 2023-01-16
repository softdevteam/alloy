#![allow(missing_docs)]
#[unstable(feature = "gc", issue = "none")]
pub use core::gc::*;

#[unstable(feature = "gc", issue = "none")]
pub use alloc_crate::gc::*;

#[unstable(feature = "gc", issue = "none")]
pub use boehm::*;

#[cfg(profile_gc)]
pub fn print_gc_stats() {
    println!(
        "Finalizers registered: {}",
        FINALIZERS_REGISTERED.load(core::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "Finalizers completed: {}",
        FINALIZERS_COMPLETED.load(core::sync::atomic::Ordering::Relaxed)
    );
}
