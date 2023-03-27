#![feature(gc)]
#![feature(negative_impls)]

use std::gc::Gc;
use std::rc::Rc;
use std::cell::Cell;
use std::marker::FinalizerSafe;

struct ShouldPass(*mut u8);

impl Drop for ShouldPass {
    // Drop doesn't do anything dangerous, so this shouldn't bork.
    fn drop(&mut self) {
        println!("Dropping Hello");
    }
}

struct ShouldFail(Cell<usize>);

impl !FinalizerSafe for ShouldFail{}

impl Drop for ShouldFail {
    // We mutate via an unsynchronized field here, this should bork.
    fn drop(&mut self) {
        self.0.replace(456);
    }
}

trait Opaque {}

impl Opaque for ShouldPass {}

struct HasGcFields(Gc<usize>);

impl Drop for HasGcFields {
    fn drop(&mut self) {
        println!("Boom {}", self.0);
    }
}

struct ShouldFail2(*mut u8);

impl ShouldFail2 {
    #[inline(never)]
    fn foo(&mut self) {}
}

impl Drop for ShouldFail2 {
    fn drop(&mut self) {
        self.foo();
    }
}

struct ShouldPassInFuture(*mut u8);

impl !FinalizerSafe for ShouldPassInFuture {}

impl ShouldPassInFuture {
    fn transparent(&self) {
        println!("Value is {:?}", self.0);
    }
}

impl Drop for ShouldPassInFuture {
    fn drop(&mut self) {
        let x = ShouldPassInFuture(456 as *mut u8);
        x.transparent();
    }
}

fn main() {
    Gc::new(ShouldPass(123 as *mut u8));

    Gc::new(ShouldFail(Cell::new(123))); //~ ERROR: `ShouldFail(Cell::new(123))` cannot be safely finalized.

    let boxed_trait: Box<dyn Opaque> = Box::new(ShouldPass(123 as *mut u8));
    Gc::new(boxed_trait); //~ ERROR: `boxed_trait` cannot be safely finalized.
                          //~^ ERROR: `boxed_trait` cannot be safely constructed.

    let gcfields = HasGcFields(Gc::new(123));
    Gc::new(gcfields); //~ ERROR: `gcfields` cannot be safely finalized.

    let self_call = ShouldFail2(123 as *mut u8);
    Gc::new(self_call); //~ ERROR: `self_call` cannot be safely finalized.

    let transparent_call = ShouldPassInFuture(123 as *mut u8);
    Gc::new(transparent_call); //~ ERROR: `transparent_call` cannot be safely finalized.
}
