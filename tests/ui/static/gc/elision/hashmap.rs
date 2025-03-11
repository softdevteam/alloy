//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
use std::collections::HashMap;
use std::gc::Gc;

static HM_TRIVIAL: bool = needs_finalizer::<HashMap<usize, usize>>();
static HM_FIN_KEY: bool = needs_finalizer::<HashMap<HasDrop, HasDropNoFinalize>>();
static HM_FIN_VAL: bool = needs_finalizer::<HashMap<HasDropNoFinalize, HasDrop>>();
static HM_FIN_BOTH: bool = needs_finalizer::<HashMap<HasDrop, HasDrop>>();
static HM_FIN_GC_KEY: bool = needs_finalizer::<HashMap<Gc<HasDropNoFinalize>, Gc<HasDrop>>>();
static HM_FIN_GC_VAL: bool = needs_finalizer::<HashMap<Gc<HasDropNoFinalize>, Gc<HasDrop>>>();
static HM_FIN_GC_BOTH: bool = needs_finalizer::<HashMap<Gc<HasDrop>, Gc<HasDrop>>>();

fn main() {
    assert!(!HM_TRIVIAL);
    assert!(HM_FIN_KEY);
    assert!(HM_FIN_VAL);
    assert!(HM_FIN_BOTH);
    assert!(!HM_FIN_GC_KEY);
    assert!(!HM_FIN_GC_VAL);
    assert!(!HM_FIN_GC_BOTH);
}
