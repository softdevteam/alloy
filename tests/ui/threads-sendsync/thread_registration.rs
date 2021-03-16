// run-pass
// ignore-emscripten no threads support
#![feature(rustc_private)]

use std::alloc::GcAllocator;
use std::thread;

pub fn main() {
    let res = thread::spawn(child).join().unwrap();
    assert!(res);
}

fn child() -> bool {
    GcAllocator::thread_registered()
}
