// run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]

use std::mem;

enum A {
    B(B),
}

struct B(Box<A>);

const CYCLIC: bool = mem::needs_finalizer::<A>();

fn main() {
    assert!(!CYCLIC);
}
