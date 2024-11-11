#![feature(gc)]
#![feature(negative_impls)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

impl<T: Debug> Drop for Wrapper<T> {
    fn drop(&mut self) {
        use_val(&self.0);
    }
}

impl<T: Debug> Drop for FinalizerUnsafeWrapper<T> {
    fn drop(&mut self) {
        use_val(&self.0);
    }
}

fn main() {
    Gc::new(Wrapper(FinalizerUnsafeWrapper(FinalizerUnsafeWrapper(FinalizerUnsafeType(1)))));
    //~^ ERROR: The drop method for `Wrapper<FinalizerUnsafeWrapper<FinalizerUnsafeWrapper<FinalizerUnsafeType>>>` cannot be safely finalized.
    //~| ERROR: The drop method for `FinalizerUnsafeWrapper<FinalizerUnsafeWrapper<FinalizerUnsafeType>>` cannot be safely finalized.
}
