#![feature(gc)]
#![feature(negative_impls)]

use std::gc::GcSmartPointer;
use std::gc::NoFinalize;

#[derive(Clone, Copy)]
struct Finalizable(usize);

struct NonFinalizable(usize);

unsafe impl GcSmartPointer for Finalizable {}

impl !NoFinalize for Finalizable {}

// EMIT_MIR prevent_early_finalization.preserve_locals.PreventEarlyFinalization.diff
fn preserve_locals() {
    let gc = Finalizable(123);
}

// EMIT_MIR prevent_early_finalization.preserve_multiple_locals.PreventEarlyFinalization.diff
fn preserve_multiple_locals() {
    let gc1 = Finalizable(123);
    let gc2 = Finalizable(123);
}

// EMIT_MIR prevent_early_finalization.preserve_args.PreventEarlyFinalization.diff
fn preserve_args() {
    let ret = preserve_args_inner(Finalizable(123), Finalizable(456));
}

// EMIT_MIR prevent_early_finalization.preserve_args_inner.PreventEarlyFinalization.diff
fn preserve_args_inner(x: Finalizable, y: Finalizable) -> Finalizable {
    Finalizable(x.0 + y.0)
}

// EMIT_MIR prevent_early_finalization.preserve_args_inl.Inline.after.mir
fn preserve_args_inl() -> Finalizable {
    let ret = preserve_args_inl_inner(Finalizable(123), Finalizable(456));
    let mut x = Finalizable(ret.0);
    for i in 1..100 {
        x.0 += i;
    }
    x
}

#[inline]
fn preserve_args_inl_inner(x: Finalizable, y: Finalizable) -> Finalizable {
    Finalizable(x.0 + y.0)
}

fn main() {
    preserve_locals();
    preserve_args_inl();
}
