// run-pass
#![feature(gc)]

use std::alloc::GcAllocator;
use std::gc::Gc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{thread, time};

struct PanicOnDrop(String);

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        eprintln!("Finalizer called. Object erroneously collected");
    }
}

static NO_CHILD_EXISTS: AtomicBool = AtomicBool::new(true);

fn main() {
    for _ in 1..10 {
        thread::spawn(child);
    }

    while NO_CHILD_EXISTS.load(Ordering::SeqCst) {}

    // This should collect no garbage, because the call stacks of each child
    // thread should be scanned for roots.
    GcAllocator::force_gc();

    // If there's a problem, a finalizer will print to stderr. Lets wait an
    // appropriate amount of time for this to happen.
    thread::sleep(time::Duration::from_millis(10));
}

fn child() {
    NO_CHILD_EXISTS.store(false, Ordering::SeqCst);
    let _gc = Gc::new(PanicOnDrop(String::from("Hello world!")));

    // Wait a bit before dying, ensuring that the thread stays alive long enough
    // cross the force_gc call.
    thread::sleep(time::Duration::from_millis(5));
}
