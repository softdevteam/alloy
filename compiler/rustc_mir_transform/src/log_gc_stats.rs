#![cfg_attr(not(feature = "rustc_log_gc_stats"), allow(dead_code))]
use std::env;
use std::fs::OpenOptions;
use std::io::Write;

use rustc_hir::def_id::DefId;
use rustc_middle::mir::*;
use rustc_middle::ty::TyCtxt;
use rustc_span::sym;
use tracing::trace;

use crate::MirPass;
use crate::errors::LogStatsError;

#[derive(Default)]
struct GcStats {
    num_gcs: u64,
    num_rcs: u64,
    num_weaks: u64,
    num_arcs: u64,
    num_arcweaks: u64,
    num_elidable_finalizers: u64,
}

pub(super) struct LogGcStats;

impl<'tcx> MirPass<'tcx> for LogGcStats {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        if env::var("ALLOY_RUSTC_LOG").is_err() {
            return;
        }
        trace!("Calculating GcStats on {:?}", body.source);

        if in_std_lib(tcx, body.source.def_id()) {
            // A hacky way of checking if we're in the standard library.
            // If we are, we don't want to record GC statistics.
            return;
        }

        let gc = tcx.get_diagnostic_item(sym::gc).unwrap();
        let rc = tcx.get_diagnostic_item(sym::Rc).unwrap();
        let arc = tcx.get_diagnostic_item(sym::Arc).unwrap();
        let weak = tcx.get_diagnostic_item(sym::RcWeak).unwrap();
        let arcweak = tcx.get_diagnostic_item(sym::ArcWeak).unwrap();

        let typing_env = body.typing_env(tcx);
        let mut stats = GcStats::default();

        for decl in body.local_decls().iter().skip(1) {
            if decl.ty.ty_adt_def().is_none() || !decl.is_user_variable() {
                // Smart pointers types are always ADTs.
                // We also only care about those that were explicitly defined by the user.
                continue;
            }

            let did = decl.ty.ty_adt_def().unwrap().did();
            match did {
                _ if did == gc => {
                    stats.num_elidable_finalizers +=
                        decl.ty.gced_ty(tcx).needs_finalizer(tcx, typing_env) as u64;
                    stats.num_gcs += 1
                }
                _ if did == rc => stats.num_rcs += 1,
                _ if did == arc => stats.num_arcs += 1,
                _ if did == weak => stats.num_weaks += 1,
                _ if did == arcweak => stats.num_arcweaks += 1,
                _ => (),
            }
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(env::var("ALLOY_RUSTC_LOG").unwrap())
            .unwrap_or_else(|e| {
                tcx.sess.psess.dcx().emit_fatal(LogStatsError { reason: e.to_string() })
            });

        if file
            .metadata()
            .unwrap_or_else(|e| {
                tcx.sess.psess.dcx().emit_fatal(LogStatsError { reason: e.to_string() })
            })
            .len()
            == 0
        {
            let _ = writeln!(
                file,
                "fn,num_gcs,num_rcs,num_arcs,num_weaks,num_arcweaks,num_elidable_finalizers"
            )
            .inspect_err(|e| {
                tcx.sess.psess.dcx().emit_fatal(LogStatsError { reason: e.to_string() })
            });
        }

        let _ = writeln!(
            file,
            "{},{},{},{},{},{},{}",
            tcx.def_path_str(body.source.def_id()),
            stats.num_gcs,
            stats.num_rcs,
            stats.num_arcs,
            stats.num_weaks,
            stats.num_arcweaks,
            stats.num_elidable_finalizers,
        )
        .inspect_err(|e| tcx.sess.psess.dcx().emit_fatal(LogStatsError { reason: e.to_string() }));
    }

    fn is_required(&self) -> bool {
        true
    }
}

fn in_std_lib<'tcx>(tcx: TyCtxt<'tcx>, did: DefId) -> bool {
    let alloc_crate = tcx.get_diagnostic_item(sym::Rc).map_or(false, |x| did.krate == x.krate);
    let core_crate = tcx.get_diagnostic_item(sym::RefCell).map_or(false, |x| did.krate == x.krate);
    let std_crate = tcx.get_diagnostic_item(sym::Mutex).map_or(false, |x| did.krate == x.krate);
    alloc_crate || std_crate || core_crate
}
