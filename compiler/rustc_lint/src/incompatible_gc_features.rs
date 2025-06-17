use rustc_hir as hir;
use rustc_session::{declare_lint, declare_lint_pass};
use rustc_span::sym;

use crate::lints::UntrackedHeapAllocation as AllocationLint;
use crate::{LateContext, LateLintPass, LintContext};

declare_lint! {
    /// The `untracked_heap_allocation` lint checks that heap allocations use the BDWGC allocator.
    ///
    /// ### Example
    /// ```rust,compile_fail
    /// #![feature(allocator_api)]
    /// use std::rc::Rc;
    /// use std::alloc::System;
    ///
    /// static A: System = System;
    ///
    /// fn foo() { Rc::new_in(123, A); }
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// Alloy must be able to track all heap allocations in order to correctly trace GC objects.
    pub UNTRACKED_HEAP_ALLOCATION,
    Deny,
    "Allocations must go through the BDWGC allocator",
}

declare_lint_pass!(UntrackedHeapAllocation => [UNTRACKED_HEAP_ALLOCATION]);

impl<'tcx> LateLintPass<'tcx> for UntrackedHeapAllocation {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        if let hir::ExprKind::Call(ref callee, _) = expr.kind {
            if let hir::ExprKind::Path(ref qpath) = callee.kind {
                if let Some(def_id) = cx.qpath_res(qpath, callee.hir_id).opt_def_id() {
                    if let Some(_) = cx.tcx.get_attr(def_id, sym::rustc_alloc_in) {
                        cx.emit_span_lint(UNTRACKED_HEAP_ALLOCATION, expr.span, AllocationLint);
                    }
                }
            }
        }
    }
}
