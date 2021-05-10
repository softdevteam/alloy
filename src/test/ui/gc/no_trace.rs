// run-pass
#![feature(gc)]
#![allow(dead_code)]

use std::gc::needs_tracing;
use std::rc::Rc;
use std::cell::Cell;


struct A {
    inner1: B,
    inner2: *mut u8
}

enum B {
    W(Vec<C>),
    X((usize, Vec<C>)),
    Y(Vec<B>),
    Z(String),
}

struct C {
    a: Rc<usize>,
    b: Cell<usize>
}

struct D(Vec<A>);

const CONST_A: bool = needs_tracing::<A>();
const CONST_B: bool = needs_tracing::<B>();
const CONST_C: bool = needs_tracing::<C>();
const CONST_D: bool = needs_tracing::<D>();

fn main() {
    assert!(CONST_A);
    assert!(!CONST_B);
    assert!(!CONST_C);
    assert!(CONST_D);
}
