// run-pass
// ignore-tidy-linelength
#![feature(gc)]

use std::gc::{Gc, GcAllocator};
use std::sync::atomic::{self, AtomicUsize};

struct Finalizable(usize);

impl Drop for Finalizable {
    fn drop(&mut self) {
        FINALIZER_COUNT.fetch_add(1, atomic::Ordering::Relaxed);
    }
}

static FINALIZER_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_COUNT: usize = 100;

fn foo() {
    for i in 0..ALLOCATED_COUNT {
        {
            let mut _gc = Some(Gc::new(Finalizable(i)));

            // Zero the root to the GC object.
            _gc = None;
        }
    }
}

fn main() {
    foo();
    GcAllocator::force_gc();
    assert_eq!(FINALIZER_COUNT.load(atomic::Ordering::Relaxed), ALLOCATED_COUNT);
}
