//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
use std::gc::Gc;

static VEC_TRIVIAL: bool = needs_finalizer::<Vec<usize>>();
static VEC_FINALIZABLE: bool = needs_finalizer::<Vec<HasDrop>>();
static VEC_UNFINALIZABLE: bool = needs_finalizer::<Vec<HasDropNoFinalize>>();
static VEC_TUPLE_UNFINALIZABLE: bool = needs_finalizer::<Vec<(HasDropNoFinalize, usize)>>();
static VEC_TUPLE_FINALIZABLE: bool = needs_finalizer::<Vec<(HasDrop, HasDrop)>>();
static VEC_TUPLE_CONTAINS_FINALIZABLE: bool = needs_finalizer::<Vec<(HasDrop, usize)>>();
static VEC_VEC_FINALIZABLE: bool = needs_finalizer::<Vec<Vec<HasDrop>>>();
static VEC_VEC_UNFINALIZABLE: bool = needs_finalizer::<Vec<Vec<HasDropNoFinalize>>>();
static VEC_STRING: bool = needs_finalizer::<Vec<String>>();
static VEC_BOX_FINALIZABLE: bool = needs_finalizer::<Vec<Box<HasDrop>>>();
static VEC_BOX_UNFINALIZABLE: bool = needs_finalizer::<Vec<Box<HasDropNoFinalize>>>();
static VEC_TUPLE_GC_UNFINALIZABLE: bool = needs_finalizer::<Vec<(HasDropNoFinalize, Gc<HasDrop>)>>();
static VEC_TUPLE_GC_FINALIZABLE: bool = needs_finalizer::<Vec<(HasDrop, Gc<HasDrop>)>>();
static VEC_COLLECTABLE_NO_DROP_ELEMENT: bool = needs_finalizer::<Vec<NonAnnotated>>();

fn main() {
    assert!(!VEC_TRIVIAL);
    assert!(VEC_FINALIZABLE);
    assert!(!VEC_UNFINALIZABLE);
    assert!(!VEC_TUPLE_UNFINALIZABLE);
    assert!(VEC_TUPLE_FINALIZABLE);
    assert!(VEC_TUPLE_CONTAINS_FINALIZABLE);
    assert!(VEC_VEC_FINALIZABLE);
    assert!(!VEC_VEC_UNFINALIZABLE);
    assert!(!VEC_STRING);
    assert!(VEC_BOX_FINALIZABLE);
    assert!(!VEC_BOX_UNFINALIZABLE);
    assert!(!VEC_TUPLE_GC_UNFINALIZABLE);
    assert!(VEC_TUPLE_GC_FINALIZABLE);
    assert!(!VEC_COLLECTABLE_NO_DROP_ELEMENT);
}
