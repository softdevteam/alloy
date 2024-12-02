#![feature(gc)]
#![feature(negative_impls)]
#![allow(dead_code)]
#![allow(unused_variables)]
include!{"./auxiliary/types.rs"}

use std::cell::Cell;

thread_local! {
    static COUNTER: Cell<u32> = Cell::new(0);
}


#[derive(Debug)]
struct S;

impl Drop for S {
    fn drop(&mut self) {
        // Access the thread-local variable
        let x = COUNTER.get();
    }
}

fn main() {
    Gc::new(FinalizerUnsafeWrapper(S));
    //~^ ERROR: The drop method for `S` cannot be safely finalized.
}
