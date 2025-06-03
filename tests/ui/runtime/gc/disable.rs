//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use std::gc::{disable, enable, is_enabled, try_enable};

fn main() {
    assert!(is_enabled());
    disable();
    disable();

    assert!(!is_enabled());

    assert!(!try_enable());
    assert!(try_enable());

    disable();
    disable();
    disable();

    enable();
    assert!(is_enabled());
}
