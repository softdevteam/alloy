//! This pass replaces a drop of a type that does not need dropping, with a goto

use crate::transform::MirPass;
use rustc_middle::mir::*;
use rustc_middle::ty::TyCtxt;

// use super::simplify::simplify_cfg;

pub struct RemoveNoFinalizeDrops;

impl<'tcx> MirPass<'tcx> for RemoveNoFinalizeDrops {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        trace!("Running RemoveFinalizerGlue on {:?}", body.source);
        debug!("Running RemoveFinalizerGlue on {:?}", body.source);

        let did = body.source.def_id();
        // let param_env = tcx.param_env(did);
        // let mut should_simplify = false;
        let flzr_glue = tcx.lang_items().finalizer_glue_fn();

        if flzr_glue.is_none() || flzr_glue.unwrap() != did {
            return;
        }

        let (basic_blocks, local_decls) = body.basic_blocks_and_local_decls_mut();
        for block in basic_blocks {
            let terminator = block.terminator_mut();
            if let TerminatorKind::Drop { place, target, .. } = terminator.kind {
                let ty = place.ty(local_decls, tcx);
                // if ty.ty.needs_drop(tcx, param_env) {
                //     continue;
                // }
                // if !tcx.consider_optimizing(|| format!("RemoveUnneededDrops {:?} ", did)) {
                //     continue;
                // }
                debug!("FOUND: `drop` with place({:?}), place_ty: {:?}, and target: {:?}", place, ty, target);
            }
        }

        // if we applied optimizations, we potentially have some cfg to cleanup to
        // make it easier for further passes
        // if should_simplify {
        //     simplify_cfg(tcx, body);
        // }
    }
}
