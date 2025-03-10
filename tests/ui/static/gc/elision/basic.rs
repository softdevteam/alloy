//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
use std::gc::Gc;
use std::gc::NonFinalizable;

const CONST_U8: bool = needs_finalizer::<u8>();
const CONST_STRING: bool = needs_finalizer::<String>();
const CONST_FINALIZABLE: bool = needs_finalizer::<HasDrop>();
const CONST_UNFINALIZABLE: bool = needs_finalizer::<HasDropNoFinalize>();
const CONST_TUPLE_UNFINALIZABLE: bool = needs_finalizer::<(HasDropNoFinalize,)>();
static STATIC_U8: bool = needs_finalizer::<u8>();
static STATIC_STRING: bool = needs_finalizer::<String>();
static STATIC_FINALIZABLE: bool = needs_finalizer::<HasDrop>();
static STATIC_UNFINALIZABLE: bool = needs_finalizer::<HasDropNoFinalize>();
static STATIC_MAYBE_FINALIZE_NO_COMPONENTS: bool = needs_finalizer::<MaybeFinalize<ExplicitNoFinalize>>();
static STATIC_MAYBE_FINALIZE_DROP_COMPONENTS: bool = needs_finalizer::<MaybeFinalize<HasDrop>>();
static NESTED_GC: bool = needs_finalizer::<Box<Gc<HasDrop>>>();
static NESTED_GC_NO_FINALIZE: bool = needs_finalizer::<Box<Gc<NonAnnotated>>>();

static NON_FINALIZABLE: bool = needs_finalizer::<NonFinalizable<HasDrop>>();
static NON_FINALIZABLE_NESTED: bool = needs_finalizer::<MaybeFinalize<NonFinalizable<HasDrop>>>();
static OUTER_NEEDS_FINALIZING: bool = needs_finalizer::<FinalizedContainer<Vec<HasDropNoFinalize>>>();

fn main() {
    assert!(!CONST_U8);
    assert!(!CONST_STRING);
    assert!(CONST_FINALIZABLE);
    assert!(!CONST_UNFINALIZABLE);
    assert!(!CONST_TUPLE_UNFINALIZABLE);
    assert!(!STATIC_U8);
    assert!(!STATIC_STRING);
    assert!(STATIC_FINALIZABLE);
    assert!(!STATIC_UNFINALIZABLE);
    assert!(!STATIC_MAYBE_FINALIZE_NO_COMPONENTS);
    assert!(STATIC_MAYBE_FINALIZE_DROP_COMPONENTS);
    assert!(!NESTED_GC);
    assert!(!NESTED_GC_NO_FINALIZE);
    assert!(!NON_FINALIZABLE);
    assert!(!NON_FINALIZABLE_NESTED);
    assert!(OUTER_NEEDS_FINALIZING);
}
