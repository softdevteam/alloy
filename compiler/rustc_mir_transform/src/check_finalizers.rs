use crate::MirPass;
use rustc_middle::mir::*;
use rustc_middle::ty::subst::InternalSubsts;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::symbol::sym;
use rustc_span::Span;
use rustc_trait_selection::infer::InferCtxtExt;
use rustc_trait_selection::infer::TyCtxtInferExt;
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
                    if let ty::FnDef(fn_did, _) = func_ty.kind() {
                        if *fn_did == ctor_did {
                            let arg_ty = args[0].ty(body, tcx);
                            if !arg_ty.needs_finalizer(tcx, param_env) {
                                return;
                            }

                            let (is_send, is_sync) = tcx.infer_ctxt().enter(|infcx| {
                                let send = tcx.get_diagnostic_item(sym::Send).map(|t| {
                                    infcx
                                        .type_implements_trait(
                                            t,
                                            arg_ty,
                                            InternalSubsts::empty(),
                                            param_env,
                                        )
                                        .may_apply()
                                }) == Some(true);

                                let sync = tcx.get_diagnostic_item(sym::Sync).map(|t| {
                                    infcx
                                        .type_implements_trait(
                                            t,
                                            arg_ty,
                                            InternalSubsts::empty(),
                                            param_env,
                                        )
                                        .may_apply()
                                }) == Some(true);
                                (send, sync)
                            });
                            if !is_send {
                                emit_err(tcx, body, source_info.span, &args[0], "Send");
                            }
                            if !is_sync {
                                emit_err(tcx, body, source_info.span, &args[0], "Sync");
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn emit_err<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, fun: Span, arg: &Operand<'tcx>, t: &str) {
    let arg_sp = match arg {
        Operand::Copy(place) | Operand::Move(place) => {
            body.local_decls()[place.local].source_info.span
        }
        Operand::Constant(con) => con.span,
    };
    let snippet = tcx.sess.source_map().span_to_snippet(arg_sp).unwrap();
    let mut err =
        tcx.sess.struct_span_err(arg_sp, format!("`{}` cannot be safely finalized.", snippet));
    err.span_label(arg_sp, "has a drop method which cannot be safely finalized.");
    err.span_label(fun, format!("`Gc::new` requires that it implements the `{}` trait.", t));
    err.help(format!("`Gc` runs finalizers on a separate thread, so `{}` must implement `{}` in order to be safely dropped", snippet, t));
    err.emit();
}
