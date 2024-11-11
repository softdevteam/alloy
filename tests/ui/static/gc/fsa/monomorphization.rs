#![feature(gc)]
#![feature(negative_impls)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

impl<T: Debug> Drop for Wrapper<T> {
    fn drop(&mut self) {
        foo(&self);
        use_val(&self.0);
    }
}

fn foo<T: Debug>(val: T) {
    use_val(val);
}

impl Drop for U8Wrapper {
    fn drop(&mut self) {
        use_val(self.0);
        bar(&self);
    }
}

#[derive(Debug)]
struct S(FinalizerUnsafeU8Wrapper);

impl Drop for S {
    fn drop(&mut self) {
        use_val(&self);
        baz(&self.0);
    }
}

fn bar(val: &U8Wrapper) {
    use_val(val.0);
}

fn baz(val: &FinalizerUnsafeU8Wrapper) {
    use_val(val.0);
}


fn main() {
    // Test that we can use the monomorphized MIR for `Wrapper<T>::drop`.

    // This should pass, because the monomorphized drop is `Wrapper<Wrapper<u8>>::drop` and
    // implements `Send` + `Sync` + `FinalizerSafe`.
    //
    // This will fail if FSA can't obtain the monomorphized drop method, as FSA can't know if the
    // `T` in `Wrapper<T>::drop` implements `Send` + `Sync` + `FinalizerSafe`. Since the generic
    // substitutions are available, that would be a bug.
    let _: Gc<Wrapper<Wrapper<u8>>> = Gc::new(Wrapper(Wrapper(1)));

    // This should fail, but only for `FinalizerSafe` because
    // `Wrapper<FinalizerUnsafeWrapper<u8>>::drop` implements `Send` + `Sync` but not
    // `FinalizerSafe`. If monomorphization fails, we will get a `NotSendAndSync` error which would
    // be incorrect.
    let _: Gc<Wrapper<FinalizerUnsafeWrapper<u8>>> = Gc::new(Wrapper(FinalizerUnsafeWrapper(1)));
    //~^ ERROR: The drop method for `Wrapper<FinalizerUnsafeWrapper<u8>>` cannot be safely finalized.

    // Test that trying to monomorphize MIR doesn't break non-generic drops.

    Gc::new(U8Wrapper(1));

    Gc::new(S(FinalizerUnsafeU8Wrapper(1)));
    //~^ ERROR: The drop method for `S` cannot be safely finalized.
}
