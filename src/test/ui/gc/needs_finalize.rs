// run-pass
#![feature(gc)]

use std::mem;

struct NeedsDrop;

impl Drop for NeedsDrop {
    fn drop(&mut self) {}
}

struct Trivial(u8, f32);

struct NonTrivial(u8, NeedsDrop);

struct ExplicitNoFinalize(Trivial, NonTrivial);

struct NestedNoFinalize(Trivial, ExplicitNoFinalize);

impl core::gc::NoFinalize for ExplicitNoFinalize {}

const CONST_U8: bool = mem::needs_finalizer::<u8>();
const CONST_STRING: bool = mem::needs_finalizer::<String>();
const CONST_TRIVIAL: bool = mem::needs_finalizer::<Trivial>();
const CONST_NON_TRIVIAL: bool = mem::needs_finalizer::<NonTrivial>();

static STATIC_U8: bool = mem::needs_finalizer::<u8>();
static STATIC_STRING: bool = mem::needs_finalizer::<String>();
static STATIC_TRIVIAL: bool = mem::needs_finalizer::<Trivial>();
static STATIC_NON_TRIVIAL: bool = mem::needs_finalizer::<NonTrivial>();

static STATIC_EXPLICIT_NO_FINALIZE: bool = mem::needs_finalizer::<ExplicitNoFinalize>();
static STATIC_NESTED_NO_FINALIZE: bool = mem::needs_finalizer::<NestedNoFinalize>();

fn main() {
    assert!(!CONST_U8);
    assert!(!CONST_STRING);
    assert!(!CONST_TRIVIAL);
    assert!(CONST_NON_TRIVIAL);

    assert!(!STATIC_U8);
    assert!(!STATIC_STRING);
    assert!(!STATIC_TRIVIAL);
    assert!(STATIC_NON_TRIVIAL);

    assert!(!STATIC_EXPLICIT_NO_FINALIZE);
    assert!(!STATIC_NESTED_NO_FINALIZE);
}
