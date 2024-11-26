#![feature(gc)]
#![feature(negative_impls)]
#![feature(rustc_private)]
#![allow(dead_code)]
#![allow(unused_variables)]
include!{"./auxiliary/types.rs"}

struct S(*mut u8);

impl Drop for S {
    fn drop(&mut self) {
        use_val(self.0);
    }
}

struct T(*mut u8);

unsafe impl Send for T {}
unsafe impl Sync for T {}

fn main() {
    Gc::new(S(std::ptr::null_mut()));
    //~^ ERROR: The drop method for `S` cannot be safely finalized.

    Gc::new(T(std::ptr::null_mut()));
}
