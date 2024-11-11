#![feature(gc)]
#![feature(negative_impls)]
#![allow(dead_code)]
#![allow(unused_variables)]
include!{"./auxiliary/types.rs"}

impl<T: Debug> Drop for Wrapper<T> {
    fn drop(&mut self) {
        fsa_unsafe(&self, true);
        fsa_safe(&self, true);
    }
}

#[inline(never)]
fn fsa_unsafe<T: Debug>(x: &Wrapper<T>, recurse: bool) {
    if recurse {
        fsa_unsafe(x, false);
    }
    use_val(&x.0); // should fail
}

#[inline(never)]
fn fsa_safe<T: Debug>(x: &Wrapper<T>, recurse: bool) {
    if recurse {
        fsa_safe(x, false);
    }
    use_val(&x); // should pass
}

fn main() {
    Gc::new(Wrapper(NotFinalizerSafe(1)));
    //~^   ERROR: The drop method for `Wrapper<NotFinalizerSafe>` cannot be safely finalized.
}
