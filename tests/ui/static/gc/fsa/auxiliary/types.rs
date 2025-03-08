#[inline(never)]
fn use_val<T: std::fmt::Debug>(x: T) {
    dbg!("{:?}", x);
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
