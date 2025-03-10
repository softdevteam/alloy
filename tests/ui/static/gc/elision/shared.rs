//@ run-pass
// ignore-tidy-linelength
#![feature(gc)]
#![allow(dead_code)]
include!{"./auxiliary/types.rs"}

use std::mem::needs_finalizer;
// use std::gc::Gc;
use std::rc::Rc;
use std::cell::{Cell, Ref, RefMut, RefCell};
use std::sync::{Arc, Mutex, MutexGuard};

static RC: bool = needs_finalizer::<Rc<HasDrop>>();
static RC_NO_FINALIZE: bool = needs_finalizer::<Rc<HasDropNoFinalize>>();
static CELL: bool = needs_finalizer::<Cell<HasDrop>>();
static CELL_NO_FINALIZE: bool = needs_finalizer::<Cell<HasDropNoFinalize>>();
static REFCELL: bool = needs_finalizer::<RefCell<HasDrop>>();
static REFCELL_NO_FINALIZE: bool = needs_finalizer::<RefCell<HasDropNoFinalize>>();
static REF: bool = needs_finalizer::<Ref<HasDrop>>();
static REF_NO_FINALIZE: bool = needs_finalizer::<Ref<HasDropNoFinalize>>();
static REFMUT: bool = needs_finalizer::<RefMut<HasDrop>>();
static REFMUT_NO_FINALIZE: bool = needs_finalizer::<RefMut<HasDropNoFinalize>>();

static ARC: bool = needs_finalizer::<Arc<HasDrop>>();
static ARC_NO_FINALIZE: bool = needs_finalizer::<Arc<HasDropNoFinalize>>();
static MUTEX: bool = needs_finalizer::<Mutex<HasDrop>>();
static MUTEX_NO_FINALIZE: bool = needs_finalizer::<Mutex<HasDropNoFinalize>>();
static MUTEXGUARD: bool = needs_finalizer::<MutexGuard<HasDrop>>();
static MUTEXGUARD_NO_FINALIZE: bool = needs_finalizer::<MutexGuard<HasDropNoFinalize>>();

fn main() {
    assert!(RC);
    assert!(RC_NO_FINALIZE);
    assert!(CELL);
    assert!(!CELL_NO_FINALIZE);
    assert!(REFCELL);
    assert!(!REFCELL_NO_FINALIZE);
    assert!(REF);
    assert!(REF_NO_FINALIZE);
    assert!(REFMUT);
    assert!(REFMUT_NO_FINALIZE);

    assert!(ARC);
    assert!(ARC_NO_FINALIZE);
    assert!(MUTEX);
    assert!(!MUTEX_NO_FINALIZE);
    assert!(MUTEXGUARD);
    assert!(MUTEXGUARD_NO_FINALIZE);
}
