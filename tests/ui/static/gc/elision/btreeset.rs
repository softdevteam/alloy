//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
use std::collections::BTreeSet;
use std::gc::Gc;

static BT_TRIVIAL: bool = needs_finalizer::<BTreeSet<usize>>();
static BT_FINALIZABLE: bool = needs_finalizer::<BTreeSet<HasDrop>>();
static BT_UNFINALIZABLE: bool = needs_finalizer::<BTreeSet<HasDropNoFinalize>>();
static BT_TUPLE_UNFINALIZABLE: bool = needs_finalizer::<BTreeSet<(HasDropNoFinalize, usize)>>();
static BT_TUPLE_FINALIZABLE: bool = needs_finalizer::<BTreeSet<(HasDrop, HasDrop)>>();
static BT_TUPLE_CONTAINS_FINALIZABLE: bool = needs_finalizer::<BTreeSet<(HasDrop, usize)>>();
static BT_BT_FINALIZABLE: bool = needs_finalizer::<BTreeSet<BTreeSet<HasDrop>>>();
static BT_BT_UNFINALIZABLE: bool = needs_finalizer::<BTreeSet<BTreeSet<HasDropNoFinalize>>>();
static BT_STRING: bool = needs_finalizer::<BTreeSet<String>>();
static BT_BOX_FINALIZABLE: bool = needs_finalizer::<BTreeSet<Box<HasDrop>>>();
static BT_BOX_UNFINALIZABLE: bool = needs_finalizer::<BTreeSet<Box<HasDropNoFinalize>>>();
static BT_TUPLE_GC_UNFINALIZABLE: bool = needs_finalizer::<BTreeSet<(HasDropNoFinalize, Gc<HasDrop>)>>();
static BT_TUPLE_GC_FINALIZABLE: bool = needs_finalizer::<BTreeSet<(HasDrop, Gc<HasDrop>)>>();
static BT_COLLECTABLE_NO_DROP_ELEMENT: bool = needs_finalizer::<BTreeSet<NonAnnotated>>();

fn main() {
    assert!(!BT_TRIVIAL);
    assert!(BT_FINALIZABLE);
    assert!(!BT_UNFINALIZABLE);
    assert!(!BT_TUPLE_UNFINALIZABLE);
    assert!(BT_TUPLE_FINALIZABLE);
    assert!(BT_TUPLE_CONTAINS_FINALIZABLE);
    assert!(BT_BT_FINALIZABLE);
    assert!(!BT_BT_UNFINALIZABLE);
    assert!(!BT_STRING);
    assert!(BT_BOX_FINALIZABLE);
    assert!(!BT_BOX_UNFINALIZABLE);
    assert!(!BT_TUPLE_GC_UNFINALIZABLE);
    assert!(BT_TUPLE_GC_FINALIZABLE);
    assert!(!BT_COLLECTABLE_NO_DROP_ELEMENT);
}
