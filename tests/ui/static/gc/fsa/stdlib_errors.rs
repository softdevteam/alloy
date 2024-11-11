#![feature(gc)]
#![feature(negative_impls)]

use std::gc::Gc;
use std::rc::Rc;

// S is FSA-safe but the inner RC is not.
#[derive(Clone)]
struct S(Rc<u8>);

struct Unsafe(u8);
impl !FinalizerSafe for Unsafe {}

// Make sure that FSA still reports an error for the `Unsafe` field.
struct T(S, Unsafe);

// This should only give a single `Rc` FSA error.
struct U(Rc<Rc<Rc<u8>>>);

impl Drop for T {
    fn drop(&mut self) {
        let x = self.1.0; // should fail
    }
}

fn main() {
    let s = S(Rc::new(1));
    let t = T(s.clone(), Unsafe(1));
    let u = U(Rc::new(Rc::new(Rc::new(1))));

    Gc::new(s);
    //~^ ERROR: The drop method for `Rc<u8>` cannot be safely finalized.

    Gc::new(t);
    //~^  ERROR: The drop method for `T` cannot be safely finalized.
    //~^^ ERROR: The drop method for `Rc<u8>` cannot be safely finalized.

    Gc::new(u);
    //~^  ERROR: The drop method for `Rc<Rc<Rc<u8>>>` cannot be safely finalized.
}
