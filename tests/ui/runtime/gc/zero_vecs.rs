//@ ignore-test
#![feature(gc)]
#![feature(negative_impls)]
#![allow(unused_assignments)]
#![allow(unused_variables)]
#![allow(dead_code)]

use std::gc::{Gc, GcAllocator};
use std::sync::atomic::{self, AtomicUsize};
use std::thread;
use std::time;

struct Finalizable(usize);

impl Drop for Finalizable {
    fn drop(&mut self) {
        FINALIZER_COUNT.fetch_add(1, atomic::Ordering::Relaxed);
    }
}

static FINALIZER_COUNT: AtomicUsize = AtomicUsize::new(0);

fn test_pop(v: &mut Vec<Gc<Finalizable>, GcAllocator>) {
    for i in 0..10 {
        let mut gc = Some(Gc::new(Finalizable(i)));
        v.push(gc.unwrap());
        gc = None;
    }

    for _ in 0..10 {
        let mut _gc = Some(v.pop());
        _gc = None;
    }

}

fn main() {
    let capacity = 10;
    let mut v1 = Vec::with_capacity_in(capacity, GcAllocator);
    test_pop(&mut v1);
    test_pop(&mut v1);

    GcAllocator::force_gc();

    // Wait enough time for the finaliser thread to finish running.
    thread::sleep(time::Duration::from_millis(100));

    // This tests that finalisation happened indirectly by trying to overwrite references to live GC
    // objects in order for Boehm to consider them dead. This is inherently flaky because we might
    // miss some references which linger on the stack or in registers. This tends to happen for the
    // last on-stack reference to an object in a tight loop.
    //
    // In this case it doesn't really matter whether or not the last object was finalised. Instead,
    // what matters is that *most* were, as this is enough to have confidence that popping an item
    // from a vector does not allow it to be indirectly kept alive from within the vector's backing
    // store.
    assert!(FINALIZER_COUNT.load(atomic::Ordering::Relaxed) >= (capacity * 2) -1);
}
