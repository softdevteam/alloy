#![feature(gc)]
#![feature(negative_impls)]

use std::gc::Gc;

struct NeedsFinalize(usize);

impl Drop for NeedsFinalize {
    fn drop(&mut self) {}
}

// EMIT_MIR prevent_early_finalization.preserve_locals.PreventEarlyFinalization.diff
fn preserve_locals() {
    let gc = Gc::new(NeedsFinalize(123));
}

// EMIT_MIR prevent_early_finalization.preserve_multiple_locals.PreventEarlyFinalization.diff
fn preserve_multiple_locals() {
    let gc1 = Gc::new(NeedsFinalize(123));
    let gc2 = Gc::new(NeedsFinalize(123));
}

// EMIT_MIR prevent_early_finalization.preserve_args.PreventEarlyFinalization.diff
fn preserve_args() {
    let ret = preserve_args_inner(Gc::new(NeedsFinalize(123)), Gc::new(NeedsFinalize(456)));
}

// EMIT_MIR prevent_early_finalization.preserve_args_inner.PreventEarlyFinalization.diff
fn preserve_args_inner(x: Gc<NeedsFinalize>, y: Gc<NeedsFinalize>) -> Gc<NeedsFinalize> {
    Gc::new(NeedsFinalize(x.0 + y.0))
}

// EMIT_MIR prevent_early_finalization.preserve_args_inl.Inline.after.mir
fn preserve_args_inl() -> Gc<NeedsFinalize> {
    let ret = preserve_args_inl_inner(Gc::new(NeedsFinalize(123)), Gc::new(NeedsFinalize(456)));
    let x = Gc::new(NeedsFinalize(ret.0));
    x
}

#[inline]
fn preserve_args_inl_inner(x: Gc<NeedsFinalize>, y: Gc<NeedsFinalize>) -> Gc<NeedsFinalize> {
    Gc::new(NeedsFinalize(x.0 + y.0))
}

fn main() {
    preserve_locals();
    preserve_args_inl();
}
