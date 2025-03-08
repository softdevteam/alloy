#![feature(gc)]
#![feature(negative_impls)]
#![allow(dead_code)]
#![allow(unused_variables)]
include!{"./auxiliary/types.rs"}

impl<'a> Drop for HasNestedRef<'a> {
    fn drop(&mut self) {
        use_val(self.a); // should fail
        use_val(self.b); // should pass

        // Project through to the nested `HasRef` struct.
        use_val(self.c.a); // should fail
        use_val(self.c.b); // should pass

        let a = self.a; // should fail
        let b = self.b; // should pass

        let ca = self.a; // should fail
        let cb = self.b; // should pass

        // should pass, as not a field projection
        let d = &1;
        use_val(d);

        let e = HasRef::default();
        // Should fail as it is a field projection. Ideally this should be allowed because these
        // references are not fields on the `self` type. However, FSA is not sophisticated enough to
        // make this distinction.
        use_val(e.a);
        // Should pass
        use_val(e.b);
    }
}

fn main() {
    std::gc::Gc::new(HasNestedRef::default());
    //~^     ERROR: `HasNestedRef::default()` has a drop method which cannot be safely finalized.
    //~^^    ERROR: `HasNestedRef::default()` has a drop method which cannot be safely finalized.
    //~^^^   ERROR: `HasNestedRef::default()` has a drop method which cannot be safely finalized.
    //~^^^^  ERROR: `HasNestedRef::default()` has a drop method which cannot be safely finalized.
    //~^^^^^ ERROR: `HasNestedRef::default()` has a drop method which cannot be safely finalized.
    //~^^^^^^ERROR: `HasNestedRef::default()` has a drop method which cannot be safely finalized.
}
