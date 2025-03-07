//@ run-pass
//@ no-prefer-dynamic
// ignore-tidy-linelength
#![feature(allocator_api)]
#![feature(gc)]
#![feature(negative_impls)]
#![feature(thread_local)]

use std::gc::{Gc, GcAllocator};
use std::{thread, time};
use std::sync::atomic::{self, AtomicUsize};
use std::time::{SystemTime, UNIX_EPOCH};

#[global_allocator]
static GC: GcAllocator = GcAllocator;

struct Finalizable(u32);

static FINALIZER_COUNT: AtomicUsize = AtomicUsize::new(0);

impl Drop for Finalizable {
    fn drop(&mut self) {
        FINALIZER_COUNT.fetch_add(1, atomic::Ordering::Relaxed);
    }
}

thread_local!{
    static LOCAL1: Gc<Finalizable> = Gc::new(Finalizable(1));
    static LOCAL2: Gc<Finalizable> = Gc::new(Finalizable(2));
    static LOCAL3: Gc<Finalizable> = Gc::new(Finalizable(3));

    static LOCAL4: Box<Gc<Finalizable>> = Box::new(Gc::new(Finalizable(4)));
    static LOCAL5: Box<Gc<Finalizable>> = Box::new(Gc::new(Finalizable(5)));
    static LOCAL6: Box<Gc<Finalizable>> = Box::new(Gc::new(Finalizable(6)));
}

fn do_stuff_with_tls() {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();

    // We need to use the thread-local at least once to ensure that it is initialised. By adding it
    // to the current system time, we ensure that this use can't be optimised away (e.g. by constant
    // folding).
    let mut dynamic_value = nanos;

    dynamic_value += LOCAL1.with(|l| l.0);
    dynamic_value += LOCAL2.with(|l| l.0);
    dynamic_value += LOCAL3.with(|l| l.0);
    dynamic_value += LOCAL4.with(|l| l.0);
    dynamic_value += LOCAL5.with(|l| l.0);
    dynamic_value += LOCAL6.with(|l| l.0);

    // Keep the thread alive long enough so that the GC has the chance to scan its thread-locals for
    // roots.
    thread::sleep(time::Duration::from_millis(20));


    assert!(dynamic_value > 0);

    // This ensures that a GC invoked from the main thread does not cause this thread's thread
    // locals to be reclaimed too early.
    assert_eq!(FINALIZER_COUNT.load(atomic::Ordering::Relaxed), 0);

}

fn main() {
    let t2 = std::thread::spawn(do_stuff_with_tls);

    // Wait a little bit of time for the t2 to initialise thread-locals.
    thread::sleep(time::Duration::from_millis(10));

    GcAllocator::force_gc();

    let _ = t2.join().unwrap();
}
