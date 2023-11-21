use std::gc::GcAllocator;

#[repr(align(1024))]
struct S(u8);

#[test]
fn large_alignment() {
    let x = Box::new_in(S(123), GcAllocator);
    let ptr = Box::into_raw(x);
    assert!(!ptr.is_null());
    assert!(ptr.is_aligned());
}
