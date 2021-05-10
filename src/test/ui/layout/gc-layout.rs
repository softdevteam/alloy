// run-pass
// ignore-tidy-linelength

#![feature(gc)]
#![feature(negative_impls)]

#![allow(dead_code)]
#![allow(unused_attributes)]
#![allow(unused_imports)]

use std::gc::{gc_layout, NoTrace, Trace};
use std::mem::size_of;
use std::mem::align_of;

struct TraceUsize(usize);
struct TraceU64(usize);

impl !NoTrace for TraceUsize {}
impl !NoTrace for TraceU64 {}

struct SmallAlign {
    a: u8,
    b: u16,
    c: u8
}

struct ShuffledFields {
    a: u8,
    b: TraceU64,
    c: u8
}

struct ShuffledFieldsNoTrace {
    a: u8,
    #[no_trace]
    b: TraceU64,
    c: u8
}

#[repr(C)]
struct FixedFields {
    a: u8,
    b: TraceU64,
    c: u8
}

#[repr(C)]
struct FixedFieldsNoTrace {
    a: u8,
    #[no_trace]
    b: TraceU64,
    c: u8
}

struct MultiFields {
    a: u8,
    b: u16,
    e: TraceUsize,
    c: u32,
    d: u8,
    #[no_trace]
    f: TraceUsize
}

struct BigAlign {
    a: MultiFields,
    b: TraceUsize,
    c: TraceUsize,
}

struct BigAlignNoTrace {
    b: TraceUsize,
    c: TraceUsize,
    #[no_trace]
    a: MultiFields,
}

struct NoTraceInner {
    a: u16,
    b: u16,
    c: u16,
    d: u16,
}

struct NoTraceOuter {
    a: NoTraceInner,
    b: TraceUsize,
    c: NoTraceInner
}

struct NoTraceOuter2 {
    a: NoTraceInner,
    c: NoTraceInner
}

struct ZST {}

fn main() {
    assert_eq!(size_of::<SmallAlign>(), 4);
    assert_eq!(size_of::<ShuffledFields>(), 16);
    assert_eq!(size_of::<ShuffledFieldsNoTrace>(), 16);
    assert_eq!(size_of::<FixedFields>(), 24);
    assert_eq!(size_of::<MultiFields>(), 24);
    assert_eq!(size_of::<BigAlign>(), 40);
    assert_eq!(size_of::<BigAlignNoTrace>(), 40);

    unsafe {
    assert_eq!(gc_layout::<SmallAlign>(),              Trace { bitmap: 0x0000000000000000, size: 0 });
    assert_eq!(gc_layout::<ShuffledFields>(),          Trace { bitmap: 0xFFFFFFFFFFFFFFFD, size: 2 });
    assert_eq!(gc_layout::<ShuffledFieldsNoTrace>(),   Trace { bitmap: 0xFFFFFFFFFFFFFFFC, size: 2 });
    assert_eq!(gc_layout::<FixedFields>(),             Trace { bitmap: 0xFFFFFFFFFFFFFFFA, size: 3 });
    assert_eq!(gc_layout::<FixedFieldsNoTrace>(),      Trace { bitmap: 0xFFFFFFFFFFFFFFF8, size: 3 });
    assert_eq!(gc_layout::<MultiFields>(),             Trace { bitmap: 0xFFFFFFFFFFFFFFF9, size: 3 });
    assert_eq!(gc_layout::<BigAlign>(),                Trace { bitmap: 0xFFFFFFFFFFFFFFFF, size: 5 });
    assert_eq!(gc_layout::<BigAlignNoTrace>(),         Trace { bitmap: 0xFFFFFFFFFFFFFFE3, size: 5 });

    assert_eq!(gc_layout::<NoTraceInner>(),            Trace { bitmap: 0x0000000000000000, size: 0 });
    assert_eq!(gc_layout::<NoTraceOuter>(),            Trace { bitmap: 0xFFFFFFFFFFFFFFF9, size: 3 });
    assert_eq!(gc_layout::<NoTraceOuter2>(),           Trace { bitmap: 0x0000000000000000, size: 0 });

    assert_eq!(gc_layout::<ZST>(),                     Trace { bitmap: 0x0000000000000000, size: 0 });

    }
}
