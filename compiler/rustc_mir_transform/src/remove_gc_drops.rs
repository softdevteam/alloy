use crate::MirPass;
use rustc_middle::mir::*;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::sym;

use super::simplify::simplify_cfg;

pub struct RemoveGcDrops;

impl<'tcx> MirPass<'tcx> for RemoveGcDrops {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        trace!("Running RemoveGcDrops on {:?}", body.source);

        let is_gc_crate = tcx
            .get_diagnostic_item(sym::gc)
            .map_or(false, |gc| gc.krate == body.source.def_id().krate);

        let did = body.source.def_id();
        let param_env = tcx.param_env_reveal_all_normalized(did);
        let mut should_simplify = false;

        for block in body.basic_blocks.as_mut() {
            let terminator = block.terminator_mut();
            if let TerminatorKind::Drop { place, target, .. } = terminator.kind {
                let ty = place.ty(&body.local_decls, tcx).ty;
                if !ty.is_gc(tcx) {
                    continue;
                }

                if let ty::Adt(_, substs) = ty.kind() {
                    if is_gc_crate || !substs.type_at(0).needs_finalizer(tcx, param_env) {
                        terminator.kind = TerminatorKind::Goto { target };
                        should_simplify = true;
                    }
                }
            }
        }
        // if we applied optimizations, we potentially have some cfg to cleanup to
        // make it easier for further passes
        if should_simplify {
            simplify_cfg(tcx, body);
        }
    }
}
