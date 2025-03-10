//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
use std::gc::Gc;

static BOX_TRIVIAL: bool = needs_finalizer::<Box<usize>>();
static BOX_FINALIZABLE: bool = needs_finalizer::<Box<HasDrop>>();
static BOX_UNFINALIZABLE: bool = needs_finalizer::<Box<HasDropNoFinalize>>();
static BOX_TUPLE_FINALIZABLE: bool = needs_finalizer::<Box<(HasDrop, HasDrop)>>();
static BOX_TUPLE_UNFINALIZABLE: bool = needs_finalizer::<Box<(HasDropNoFinalize, HasDropNoFinalize)>>();
static BOX_TUPLE_GC_UNFINALIZABLE: bool = needs_finalizer::<Box<(HasDropNoFinalize, Gc<HasDrop>)>>();
static BOX_TUPLE_GC_FINALIZABLE: bool = needs_finalizer::<Box<(HasDrop, Gc<HasDrop>)>>();
static BOX_COLLECTABLE_NO_DROP_ELEMENT: bool = needs_finalizer::<Box<NonAnnotated>>();

fn main() {
    assert!(!BOX_TRIVIAL);
    assert!(BOX_FINALIZABLE);
    assert!(!BOX_UNFINALIZABLE);
    assert!(BOX_TUPLE_FINALIZABLE);
    assert!(!BOX_TUPLE_UNFINALIZABLE);
    assert!(!BOX_TUPLE_GC_UNFINALIZABLE);
    assert!(BOX_TUPLE_GC_FINALIZABLE);
    assert!(!BOX_COLLECTABLE_NO_DROP_ELEMENT);
}
