#![feature(gc)]
#![feature(negative_impls)]
#![feature(rustc_private)]
#![allow(dead_code)]
#![allow(unused_variables)]

use std::gc::Gc;
use std::mem::ManuallyDrop;

struct S(u8);
struct T(u8);

impl Drop for S {
    fn drop(&mut self) {
    }
}


impl Drop for T {
    fn drop(&mut self) {
    }
}

union U {
    a: ManuallyDrop<S>,
    b: ManuallyDrop<T>,
}

impl Drop for U {
    fn drop(&mut self) {
        let x = unsafe  { &self.a };
    }
}

impl !Send for U {}
impl !Send for S {}
impl !Send for T {}

fn main() {
    let u = U { a: ManuallyDrop::new(S(1)) };
    Gc::new(u);
    //~^ ERROR: The drop method for `U` cannot be safely finalized.
}
