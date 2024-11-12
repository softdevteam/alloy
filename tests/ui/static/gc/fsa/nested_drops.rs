#![feature(gc)]
#![feature(negative_impls)]
#![feature(rustc_private)]
#![allow(dead_code)]
#![allow(unused_variables)]
include!{"./auxiliary/types.rs"}


extern crate libc;

struct HasSafeDrop;
struct HasUnsafeDrop1;
struct HasUnsafeDrop2;

impl Drop for HasSafeDrop { fn drop(&mut self) { let x = 1;} }

impl Drop for HasUnsafeDrop1 {
    fn drop(&mut self) {
        unsafe { libc::malloc(8) as *mut i32 };
        foo();
    }
}

impl Drop for HasUnsafeDrop2 {
    fn drop(&mut self) {
        unsafe { libc::calloc(8, 8) as *mut i32 };
    }
}

fn foo() {
    let s = HasUnsafeDrop2;
}

#[derive(Debug)]
struct HasUnsafeNestedDrop(u8);
#[derive(Debug)]
struct HasSafeNestedDrop(u8);

impl Drop for HasUnsafeNestedDrop  {
    fn drop(&mut self) { let s = HasUnsafeDrop1; }
}

impl Drop for HasSafeNestedDrop  {
    fn drop(&mut self) { let s = HasSafeDrop; }
}

fn main() {
    Gc::new(FinalizerUnsafeWrapper(HasUnsafeNestedDrop(1)));
    //~^ ERROR: The drop method for `HasUnsafeNestedDrop` cannot be safely finalized.
    //~| ERROR: The drop method for `HasUnsafeNestedDrop` cannot be safely finalized.

    Gc::new(FinalizerUnsafeWrapper(HasSafeNestedDrop(1)));
}
