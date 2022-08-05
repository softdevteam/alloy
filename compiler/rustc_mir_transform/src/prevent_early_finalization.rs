use crate::MirPass;
use rustc_ast::{LlvmAsmDialect, StrStyle};
use rustc_hir as hir;
use rustc_middle::mir::patch::MirPatch;
use rustc_middle::mir::*;
use rustc_middle::ty::{self, ParamEnv, TyCtxt};
use rustc_span::symbol::{kw, sym, Symbol};
use rustc_span::DUMMY_SP;

#[derive(PartialEq)]
pub struct PreventEarlyFinalization;

impl<'tcx> MirPass<'tcx> for PreventEarlyFinalization {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        if tcx.lang_items().gc_type().is_none() {
            return;
        }
        let barrier = build_asm_barrier();
        let param_env = tcx.param_env_reveal_all_normalized(body.source.def_id());
        let gc_locals = body
            .local_decls()
            .iter_enumerated()
            .filter(|(_, d)| needs_barrier(d, tcx, param_env))
            .map(|(l, _)| l)
            .collect::<Vec<_>>();

        if gc_locals.is_empty() {
            return;
        }
        // First, remove the StorageDead for each local, so that a new one can
        // be inserted after the barrier. Usually, this is at the end of the
        // body anyway, so it's quicker to iterate backwards over the MIR.
        for data in body.basic_blocks_mut() {
            for stmt in data.statements.iter_mut().rev() {
                if let StatementKind::StorageDead(ref local) = stmt.kind {
                    if gc_locals.contains(local) {
                        stmt.make_nop();
                    }
                }
            }
        }

        let mut patch = MirPatch::new(body);
        // There can be many BBs which terminate with a return, so to be safe,
        // we add an asm barrier to each one.
        let return_blocks =
            body.basic_blocks().iter_enumerated().filter(|(_, b)| is_return(b.terminator()));
        for (block, data) in return_blocks {
            for local in gc_locals.iter() {
                let loc = Location { block, statement_index: data.statements.len() };
                let mut asm = barrier.clone();
                asm.inputs = Box::new([(DUMMY_SP, Operand::Copy(Place::from(*local)))]);
                // patch.add_statement(loc, StatementKind::LlvmInlineAsm(asm));
                patch.add_statement(loc, StatementKind::StorageDead(*local));
            }
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

fn needs_barrier<'tcx>(
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

fn build_asm_barrier<'tcx>() -> Box<LlvmInlineAsm<'tcx>> {
    let asm_inner = hir::LlvmInlineAsmInner {
        asm: kw::Empty,
        asm_str_style: StrStyle::Cooked,
        outputs: Vec::new(),
        inputs: vec![Symbol::intern("r")],
        clobbers: vec![sym::memory],
        volatile: true,
        alignstack: false,
        dialect: LlvmAsmDialect::Att,
    };

    Box::new(LlvmInlineAsm { asm: asm_inner, inputs: Box::new([]), outputs: Box::new([]) })
}
