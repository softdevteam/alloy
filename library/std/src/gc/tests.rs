use std::alloc::{GlobalAlloc, Layout};

use super::*;

#[test]
fn test_dispatchable() {
    struct S1 {
        x: u64,
    }
    struct S2 {
        y: u64,
    }
    trait T: Send {
        fn f(&self) -> u64;
    }
    impl T for S1 {
        fn f(&self) -> u64 {
            self.x
        }
    }
    impl T for S2 {
        fn f(&self) -> u64 {
            self.y
        }
    }

    let s1 = S1 { x: 1 };
    let s2 = S2 { y: 2 };
    let s1gc: Gc<S1> = Gc::new(s1);
    let s2gc: Gc<S2> = Gc::new(s2);
    assert_eq!(s1gc.f(), 1);
    assert_eq!(s2gc.f(), 2);

    let s1gcd: Gc<dyn T> = s1gc;
    let s2gcd: Gc<dyn T> = s2gc;
    assert_eq!(s1gcd.f(), 1);
    assert_eq!(s2gcd.f(), 2);
}

#[repr(align(1024))]
struct S(u8);

#[repr(align(16))]
struct T(usize, usize, usize, usize);

#[test]
fn large_alignment() {
    let x = Box::new_in(S(123), GcAllocator);
    let ptr = Box::into_raw(x);
    assert!(!ptr.is_null());
    assert!(ptr.is_aligned());

    // When this is freed, GC assertions will check if the base pointer can be
    // correctly recovered.
    unsafe {
        let _ = Box::from_raw_in(ptr, GcAllocator);
    }

    let y = Box::new_in(T(1, 2, 3, 4), GcAllocator);
    let ptr = Box::into_raw(y);
    assert!(!ptr.is_null());
    assert!(ptr.is_aligned());

    unsafe {
        let _ = Box::from_raw_in(ptr, GcAllocator);
    }
}

#[test]
fn bdwgc_issue_589() {
    // Test the specific size / alignment problem raised in [1].
    //
    // [1]: https://github.com/ivmai/bdwgc/issues/589
    unsafe {
        let allocator = GcAllocator;
        let layout = Layout::from_size_align_unchecked(512, 64);
        let raw_ptr = GlobalAlloc::alloc(&allocator, layout);
        GlobalAlloc::dealloc(&allocator, raw_ptr, layout);
    }
}
#[test]
fn test_unsized() {
    let foo: Gc<[i32]> = Gc::new([1, 2, 3]);
    assert_eq!(foo, foo.clone());
}

#[test]
fn test_from_box() {
    let b: Box<u32> = Box::new(123);
    let g: Gc<u32> = Gc::from(b);

    assert_eq!(*g, 123);
}

#[test]
fn test_from_box_trait() {
    use crate::fmt::Display;
    use crate::string::ToString;

    let b: Box<dyn Display> = Box::new(123);
    let g: Gc<dyn Display> = Gc::from(b);

    assert_eq!(g.to_string(), "123");
}

#[test]
fn test_from_box_trait_zero_sized() {
    use crate::fmt::Debug;

    let b: Box<dyn Debug> = Box::new(());
    let g: Gc<dyn Debug> = Gc::from(b);

    assert_eq!(format!("{g:?}"), "()");
}
