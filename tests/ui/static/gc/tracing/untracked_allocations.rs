#![deny(untracked_heap_allocation)]
#![feature(allocator_api)]
#![feature(gc)]

use std::alloc::System;
use std::rc::Rc;

static A: System = System;

fn main() {
    let rc = Rc::new_in(123, A); //~ ERROR: uses a non-Alloy allocator
}
