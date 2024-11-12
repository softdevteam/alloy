#![feature(gc)]
#![feature(negative_impls)]
#![feature(rustc_private)]
#![allow(dead_code)]
#![allow(unused_variables)]
include!{"./auxiliary/types.rs"}

use std::arch::asm;

#[derive(Debug)]
struct ASM;

impl Drop for ASM {
    fn drop(&mut self) {
        let a: u64 = 10;
        let b: u64 = 20;
        let result: u64;
        unsafe {
            asm!(
                "add {0}, {1}, {2}",
                out(reg) result,
                in(reg) a,
                in(reg) b
            );
        }
    }
}


fn main() {
    Gc::new(FinalizerUnsafeWrapper(ASM));
    //~^ ERROR: The drop method for `ASM` cannot be safely finalized.
}
