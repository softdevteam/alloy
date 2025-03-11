//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
use std::collections::HashSet;
use std::gc::Gc;

static HS_TRIVIAL: bool = needs_finalizer::<HashSet<usize>>();
static HS_FINALIZABLE: bool = needs_finalizer::<HashSet<HasDrop>>();
static HS_UNFINALIZABLE: bool = needs_finalizer::<HashSet<HasDropNoFinalize>>();
static HS_TUPLE_UNFINALIZABLE: bool = needs_finalizer::<HashSet<(HasDropNoFinalize, usize)>>();
static HS_TUPLE_FINALIZABLE: bool = needs_finalizer::<HashSet<(HasDrop, HasDrop)>>();
static HS_TUPLE_CONTAINS_FINALIZABLE: bool = needs_finalizer::<HashSet<(HasDrop, usize)>>();
static HS_HS_FINALIZABLE: bool = needs_finalizer::<HashSet<HashSet<HasDrop>>>();
static HS_HS_UNFINALIZABLE: bool = needs_finalizer::<HashSet<HashSet<HasDropNoFinalize>>>();
static HS_STRING: bool = needs_finalizer::<HashSet<String>>();
static HS_BOX_FINALIZABLE: bool = needs_finalizer::<HashSet<Box<HasDrop>>>();
static HS_BOX_UNFINALIZABLE: bool = needs_finalizer::<HashSet<Box<HasDropNoFinalize>>>();
static HS_TUPLE_GC_UNFINALIZABLE: bool = needs_finalizer::<HashSet<(HasDropNoFinalize, Gc<HasDrop>)>>();
static HS_TUPLE_GC_FINALIZABLE: bool = needs_finalizer::<HashSet<(HasDrop, Gc<HasDrop>)>>();
static HS_COLLECTABLE_NO_DROP_ELEMENT: bool = needs_finalizer::<HashSet<NonAnnotated>>();

fn main() {
    assert!(!HS_TRIVIAL);
    assert!(HS_FINALIZABLE);
    assert!(!HS_UNFINALIZABLE);
    assert!(!HS_TUPLE_UNFINALIZABLE);
    assert!(HS_TUPLE_FINALIZABLE);
    assert!(HS_TUPLE_CONTAINS_FINALIZABLE);
    assert!(HS_HS_FINALIZABLE);
    assert!(!HS_HS_UNFINALIZABLE);
    assert!(!HS_STRING);
    assert!(HS_BOX_FINALIZABLE);
    assert!(!HS_BOX_UNFINALIZABLE);
    assert!(!HS_TUPLE_GC_UNFINALIZABLE);
    assert!(HS_TUPLE_GC_FINALIZABLE);
    assert!(!HS_COLLECTABLE_NO_DROP_ELEMENT);
}
