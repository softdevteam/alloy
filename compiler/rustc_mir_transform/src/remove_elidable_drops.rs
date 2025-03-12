#![cfg_attr(feature = "rustc_no_premopt", allow(dead_code))]
use rustc_middle::mir::*;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::sym;
use tracing::trace;

use super::simplify::simplify_cfg;
use crate::MirPass;

pub(super) struct RemoveElidableDrops;

impl<'tcx> MirPass<'tcx> for RemoveElidableDrops {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        trace!("Running RemoveElidableDrops on {:?}", body.source);

        let is_gc_crate = tcx
            .get_diagnostic_item(sym::gc)
            .map_or(false, |gc| gc.krate == body.source.def_id().krate);

        let typing_env = body.typing_env(tcx);
        let mut should_simplify = false;

        for block in body.basic_blocks.as_mut() {
            let terminator = block.terminator_mut();
            if let TerminatorKind::Drop { place, target, .. } = terminator.kind {
                let ty = place.ty(&body.local_decls, tcx).ty;
                if !ty.is_gc(tcx) {
                    continue;
                }

                if let ty::Adt(_, substs) = ty.kind() {
                    if is_gc_crate || !substs.type_at(0).needs_finalizer(tcx, typing_env) {
                        terminator.kind = TerminatorKind::Goto { target };
                        should_simplify = true;
                    }
                }
            }
        }
        // if we applied optimizations, we potentially have some cfg to cleanup to
        // make it easier for further passes
        if should_simplify {
            simplify_cfg(body);
        }
    }
    fn is_required(&self) -> bool {
        true
    }
}
