use crate::MirPass;
use rustc_hir::lang_items::LangItem;
use rustc_middle::mir::visit::PlaceContext;
use rustc_middle::mir::visit::Visitor;
use rustc_middle::mir::*;
use rustc_middle::ty::subst::InternalSubsts;
use rustc_middle::ty::{self, ParamEnv, Subst, Ty, TyCtxt};
use rustc_span::symbol::sym;
use rustc_span::{Span, DUMMY_SP};
use rustc_trait_selection::infer::{InferCtxtExt, TyCtxtInferExt};

#[derive(PartialEq)]
pub struct CheckFinalizers;

impl<'tcx> MirPass<'tcx> for CheckFinalizers {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        let ctor = tcx.get_diagnostic_item(sym::gc_ctor);

        if ctor.is_none() {
            return;
        }

        let ctor_did = ctor.unwrap();
        let param_env = tcx.param_env_reveal_all_normalized(body.source.def_id());

        for block in body.basic_blocks() {
            match &block.terminator {
                Some(Terminator { kind: TerminatorKind::Call { func, args, .. }, source_info }) => {
                    let func_ty = func.ty(body, tcx);
                    if let ty::FnDef(fn_did, ..) = func_ty.kind() {
                        if *fn_did == ctor_did {
                            let arg = match &args[0] {
                                Operand::Copy(place) | Operand::Move(place) => {
                                    body.local_decls()[place.local].source_info.span
                                }
                                Operand::Constant(con) => con.span,
                            };
                            let arg_ty = args[0].ty(body, tcx);

                            let mut finalizer_cx =
                                FinalizationCtxt { ctor: source_info.span, arg, tcx, param_env };
                            finalizer_cx.check_for_dangling_refs(arg_ty);
                            finalizer_cx.check(arg_ty);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

struct FinalizationCtxt<'tcx> {
    ctor: Span,
    arg: Span,
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
}

impl<'tcx> FinalizationCtxt<'tcx> {
    fn check_for_dangling_refs(&mut self, ty: Ty<'tcx>) {
        if !self.is_reference_free(ty) && self.tcx.needs_finalizer_raw(self.param_env.and(ty)) {
            let arg = self.tcx.sess.source_map().span_to_snippet(self.arg).unwrap();
            let mut err = self
                .tcx
                .sess
                .struct_span_err(self.arg, format!("`{arg}` cannot be safely constructed.",));
            err.span_label(
                self.arg,
                "contains a reference (&) which may no longer be valid when it is finalized.",
            );
            err.span_label(
                self.ctor,
                format!("`Gc::new` requires that a type is reference free.",),
            );
            err.emit();
        }
    }

    fn check(&mut self, ty: Ty<'tcx>) {
        if self.is_finalizer_safe(ty) || !self.tcx.needs_finalizer_raw(self.param_env.and(ty)) {
            return;
        }

        // We must now recurse through the `Ty`'s component types and search for
        // all the `Drop` impls. If we find any, we have to check that there are
        // no unsound projections into fields in their drop method body. More
        // specifically: if one of the drop methods dereferences a field which
        // is !FinalizerSafe, we must throw an error.
        match ty.kind() {
            ty::Infer(ty::FreshIntTy(_))
            | ty::Infer(ty::FreshFloatTy(_))
            | ty::Bool
            | ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Never
            | ty::FnDef(..)
            | ty::FnPtr(_)
            | ty::Char
            | ty::GeneratorWitness(..)
            | ty::RawPtr(_)
            | ty::Ref(..)
            | ty::Str
            | ty::Foreign(..) => {
                // None of these types can implement Drop.
                return;
            }
            ty::Dynamic(..) | ty::Error(..) => {
                // Dropping a trait object uses a virtual call, so we can't
                // work out which drop method to look at compile-time. This
                // means we must be more conservative and bail with an error
                // here, even if the drop impl itself would have been safe.
                self.emit_err();
            }
            ty::Slice(ty) => self.check(*ty),
            ty::Array(elem_ty, ..) => {
                self.check(*elem_ty);
            }
            ty::Tuple(fields) => {
                // Each tuple field must be individually checked for a `Drop`
                // impl.
                fields.iter().for_each(|f_ty| self.check(f_ty));
            }
            ty::Adt(def, substs) if !self.is_copy(ty) => {
                if def.has_dtor(self.tcx) {
                    if def.is_box() {
                        // This is a special case because Box has an empty drop
                        // method which is filled in later by the compiler.
                        self.emit_err();
                    }

                    let drop_trait = self.tcx.require_lang_item(LangItem::Drop, None);
                    let drop_fn = self.tcx.associated_item_def_ids(drop_trait)[0];
                    let substs = self.tcx.mk_substs_trait(ty, substs);
                    let instance = ty::Instance::resolve(self.tcx, self.param_env, drop_fn, substs)
                        .unwrap()
                        .unwrap();
                    let mir = self.tcx.instance_mir(instance.def);
                    let mut checker = ProjectionChecker { cx: self, body: mir };
                    checker.visit_body(&mir);
                }

                for field in def.all_fields() {
                    let field_ty = self.tcx.bound_type_of(field.did).subst(self.tcx, substs);
                    self.check(field_ty);
                }
            }
            _ => (),
        }
    }

    fn is_finalizer_safe(&self, ty: Ty<'tcx>) -> bool {
        self.tcx.infer_ctxt().enter(|infcx| {
            self.tcx.get_diagnostic_item(sym::FinalizerSafe).map(|t| {
                infcx
                    .type_implements_trait(t, ty, InternalSubsts::empty(), self.param_env)
                    .may_apply()
            }) == Some(true)
        })
    }

    fn is_reference_free(&self, ty: Ty<'tcx>) -> bool {
        self.tcx.infer_ctxt().enter(|infcx| {
            self.tcx.get_diagnostic_item(sym::ReferenceFree).map(|t| {
                infcx
                    .type_implements_trait(t, ty, InternalSubsts::empty(), self.param_env)
                    .may_apply()
            }) == Some(true)
        })
    }

    fn is_copy(&self, ty: Ty<'tcx>) -> bool {
        ty.is_copy_modulo_regions(self.tcx.at(DUMMY_SP), self.param_env)
    }

    fn is_gc(&self, ty: Ty<'tcx>) -> bool {
        if let ty::Adt(def, ..) = ty.kind() {
            if def.did() == self.tcx.get_diagnostic_item(sym::gc).unwrap() {
                return true;
            }
        }
        return false;
    }

    fn emit_err(&self) {
        let arg = self.tcx.sess.source_map().span_to_snippet(self.arg).unwrap();
        let mut err = self
            .tcx
            .sess
            .struct_span_err(self.arg, format!("`{arg}` cannot be safely finalized.",));
        err.span_label(self.arg, "has a drop method which cannot be safely finalized.");
        err.span_label(
            self.ctor,
            format!("`Gc::new` requires that it implements the `FinalizeSafe` trait.",),
        );
        err.help(format!("`Gc` runs finalizers on a separate thread, so `{arg}` must implement `FinalizeSafe` in order to be safely dropped.",));
        err.emit();
    }
}

struct ProjectionChecker<'a, 'tcx> {
    cx: &'a FinalizationCtxt<'tcx>,
    body: &'a Body<'tcx>,
}

impl<'a, 'tcx> ProjectionChecker<'a, 'tcx> {
    fn emit_err(&self, ty: Ty<'tcx>, span: Span) {
        let arg = self.cx.tcx.sess.source_map().span_to_snippet(self.cx.arg).unwrap();
        let mut err = self
            .cx
            .tcx
            .sess
            .struct_span_err(self.cx.arg, format!("`{arg}` cannot be safely finalized.",));
        if self.cx.is_gc(ty) {
            err.span_label(self.cx.arg, "has a drop method which cannot be safely finalized.");
            err.span_label(span, "caused by the expression here in `fn drop(&mut)` because");
            err.span_label(span, "it uses another `Gc` type.");
            err.help("`Gc` finalizers are unordered, so this field may have already been dropped. It is not safe to dereference.");
        } else {
            err.span_label(self.cx.arg, "has a drop method which cannot be safely finalized.");
            err.span_label(span, "caused by the expression in `fn drop(&mut)` here because");
            err.span_label(span, "it uses a type which is not safe to use in a finalizer.");
            err.help("`Gc` runs finalizers on a separate thread, so drop methods\nmust only use values whose types implement `Send + Sync` or `FinalizerSafe`.");
        }
        err.emit();
    }
}

impl<'a, 'tcx> Visitor<'tcx> for ProjectionChecker<'a, 'tcx> {
    fn visit_projection(
        &mut self,
        place_ref: PlaceRef<'tcx>,
        context: PlaceContext,
        location: Location,
    ) {
        for (_, proj) in place_ref.iter_projections() {
            match proj {
                ProjectionElem::Field(_, ty) => {
                    if !self.cx.is_finalizer_safe(ty) {
                        let span = self.body.source_info(location).span;
                        self.emit_err(ty, span);
                    }
                }
                _ => (),
            }
        }
        self.super_projection(place_ref, context, location);
    }

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
        if let TerminatorKind::Call { ref args, .. } = terminator.kind {
            for caller_arg in self.body.args_iter() {
                let recv_ty = self.body.local_decls()[caller_arg].ty;
                for arg in args.iter() {
                    let arg_ty = arg.ty(self.body, self.cx.tcx);
                    if arg_ty == recv_ty {
                        // Currently, we do not recurse into function calls
                        // to see whether they access `!FinalizerSafe`
                        // fields, so we must throw an error in `drop`
                        // methods which call other functions and pass
                        // `self` as an argument.
                        //
                        // Here, we throw an error if `drop(&mut self)`
                        // calls a function with an argument that has the
                        // same type as the drop receiver (e.g. foo(x:
                        // &Self)). This approximation will always prevent
                        // unsound `drop` methods, however, it is overly
                        // conservative and will prevent correct examples
                        // like below from compiling:
                        //
                        // ```
                        // fn drop(&mut self) {
                        //   let x = Self { ... };
                        //   x.foo();
                        // }
                        // ```
                        //
                        // This example is sound, because `x` is a local
                        // that was instantiated on the finalizer thread, so
                        // its fields are always safe to access from inside
                        // this drop method.
                        //
                        // However, this will not compile, because the
                        // receiver for `x.foo()` is the same type as the
                        // `self` reference. To fix this, we would need to
                        // do a def-use analysis on the self reference to
                        // find every MIR local which refers to it that ends
                        // up being passed to a call terminator. This is not
                        // trivial to do at the moment.
                        let span = self.body.source_info(location).span;
                        self.emit_err(arg_ty, span);
                    }
                }
            }
        }
    }
}
