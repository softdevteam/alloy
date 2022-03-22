use rustc_hir::LangItem;
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::subst::GenericArg;
use rustc_middle::ty::subst::SubstsRef;
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::source_map::Spanned;
use rustc_span::DUMMY_SP;

use rustc_hir::def_id::DefId;
/// Recurses through the component types of `ty` looking for types which
/// implement the `Collectable` trait so that each
/// `Collectable::set_collectable` method can be added to the monomorphization
/// collector's output.
pub(crate) fn collect_mono<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    substs: SubstsRef<'tcx>,
    output: &'a mut Vec<Spanned<MonoItem<'tcx>>>,
) {
    let set_col_did =
        tcx.lang_items().require(LangItem::SetCollectable).unwrap_or_else(|e| tcx.sess.fatal(&e));
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
        | ty::Str => return,

        ty::RawPtr(raw) => collect_mono(tcx, raw.ty, substs, output),
        ty::Ref(_, refty, ..) => collect_mono(tcx, refty, substs, output),
        ty::Adt(adt_def, ..) => {
            if ty.is_collectable(tcx.at(DUMMY_SP), ty::ParamEnv::reveal_all()) {
                let subs = tcx.mk_substs(std::iter::once::<GenericArg<'tcx>>(ty.into()));
                let f = ty::Instance::resolve(tcx, ty::ParamEnv::reveal_all(), set_col_did, subs)
                    .unwrap()
                    .unwrap();
                output.push(crate::collector::create_fn_mono_item(tcx, f, DUMMY_SP));
            }

            if adt_def.is_enum() {
                for v in adt_def.variants.iter() {
                    for f in v.fields.iter() {
                        collect_mono(tcx, f.ty(tcx, substs), substs, output);
                    }
                }
            }

            for field in adt_def.all_fields() {
                let field_ty = tcx.type_of(field.did);
                collect_mono(tcx, field_ty, substs, output);
            }
        }
        ty::Tuple(substs) => {
            for f in ty.tuple_fields() {
                collect_mono(tcx, f, substs, output);
            }
        }
        ty::Array(aty, ..) => collect_mono(tcx, aty, substs, output),
        ty::Slice(sty) => collect_mono(tcx, sty, substs, output),
        _ => return,
    }
}

pub fn is_collectable_trait_method<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> bool {
    let col_did = tcx.lang_items().require(LangItem::Collectable);
    if col_did.is_ok() {
        match tcx.impl_of_method(def_id) {
            Some(d) => tcx.trait_id_of_impl(d).map_or(false, |d| d == col_did.unwrap()),
            None => false,
        }
    } else {
        false
    }
}
