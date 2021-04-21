use crate::{LateContext, LateLintPass, LintContext};
use rustc_attr as attr;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_middle::ty;

declare_lint! {
    /// The `misaligned_gc_pointers` lint checks that packed structs have no
    /// traceable fields.
    ///
    /// ### Example
    ///
    /// ```rust,compile_fail
    /// #[repr(packed)]
    /// struct S(u8, *mut u8);
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// Packed structs have a min alignment of 1 byte, so fields which need to
    /// be traced by the GC --  such as `*mut u8` -- are not guaranteed to be
    /// word-aligned. This can cause fields to be missed by the collector and is
    /// undefined behaviour.
    pub MISALIGNED_GC_POINTERS,
    Deny,
    "packed structs should not contain traceable values"
}

declare_lint_pass!(MisalignedGcPointers => [MISALIGNED_GC_POINTERS]);

impl<'tcx> LateLintPass<'tcx> for MisalignedGcPointers {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &hir::Item<'_>) {
        for attr in cx.tcx.hir().attrs(item.hir_id()) {
            for r in attr::find_repr_attrs(&cx.tcx.sess, attr) {
                if let attr::ReprPacked(_) = r {
                    let def_id = cx.tcx.hir().local_def_id(item.hir_id()).to_def_id();
                    let ty = cx.tcx.type_of(def_id);
                    let param_env = ty::ParamEnv::empty();

                    if ty.is_conservative(cx.tcx.at(item.span), param_env) {
                        let msg = "packed structs must not implement the `Conservative` trait.";
                        cx.struct_span_lint(MISALIGNED_GC_POINTERS, item.span, |lint| {
                            lint.build(msg)
                                .span_suggestion_short(
                                    attr.span,
                                    "remove this attribute",
                                    String::new(),
                                    Applicability::MachineApplicable,
                                )
                                .emit()
                        });
                    } else if !ty.is_no_trace(cx.tcx.at(item.span), param_env) {
                        let msg = "packed structs must implement the `NoTrace` trait.";
                        cx.struct_span_lint(MISALIGNED_GC_POINTERS, item.span, |lint| {
                            lint.build(msg)
                                .span_suggestion_short(
                                    attr.span,
                                    "remove this attribute",
                                    String::new(),
                                    Applicability::MachineApplicable,
                                )
                                .emit()
                        });
                    }
                    return;
                }
            }
        }
    }
}
