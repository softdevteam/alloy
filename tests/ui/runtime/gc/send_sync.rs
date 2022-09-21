// run-pass
#![feature(gc)]
#![feature(negative_impls)]

use std::gc::Gc;
use std::sync::Mutex;

fn assert_send<T: Send>() {}
fn assert_both<T: Sync + Send>() {}

struct NoSync;

impl !Sync for NoSync {}

fn main() {
    assert_send::<Gc<NoSync>>();
    assert_both::<Gc<Mutex<usize>>>();
}
