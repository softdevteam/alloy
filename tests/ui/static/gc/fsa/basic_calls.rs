#![feature(gc)]
#![feature(negative_impls)]
#![allow(dead_code)]
#![allow(unused_variables)]
include!{"./auxiliary/types.rs"}

impl<T: Debug> Drop for Wrapper<T> {
    fn drop(&mut self) {
        foo(self);
        bar(self);
        baz(self);
    }
}

#[inline(never)]
fn foo<T: Debug>(x: &Wrapper<T>) {
    use_val(&x.0); // should fail
}

#[inline(never)]
fn bar<T: Debug>(x: &Wrapper<T>) {
    foo(x);
}

#[inline(never)]
fn baz<T: Debug>(x: &Wrapper<T>) {
    use_val(&x.0); // should fail
    use_val(&x); // should pass (no projection)
}

fn main() {
    Gc::new(Wrapper(FinalizerUnsafeU8Wrapper(1)));
    //~^ ERROR: The drop method for `Wrapper<FinalizerUnsafeU8Wrapper>` cannot be safely finalized.
    //~| ERROR: The drop method for `Wrapper<FinalizerUnsafeU8Wrapper>` cannot be safely finalized.

    Gc::new(Wrapper(Wrapper(1)));
}
