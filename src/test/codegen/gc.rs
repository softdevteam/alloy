// ignore-wasm32-bare compiled with panic=abort by default
// compile-flags: -C no-prepopulate-passes

#![crate_type = "lib"]
#![feature(gc)]

use std::gc::Gc;

struct Finalizable(usize);

impl Drop for Finalizable {
    fn drop(&mut self) {
    }
}

// CHECK-LABEL: @will_drop
#[no_mangle]
pub fn will_drop() {
   let _gc = Gc::new(Finalizable(123));
// CHECK-COUNT-1: {{(call|invoke) .*}}drop_in_place<alloc::gc::Gc<gc::Finalizable>>
// CHECK-LABEL: {{^[}]}}
}

// CHECK-LABEL: @wont_drop
#[no_mangle]
pub fn wont_drop() {
   let a = Gc::new(123);
   Gc::new("Hello");
   let b: Gc<Vec<usize>> = Gc::new(Vec::new());
// CHECK-NOT: {{(call|invoke) .*}}drop_in_place{{.*}}
// CHECK-LABEL: {{^[}]}}
}
