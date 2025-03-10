//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
use std::collections::BTreeMap;
use std::gc::Gc;

static BTM_TRIVIAL: bool = needs_finalizer::<BTreeMap<usize, usize>>();
static BTM_FIN_KEY: bool = needs_finalizer::<BTreeMap<HasDrop, HasDropNoFinalize>>();
static BTM_FIN_VAL: bool = needs_finalizer::<BTreeMap<HasDropNoFinalize, HasDrop>>();
static BTM_FIN_BOTH: bool = needs_finalizer::<BTreeMap<HasDrop, HasDrop>>();
static BTM_FIN_GC_KEY: bool = needs_finalizer::<BTreeMap<Gc<HasDropNoFinalize>, Gc<HasDrop>>>();
static BTM_FIN_GC_VAL: bool = needs_finalizer::<BTreeMap<Gc<HasDropNoFinalize>, Gc<HasDrop>>>();
static BTM_FIN_GC_BOTH: bool = needs_finalizer::<BTreeMap<Gc<HasDrop>, Gc<HasDrop>>>();

fn main() {
    assert!(!BTM_TRIVIAL);
    assert!(BTM_FIN_KEY);
    assert!(BTM_FIN_VAL);
    assert!(BTM_FIN_BOTH);
    assert!(!BTM_FIN_GC_KEY);
    assert!(!BTM_FIN_GC_VAL);
    assert!(!BTM_FIN_GC_BOTH);
}
