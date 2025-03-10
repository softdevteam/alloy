//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
use std::collections::VecDeque;
use std::gc::Gc;

static VD_TRIVIAL: bool = needs_finalizer::<VecDeque<usize>>();
static VD_FINALIZABLE: bool = needs_finalizer::<VecDeque<HasDrop>>();
static VD_UNFINALIZABLE: bool = needs_finalizer::<VecDeque<HasDropNoFinalize>>();
static VD_TUPLE_UNFINALIZABLE: bool = needs_finalizer::<VecDeque<(HasDropNoFinalize, usize)>>();
static VD_TUPLE_FINALIZABLE: bool = needs_finalizer::<VecDeque<(HasDrop, HasDrop)>>();
static VD_TUPLE_CONTAINS_FINALIZABLE: bool = needs_finalizer::<VecDeque<(HasDrop, usize)>>();
static VD_VD_FINALIZABLE: bool = needs_finalizer::<VecDeque<VecDeque<HasDrop>>>();
static VD_VD_UNFINALIZABLE: bool = needs_finalizer::<VecDeque<VecDeque<HasDropNoFinalize>>>();
static VD_STRING: bool = needs_finalizer::<VecDeque<String>>();
static VD_BOX_FINALIZABLE: bool = needs_finalizer::<VecDeque<Box<HasDrop>>>();
static VD_BOX_UNFINALIZABLE: bool = needs_finalizer::<VecDeque<Box<HasDropNoFinalize>>>();
static VD_TUPLE_GC_UNFINALIZABLE: bool = needs_finalizer::<VecDeque<(HasDropNoFinalize, Gc<HasDrop>)>>();
static VD_TUPLE_GC_FINALIZABLE: bool = needs_finalizer::<VecDeque<(HasDrop, Gc<HasDrop>)>>();
static VD_COLLECTABLE_NO_DROP_ELEMENT: bool = needs_finalizer::<VecDeque<NonAnnotated>>();

fn main() {
    assert!(!VD_TRIVIAL);
    assert!(VD_FINALIZABLE);
    assert!(!VD_UNFINALIZABLE);
    assert!(!VD_TUPLE_UNFINALIZABLE);
    assert!(VD_TUPLE_FINALIZABLE);
    assert!(VD_TUPLE_CONTAINS_FINALIZABLE);
    assert!(VD_VD_FINALIZABLE);
    assert!(!VD_VD_UNFINALIZABLE);
    assert!(!VD_STRING);
    assert!(VD_BOX_FINALIZABLE);
    assert!(!VD_BOX_UNFINALIZABLE);
    assert!(!VD_TUPLE_GC_UNFINALIZABLE);
    assert!(VD_TUPLE_GC_FINALIZABLE);
    assert!(!VD_COLLECTABLE_NO_DROP_ELEMENT);
}
