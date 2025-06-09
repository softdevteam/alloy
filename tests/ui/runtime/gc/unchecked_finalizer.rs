//@ ignore-test
// ignore-tidy-linelength
#![feature(gc)]
#![feature(rustc_private)]
#![feature(negative_impls)]
#![allow(dead_code)]

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

static FINALIZER_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_COUNT: usize = 100;

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

    // Wait enough time for the finaliser thread to finish running.
    thread::sleep(time::Duration::from_millis(100));

    // On some platforms, the last object might not be finalised because it's
    // kept alive by a lingering reference.
    assert!(FINALIZER_COUNT.load(atomic::Ordering::Relaxed) >= ALLOCATED_COUNT -1);
}
