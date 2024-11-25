#![feature(gc)]
#![feature(negative_impls)]
#![allow(dead_code)]
#![allow(unused_variables)]
include!{"./auxiliary/types.rs"}

impl<'a> Drop for HasRef<'a> {
    fn drop(&mut self) {
        use_val(self.a); // should fail
        use_val(self.b); // should pass
        use_val(self.c[0]); // should fail

        let a = self.a; // should fail
        let b = self.b; // should pass
        let c = self.c;
        use_val(c[1]); // should fail

        // should pass, as not a field projection
        let c = &1;
        use_val(c);
    }
}

fn main() {
    Gc::new(HasRef::default());
    //~^     ERROR: The drop method for `HasRef<'_>` cannot be safely finalized.
    //~|     ERROR: The drop method for `HasRef<'_>` cannot be safely finalized.
    //~|     ERROR: The drop method for `HasRef<'_>` cannot be safely finalized.
    //~|     ERROR: The drop method for `HasRef<'_>` cannot be safely finalized.
}
