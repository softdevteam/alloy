// run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![feature(negative_impls)]

use std::gc::{Gc, GcAllocator, FinalizeUnchecked};
use std::sync::atomic::{self, AtomicUsize};
use std::thread;
use std::time;

struct UnsafeContainer(usize);

impl Drop for UnsafeContainer {
    fn drop(&mut self) {
        FINALIZER_COUNT.fetch_add(1, atomic::Ordering::Relaxed);
    }
}

impl !FinalizerSafe for UnsafeContainer {}

static FINALIZER_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_COUNT: usize = 10;
static SLEEP_MAX: u64 = 8192; // in millis.

fn foo() {
    for i in 0..ALLOCATED_COUNT {
        {
            let mut _gc = unsafe { Some(Gc::new(FinalizeUnchecked::new(UnsafeContainer(i)))) };

            // Zero the root to the GC object.
            _gc = None;
        }
    }
}

fn main() {
    foo();
    GcAllocator::force_gc();

    let mut count = FINALIZER_COUNT.load(atomic::Ordering::Relaxed);
    let mut sleep_duration = 2;
    while count < ALLOCATED_COUNT - 1 && sleep_duration <= SLEEP_MAX {
        // Wait an acceptable amount of time for the finalizer thread to do its work.
        thread::sleep(time::Duration::from_millis(sleep_duration));
        sleep_duration = sleep_duration * 2;
        count = FINALIZER_COUNT.load(atomic::Ordering::Relaxed);
    }

    // On some platforms, the last object might not be finalised because it's
    // kept alive by a lingering reference.
    assert!(count >= ALLOCATED_COUNT - 1);
    assert!(count <= ALLOCATED_COUNT);
}
