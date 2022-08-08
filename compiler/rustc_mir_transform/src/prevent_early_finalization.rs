use crate::MirPass;
use rustc_middle::mir::patch::MirPatch;
use rustc_middle::mir::*;
use rustc_middle::ty::{self, ParamEnv, TyCtxt};
use rustc_span::symbol::sym;
use rustc_span::DUMMY_SP;

#[derive(PartialEq)]
pub struct PreventEarlyFinalization;

impl<'tcx> MirPass<'tcx> for PreventEarlyFinalization {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        if tcx.lang_items().gc_type().is_none() {
            return;
        }

        let param_env = tcx.param_env_reveal_all_normalized(body.source.def_id());
        let gc_locals = body
            .local_decls()
            .iter_enumerated()
            .filter(|(_, d)| needs_black_box(d, tcx, param_env))
            .map(|(l, _)| l)
            .collect::<Vec<_>>();

        if gc_locals.is_empty() {
            return;
        }

        let mut patch = MirPatch::new(body);
        let operand = Operand::function_handle(
            tcx,
            tcx.get_diagnostic_item(sym::black_box).unwrap(),
            ty::List::empty(),
            DUMMY_SP,
        );

        // There can be many BBs which terminate with a return, so to be safe,
        // we add a black box to each one.
        let return_blocks =
            body.basic_blocks().iter_enumerated().filter(|(_, b)| is_return(b.terminator()));
        for (ret_bb, ret_bb_data) in return_blocks {
            let return_terminator = ret_bb_data.terminator().clone();
            let mut successor = patch.new_block(BasicBlockData {
                statements: vec![],
                terminator: Some(return_terminator),
                is_cleanup: false,
            });

            for keep_alive in gc_locals.iter() {
                let unit_temp = Place::from(patch.new_temp(tcx.mk_unit(), DUMMY_SP));
                let black_box_call = TerminatorKind::Call {
                    func: operand.clone(),
                    args: vec![Operand::Move(Place::from(*keep_alive))],
                    destination: unit_temp,
                    target: Some(successor),
                    cleanup: None,
                    from_hir_call: false,
                    fn_span: DUMMY_SP,
                };

                successor = patch.new_block(BasicBlockData {
                    statements: vec![],
                    terminator: Some(Terminator {
                        source_info: SourceInfo::outermost(DUMMY_SP),
                        kind: black_box_call,
                    }),
                    is_cleanup: false,
                });
            }
            patch.patch_terminator(ret_bb, TerminatorKind::Goto { target: successor });
        }
        patch.apply(body);
    }
}

fn is_return<'tcx>(terminator: &Terminator<'tcx>) -> bool {
    match terminator.kind {
        TerminatorKind::Return => true,
        _ => false,
    }
}

fn needs_black_box<'tcx>(
    local: &LocalDecl<'tcx>,
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
) -> bool {
    if !local.is_user_variable() {
        return false;
    }

    if let ty::Adt(def, ..) = local.ty.kind() {
        if def.did() != tcx.lang_items().gc_type().unwrap() {
            return false;
        }

        if local.ty.is_no_finalize_modulo_regions(tcx.at(DUMMY_SP), param_env) {
            return false;
        }

        return true;
    }

    return false;
}
