#![feature(gc)]
#![feature(negative_impls)]
#![allow(dead_code)]
#![allow(unused_variables)]
include!{"./auxiliary/types.rs"}

impl Drop for HasNestedGc {
    fn drop(&mut self) {
        use_val(self.a); // should fail
        use_val(self.b); // should pass

        // Project through to the nested `HasGc` struct.
        use_val(self.c.a); // should fail
        use_val(self.c.b); // should pass

        let a = self.a; // should fail
        let b = self.b; // should pass

        let ca = self.a; // should fail
        let cb = self.b; // should pass

        // should pass, as not a field projection
        let d = &1;
        use_val(d);
    }
}

fn main() {
    Gc::new(HasNestedGc::default());
    //~^     ERROR: The drop method for `HasNestedGc` cannot be safely finalized.
    //~|     ERROR: The drop method for `HasNestedGc` cannot be safely finalized.
    //~|     ERROR: The drop method for `HasNestedGc` cannot be safely finalized.
    //~|     ERROR: The drop method for `HasNestedGc` cannot be safely finalized.
}
