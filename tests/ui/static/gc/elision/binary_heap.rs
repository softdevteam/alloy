//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
use std::collections::BinaryHeap;
use std::gc::Gc;

static BH_TRIVIAL: bool = needs_finalizer::<BinaryHeap<usize>>();
static BH_FINALIZABLE: bool = needs_finalizer::<BinaryHeap<HasDrop>>();
static BH_UNFINALIZABLE: bool = needs_finalizer::<BinaryHeap<HasDropNoFinalize>>();
static BH_TUPLE_UNFINALIZABLE: bool = needs_finalizer::<BinaryHeap<(HasDropNoFinalize, usize)>>();
static BH_TUPLE_FINALIZABLE: bool = needs_finalizer::<BinaryHeap<(HasDrop, HasDrop)>>();
static BH_TUPLE_CONTAINS_FINALIZABLE: bool = needs_finalizer::<BinaryHeap<(HasDrop, usize)>>();
static BH_BH_FINALIZABLE: bool = needs_finalizer::<BinaryHeap<BinaryHeap<HasDrop>>>();
static BH_BH_UNFINALIZABLE: bool = needs_finalizer::<BinaryHeap<BinaryHeap<HasDropNoFinalize>>>();
static BH_STRING: bool = needs_finalizer::<BinaryHeap<String>>();
static BH_BOX_FINALIZABLE: bool = needs_finalizer::<BinaryHeap<Box<HasDrop>>>();
static BH_BOX_UNFINALIZABLE: bool = needs_finalizer::<BinaryHeap<Box<HasDropNoFinalize>>>();
static BH_TUPLE_GC_UNFINALIZABLE: bool = needs_finalizer::<BinaryHeap<(HasDropNoFinalize, Gc<HasDrop>)>>();
static BH_TUPLE_GC_FINALIZABLE: bool = needs_finalizer::<BinaryHeap<(HasDrop, Gc<HasDrop>)>>();
static BH_COLLECTABLE_NO_DROP_ELEMENT: bool = needs_finalizer::<BinaryHeap<NonAnnotated>>();

fn main() {
    assert!(!BH_TRIVIAL);
    assert!(BH_FINALIZABLE);
    assert!(!BH_UNFINALIZABLE);
    assert!(!BH_TUPLE_UNFINALIZABLE);
    assert!(BH_TUPLE_FINALIZABLE);
    assert!(BH_TUPLE_CONTAINS_FINALIZABLE);
    assert!(BH_BH_FINALIZABLE);
    assert!(!BH_BH_UNFINALIZABLE);
    assert!(!BH_STRING);
    assert!(BH_BOX_FINALIZABLE);
    assert!(!BH_BOX_UNFINALIZABLE);
    assert!(!BH_TUPLE_GC_UNFINALIZABLE);
    assert!(BH_TUPLE_GC_FINALIZABLE);
    assert!(!BH_COLLECTABLE_NO_DROP_ELEMENT);
}
