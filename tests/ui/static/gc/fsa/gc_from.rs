#![feature(gc)]
#![feature(negative_impls)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

impl<'a> Drop for HasRef<'a> {
    fn drop(&mut self) {
        use_val(self.a); // should fail
    }
}

fn main() {
    let _: Gc<HasRef> = Gc::from(HasRef::default());
    //~^ ERROR: The drop method for `HasRef<'_>` cannot be safely finalized.
    let _: Gc<HasRef> = Gc::from(Box::new(HasRef::default()));
    //~^ ERROR: The drop method for `HasRef<'_>` cannot be safely finalized.
    let _: Gc<[HasRef]> = Gc::from(vec![HasRef::default()]);
    //~^ ERROR: The drop method for `HasRef<'_>` cannot be safely finalized.
    let _: Gc<[HasRef]> = Gc::from(vec![HasRef::default()].into_boxed_slice());
    //~^ ERROR: The drop method for `HasRef<'_>` cannot be safely finalized.

    // The following should all pass.
    let _: Gc<u8> = Gc::from(1);
    let _: Gc<u8> = Gc::from(Box::new(1));
    let _: Gc<[u8]> = Gc::from(vec![1, 2, 3]);
    let _: Gc<[u8]> = Gc::from(vec![1, 2, 3].into_boxed_slice());
}
