//@ run-pass
//@ ignore-emscripten no threads support
#![feature(gc)]

use std::thread;

pub fn main() {
    let res = thread::spawn(child).join().unwrap();
    assert!(res);
}

fn child() -> bool {
    std::gc::thread_registered()
}
