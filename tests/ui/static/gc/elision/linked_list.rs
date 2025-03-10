//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
use std::collections::LinkedList;
use std::gc::Gc;

static LL_TRIVIAL: bool = needs_finalizer::<LinkedList<usize>>();
static LL_FINALIZABLE: bool = needs_finalizer::<LinkedList<HasDrop>>();
static LL_UNFINALIZABLE: bool = needs_finalizer::<LinkedList<HasDropNoFinalize>>();
static LL_TUPLE_UNFINALIZABLE: bool = needs_finalizer::<LinkedList<(HasDropNoFinalize, usize)>>();
static LL_TUPLE_FINALIZABLE: bool = needs_finalizer::<LinkedList<(HasDrop, HasDrop)>>();
static LL_TUPLE_CONTAINS_FINALIZABLE: bool = needs_finalizer::<LinkedList<(HasDrop, usize)>>();
static LL_LL_FINALIZABLE: bool = needs_finalizer::<LinkedList<LinkedList<HasDrop>>>();
static LL_LL_UNFINALIZABLE: bool = needs_finalizer::<LinkedList<LinkedList<HasDropNoFinalize>>>();
static LL_STRING: bool = needs_finalizer::<LinkedList<String>>();
static LL_BOX_FINALIZABLE: bool = needs_finalizer::<LinkedList<Box<HasDrop>>>();
static LL_BOX_UNFINALIZABLE: bool = needs_finalizer::<LinkedList<Box<HasDropNoFinalize>>>();
static LL_TUPLE_GC_UNFINALIZABLE: bool = needs_finalizer::<LinkedList<(HasDropNoFinalize, Gc<HasDrop>)>>();
static LL_TUPLE_GC_FINALIZABLE: bool = needs_finalizer::<LinkedList<(HasDrop, Gc<HasDrop>)>>();
static LL_COLLECTABLE_NO_DROP_ELEMENT: bool = needs_finalizer::<LinkedList<NonAnnotated>>();

fn main() {
    assert!(!LL_TRIVIAL);
    assert!(LL_FINALIZABLE);
    assert!(!LL_UNFINALIZABLE);
    assert!(!LL_TUPLE_UNFINALIZABLE);
    assert!(LL_TUPLE_FINALIZABLE);
    assert!(LL_TUPLE_CONTAINS_FINALIZABLE);
    assert!(LL_LL_FINALIZABLE);
    assert!(!LL_LL_UNFINALIZABLE);
    assert!(!LL_STRING);
    assert!(LL_BOX_FINALIZABLE);
    assert!(!LL_BOX_UNFINALIZABLE);
    assert!(!LL_TUPLE_GC_UNFINALIZABLE);
    assert!(LL_TUPLE_GC_FINALIZABLE);
    assert!(!LL_COLLECTABLE_NO_DROP_ELEMENT);
}
