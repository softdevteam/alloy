//! Queries for checking whether a type implements one of a few common traits.

use rustc_hir::lang_items::LangItem;
use rustc_infer::infer::TyCtxtInferExt;
use rustc_middle::query::Providers;
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::{Symbol, sym};
use rustc_trait_selection::traits;

fn is_copy_raw<'tcx>(tcx: TyCtxt<'tcx>, query: ty::PseudoCanonicalInput<'tcx, Ty<'tcx>>) -> bool {
    is_item_raw(tcx, query, LangItem::Copy)
}

fn is_sized_raw<'tcx>(tcx: TyCtxt<'tcx>, query: ty::PseudoCanonicalInput<'tcx, Ty<'tcx>>) -> bool {
    is_item_raw(tcx, query, LangItem::Sized)
}

fn is_freeze_raw<'tcx>(tcx: TyCtxt<'tcx>, query: ty::PseudoCanonicalInput<'tcx, Ty<'tcx>>) -> bool {
    is_item_raw(tcx, query, LangItem::Freeze)
}

fn is_unpin_raw<'tcx>(tcx: TyCtxt<'tcx>, query: ty::PseudoCanonicalInput<'tcx, Ty<'tcx>>) -> bool {
    is_item_raw(tcx, query, LangItem::Unpin)
}

fn drop_method_finalizer_elidable_raw<'tcx>(
    tcx: TyCtxt<'tcx>,
    query: ty::PseudoCanonicalInput<'tcx, Ty<'tcx>>,
) -> bool {
    is_item_raw(tcx, query, LangItem::DropMethodFinalizerElidable)
}

fn is_finalizer_safe_raw<'tcx>(
    tcx: TyCtxt<'tcx>,
    query: ty::PseudoCanonicalInput<'tcx, Ty<'tcx>>,
) -> bool {
    is_diagnostic_item_raw(tcx, query, sym::FinalizerSafe)
}

fn is_send_raw<'tcx>(tcx: TyCtxt<'tcx>, query: ty::PseudoCanonicalInput<'tcx, Ty<'tcx>>) -> bool {
    is_diagnostic_item_raw(tcx, query, sym::Send)
}

fn is_sync_raw<'tcx>(tcx: TyCtxt<'tcx>, query: ty::PseudoCanonicalInput<'tcx, Ty<'tcx>>) -> bool {
    is_diagnostic_item_raw(tcx, query, sym::Sync)
}

fn is_diagnostic_item_raw<'tcx>(
    tcx: TyCtxt<'tcx>,
    query: ty::PseudoCanonicalInput<'tcx, Ty<'tcx>>,
    item: Symbol,
) -> bool {
    let trait_def_id = tcx.get_diagnostic_item(item).unwrap();
    let (infcx, param_env) = tcx.infer_ctxt().build_with_typing_env(query.typing_env);
    traits::type_known_to_meet_bound_modulo_regions(&infcx, param_env, query.value, trait_def_id)
}

fn is_item_raw<'tcx>(
    tcx: TyCtxt<'tcx>,
    query: ty::PseudoCanonicalInput<'tcx, Ty<'tcx>>,
    item: LangItem,
) -> bool {
    let (infcx, param_env) = tcx.infer_ctxt().build_with_typing_env(query.typing_env);
    let trait_def_id = tcx.require_lang_item(item, None);
    traits::type_known_to_meet_bound_modulo_regions(&infcx, param_env, query.value, trait_def_id)
}

pub(crate) fn provide(providers: &mut Providers) {
    *providers = Providers {
        is_copy_raw,
        is_sized_raw,
        is_send_raw,
        is_sync_raw,
        is_finalizer_safe_raw,
        is_freeze_raw,
        is_unpin_raw,
        drop_method_finalizer_elidable_raw,
        ..*providers
    };
}
