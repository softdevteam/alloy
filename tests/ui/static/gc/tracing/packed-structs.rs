#![deny(misaligned_gc_pointers)]
#![feature(gc)]

struct RawPtr(*mut u8);

#[repr(packed)]
struct PackedUsize { //~ ERROR: contains a non-word aligned field `usize`
    x: u16,
    y: usize,
}

#[repr(packed)]
struct PackedUsizeAligned {
    x: u16,
    y: u16,
    z: u32,
    a: usize,
}

#[repr(packed)]
struct PackedI64 { //~ ERROR: contains a non-word aligned field `i64`
    x: u16,
    y: i64,
}

#[repr(packed)]
struct PackedU64 { //~ ERROR: contains a non-word aligned field `u64`
    x: u16,
    y: u64,
}

#[repr(packed)]
struct PackedRef<'a> { //~ ERROR: contains a non-word aligned field `&'a usize`
    x: u16,
    y: &'a usize,
}

#[repr(packed)]
struct PackedRefAligned<'a> {
    x: u16,
    y: u16,
    z: u32,
    a: &'a usize,
}

#[repr(packed)]
struct PackedRaw { //~ ERROR: contains a non-word aligned field `*mut usize`
    x: u16,
    y: *mut usize,
}

#[repr(packed)]
struct PackedRawAligned {
    x: u16,
    y: u16,
    z: u32,
    a: *mut usize,
}

#[repr(packed)]
struct PackedTuple { //~ ERROR: contains a non-word aligned field `usize`
    x: u16,
    y: (usize, u8),
}

#[repr(packed)]
struct PackedArray { //~ ERROR: contains a non-word aligned field `usize`
    x: u16,
    y: [usize; 3],
}

#[repr(packed)]
struct PackedArrayOffset { //~ ERROR: contains a non-word aligned field `u64`
    x: usize,
    y: [u16; 3],
    z: u64
}

#[repr(packed)]
struct PackedAdt { //~ ERROR: contains a non-word aligned field `*mut u8`
    x: u16,
    y: RawPtr,
}

fn main() {}
