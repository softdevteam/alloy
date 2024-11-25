use std::gc::Gc;
use std::fmt::Debug;

#[inline(never)]
fn use_val<T: std::fmt::Debug>(x: T) {
}

#[derive(Debug)]
struct HasNestedRef<'a> {
    a: &'a u64,
    b: u64,
    c: HasRef<'a>,
}

#[derive(Debug)]
struct HasRef<'a> {
    a: &'a u64,
    b: u64,
    c: [&'a u64; 2]
}

impl<'a> HasRef<'a> {
    #[inline(never)]
    fn new(a: &'a u64, b: u64, c: [&'a u64; 2]) -> Self {
        Self { a, b, c }
    }

    #[inline(always)]
    fn new_inlined(a: &'a u64, b: u64, c: [&'a u64; 2]) -> Self {
        Self { a, b, c }
    }
}

impl<'a> std::default::Default for HasRef<'a> {
    #[inline(never)]
    fn default() -> Self {
        Self { a: &1, b: 1, c: [&1, &2] }
    }
}

impl<'a> std::default::Default for HasNestedRef<'a> {
    #[inline(never)]
    fn default() -> Self {
        Self { a: &1, b: 1, c: HasRef::default() }
    }
}

#[derive(Debug)]
struct HasNestedGc {
    a: Gc<u64>,
    b: u64,
    c: HasGc,
}

#[derive(Debug)]
struct HasGc {
    a: Gc<u64>,
    b: u64,
    c: [Gc<u64>; 2]
}

impl std::default::Default for HasGc {
    #[inline(never)]
    fn default() -> Self {
        Self { a: Gc::new(1), b: 1, c: [Gc::new(1), Gc::new(2)] }
    }
}

impl<'a> std::default::Default for HasNestedGc {
    #[inline(never)]
    fn default() -> Self {
        Self { a: Gc::new(1), b: 1, c: HasGc::default() }
    }
}

#[derive(Debug)]
struct Wrapper<T: Debug>(T);

#[derive(Debug)]
struct U8Wrapper(u8);

#[derive(Debug)]
struct FinalizerUnsafeU8Wrapper(u8);
impl !Send for FinalizerUnsafeU8Wrapper {}

#[derive(Debug)]
struct FinalizerUnsafeWrapper<T: Debug>(T);
impl<T> !Send for FinalizerUnsafeWrapper<T> {}

#[derive(Debug)]
struct FinalizerUnsafeType(u8);
