// run-pass
// no-prefer-dynamic
// ignore-tidy-linelength

#![feature(gc)]
#![feature(rustc_private)]
#![allow(dead_code)]

use std::gc::GcAllocator;
use std::mem::make_collectable;

#[global_allocator]
static A: GcAllocator = GcAllocator;

fn main() {
    test_adt();
}

fn test_adt() {
    struct S(Box<usize>);
    struct T(S, Box<usize>, usize);
    struct Named<'a> {
        a: Box<usize>,
        b: &'a Box<usize>,
        c: usize
    }

    enum MaybeCollectable {
        Yes(Box<usize>),
        No(usize),
    }

    enum MaybeCollectableFields {
        Yes(Box<usize>, S),
        No(usize),
    }

    enum MaybeCollectableNested {
        Yes(MaybeCollectable),
        // Maybe(Option<MaybeCollectable>),
        No(usize),
    }

    // Simple top level struct (Box) with a Collectable trait impl.
    {
        let test = Box::new(123);

        assert!(!(GcAllocator::is_managed(test.as_ref())));

        unsafe { make_collectable(&test) };

        assert!(GcAllocator::is_managed(test.as_ref()));
    }

    // Box nested inside a struct. This checks that we correctly project
    // through fields.
    {
        let test = S(Box::new(123));

        assert!(!(GcAllocator::is_managed(test.0.as_ref())));
        unsafe { make_collectable(&test) };
        assert!(GcAllocator::is_managed(test.0.as_ref()));
    }

    // More complex nesting
    {
        let test = T(S(Box::new(123)), Box::new(456), 789);

        assert!(!(GcAllocator::is_managed(test.0.0.as_ref())));
        assert!(!(GcAllocator::is_managed(test.1.as_ref())));

        unsafe { make_collectable(&test) };

        assert!(GcAllocator::is_managed(test.0.0.as_ref()));
        assert!(GcAllocator::is_managed(test.1.as_ref()));
    }

    // Same, but with named fields and a reference.
    {
        let b = Box::new(123);
        let test = Named {
            a: Box::new(456),
            b: &b,
            c: 789,
        };

        assert!(!(GcAllocator::is_managed(test.a.as_ref())));
        assert!(!(GcAllocator::is_managed(test.b.as_ref())));

        unsafe { make_collectable(&test) };

        assert!(GcAllocator::is_managed(test.a.as_ref()));
        assert!(GcAllocator::is_managed(test.b.as_ref()));

    }

    // Test an enum, where the variant is only known at runtime.
    {
        let test = MaybeCollectable::Yes(Box::new(123));

        match test {
            MaybeCollectable::Yes(ref b) => assert!(!(GcAllocator::is_managed(b.as_ref()))),
            _ => assert!(false),
        }

        unsafe { make_collectable(&test) };

        match test {
            MaybeCollectable::Yes(ref b) => assert!(GcAllocator::is_managed(b.as_ref())),
            _ => assert!(false),
        }
    }

    // Test an enum with many fields
    {
        let test = MaybeCollectableFields::Yes(Box::new(123), S(Box::new(456)));

        match test {
            MaybeCollectableFields::Yes(ref a, ref b) => {
                assert!(!(GcAllocator::is_managed(a.as_ref())));
                assert!(!(GcAllocator::is_managed(b.0.as_ref())));
            }
            _ => assert!(false),
        }

        unsafe { make_collectable(&test) };

        match test {
            MaybeCollectableFields::Yes(ref a, ref b) => {
                assert!(GcAllocator::is_managed(a.as_ref()));
                assert!(GcAllocator::is_managed(b.0.as_ref()));
            }
            _ => assert!(false),
        }
    }

    // Test a nested enum
    enum A {
        V1(B),
        V2(usize)
    }

    enum B {
        V1(ColMe),
        V2(usize)
    }

    static mut COUNT: usize = 0;

    struct ColMe(usize);

    unsafe impl std::gc::Collectable for ColMe {
        unsafe fn set_collectable(&self) {
            COUNT += 1;
        }
    }

    unsafe impl std::gc::Collectable for B {
        unsafe fn set_collectable(&self) {
            COUNT += 1;
        }
    }

    {
        let test = B::V1(ColMe(123));
        unsafe {
            assert!(COUNT == 0);
            make_collectable(&test);
            assert!(COUNT == 2);
        }
    }

    {
        let test = MaybeCollectableNested::Yes(MaybeCollectable::Yes(Box::new(123)));

        match test {
            MaybeCollectableNested::Yes(ref y) => {
                match y {
                    MaybeCollectable::Yes(ref yinner) => assert!(!(GcAllocator::is_managed(yinner.as_ref()))),
                    _ => assert!(false),
                }
            }
            _ => assert!(false),
        }

        unsafe { make_collectable(&test) };

        match test {
            MaybeCollectableNested::Yes(ref y) => {
                match y {
                    MaybeCollectable::Yes(ref yinner) => assert!(GcAllocator::is_managed(yinner.as_ref())),
                    _ => assert!(false),
                }
            }
            _ => assert!(false),
        }
    }

    // Test an enum with multiple collectable variants
    enum MultiVariant {
        One(S),
        Two(Box<usize>)
    }

    {
        let test = MultiVariant::One(S(Box::new(123)));

        match test {
            MultiVariant::One(ref s) => assert!(!(GcAllocator::is_managed(s.0.as_ref()))),
            _ => assert!(false),
        }

        unsafe { make_collectable(&test) };

        match test {
            MultiVariant::One(ref s) => assert!(GcAllocator::is_managed(s.0.as_ref())),
            _ => assert!(false),
        }
    }

    // Test a tuple
    {
        let test = (Box::new(123), Box::new(456));

        assert!(!GcAllocator::is_managed(test.0.as_ref()));
        assert!(!GcAllocator::is_managed(test.1.as_ref()));

        unsafe { make_collectable(&test) };

        assert!(GcAllocator::is_managed(test.0.as_ref()));
        assert!(GcAllocator::is_managed(test.1.as_ref()));
    }

    // Test an array
    {
        let test = [Box::new(123), Box::new(456)];

        assert!(!GcAllocator::is_managed(test[0].as_ref()));
        assert!(!GcAllocator::is_managed(test[1].as_ref()));

        unsafe { make_collectable(&test) };

        assert!(GcAllocator::is_managed(test[0].as_ref()));
        assert!(GcAllocator::is_managed(test[1].as_ref()));
    }

    // Test a slice
    {
        let test = &[Box::new(123), Box::new(456)];

        assert!(!GcAllocator::is_managed(test[0].as_ref()));
        assert!(!GcAllocator::is_managed(test[1].as_ref()));

        unsafe { make_collectable(&test) };

        assert!(GcAllocator::is_managed(test[0].as_ref()));
        assert!(GcAllocator::is_managed(test[1].as_ref()));
    }

    // Test a trait object
    {

        struct Concrete(Box<usize>);

        trait Virtual {
            fn get_underlying_heap_addr(&self) -> *const u8;
        }

        impl Virtual for Concrete {
            fn get_underlying_heap_addr(&self) -> *const u8 {
                self.0.as_ref() as *const _ as *const u8
            }
        }

        unsafe impl<'a> std::gc::Collectable for dyn Virtual + 'a {
            unsafe fn set_collectable(&self) {
                std::alloc::set_managed(self.get_underlying_heap_addr() as *mut u8)
            }
        }

        let test = &Concrete(Box::new(123)) as &dyn Virtual;

        assert!(!GcAllocator::is_managed(test.get_underlying_heap_addr()));

        unsafe { make_collectable(&test) };

        assert!(GcAllocator::is_managed(test.get_underlying_heap_addr()));
    }
}
