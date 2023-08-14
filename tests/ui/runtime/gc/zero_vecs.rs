// run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![feature(rustc_private)]
#![feature(negative_impls)]
#![feature(allocator_api)]
#![allow(unused_assignments)]
#![allow(unused_variables)]

use std::gc::{Gc, GcAllocator};
use std::sync::atomic::{self, AtomicUsize};

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
    let mut v1 = Vec::with_capacity_in(10, GcAllocator);
    test_pop(&mut v1);
    test_pop(&mut v1);

    GcAllocator::force_gc();

    assert_eq!(FINALIZER_COUNT.load(atomic::Ordering::Relaxed), 20);
}
