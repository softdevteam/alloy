#![feature(gc)]
#![feature(negative_impls)]

use std::cell::Cell;
use std::gc::{Gc, FinalizeUnchecked};
use std::marker::FinalizerSafe;
use std::rc::Rc;

struct ShouldPass(*mut u8);

impl Drop for ShouldPass {
    // Drop doesn't do anything dangerous, so this shouldn't bork.
    fn drop(&mut self) {
        println!("Dropping Hello");
    }
}

struct ShouldFail(Cell<usize>);

impl !FinalizerSafe for ShouldFail {}

impl Drop for ShouldFail {
    // We mutate via an unsynchronized field here, this should bork.
    fn drop(&mut self) {
        self.0.replace(456);
    }
}

trait Opaque {}

impl Opaque for ShouldPass {}

struct ShouldFail2(*mut u8);

struct NotThreadSafe(usize);

impl !Send for NotThreadSafe {}
impl !Sync for NotThreadSafe {}

struct ShouldFail3(NotThreadSafe);


impl ShouldFail2 {
    #[inline(never)]
    fn foo(&mut self) {}
}

impl Drop for ShouldFail2 {
    fn drop(&mut self) {
        self.foo();
    }
}

impl Drop for ShouldFail3 {
    fn drop(&mut self) {
        println!("Boom {}", self.0.0);
    }
}

fn main() {
    Gc::new(ShouldPass(123 as *mut u8));

    Gc::new(ShouldFail(Cell::new(123)));
    //~^ ERROR: The drop method for `ShouldFail` cannot be safely finalized.

    let self_call = ShouldFail2(123 as *mut u8);
    Gc::new(self_call);
    //~^ ERROR: The drop method for `ShouldFail2` cannot be safely finalized.

    let not_threadsafe = ShouldFail3(NotThreadSafe(123));
    Gc::new(not_threadsafe);
    //~^ ERROR: The drop method for `ShouldFail3` cannot be safely finalized.

    unsafe { Gc::new(FinalizeUnchecked::new(ShouldFail(Cell::new(123)))) };
}
