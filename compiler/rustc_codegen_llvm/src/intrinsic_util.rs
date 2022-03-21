use rustc_codegen_ssa::mir::operand::OperandRef;
use rustc_codegen_ssa::mir::operand::OperandValue;
use rustc_codegen_ssa::mir::place::PlaceRef;
use rustc_codegen_ssa::traits::{BaseTypeMethods, BuilderMethods, MiscMethods};
use rustc_hir::LangItem;
use rustc_middle::ty;
use rustc_middle::ty::layout::LayoutOf;
use rustc_middle::ty::subst::GenericArg;
use rustc_middle::ty::subst::{Subst, SubstsRef};
use rustc_middle::ty::util::IntTypeExt;
use rustc_middle::ty::AdtDef;
use rustc_span::DUMMY_SP;

use crate::builder::Builder;
use crate::value::Value;

/// Recurse through the type of an rvalue and generate calls to
/// `Collectable::set_collectable` for every component type which implements
/// `Collectable`.
///
/// This is analogous to the MIR drop glue elaboration, except it must happen
/// during codegen because it needs access to monomorphized types for the
/// `ty.is_collectable` trait resolution.
pub(crate) fn codegen_collectable_calls<'ll, 'tcx>(
    builder: &mut Builder<'_, 'll, 'tcx>,
    op: OperandRef<'tcx, &'ll Value>,
    substs: SubstsRef<'tcx>,
) {
    let place = op.deref(builder.cx());
    codegen_collectable_calls_for_value(builder, place, substs);
}

fn codegen_collectable_calls_for_value<'ll, 'tcx>(
    builder: &mut Builder<'_, 'll, 'tcx>,
    place: PlaceRef<'tcx, &'ll Value>,
    substs: SubstsRef<'tcx>,
) {
    let ty = place.layout.ty.subst(builder.tcx, substs);
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
            // Rustc can't automatically generate calls to `Collectable` trait
            // methods behind a raw pointer because the pointer might be
            // invalid. If such a case is necessary (i.e. a smart ptr
            // abstraction over a raw ptr), then it must be called manually in
            // the `Collectable` impl of the parent type.
            | ty::RawPtr(..)
            | ty::Str => return,

            ty::Ref(..) => {
                let place = builder.load_operand(place).deref(builder.cx());
                codegen_collectable_calls_for_value(builder, place, substs)
            },
            ty::Adt(adt_def, substs) => {
                let set_col_did = match builder.tcx.lang_items().require(LangItem::SetCollectable) {
                    Ok(id) => id,
                    Err(err) => builder.tcx.sess.fatal(&err),
                };

                // First, add a call to the current ADT's collectable trait if
                // it exists.
                if ty.is_collectable(builder.tcx.at(DUMMY_SP), ty::ParamEnv::reveal_all()) {
                    let mono_ty = builder.tcx.mk_substs(std::iter::once::<GenericArg<'tcx>>(ty.into()));
                    let inst = ty::Instance::resolve(builder.tcx, ty::ParamEnv::reveal_all(), set_col_did, mono_ty).unwrap().unwrap();
                    let f = builder.cx().get_fn_addr(inst);
                    let fn_ty = builder.type_func(&[builder.val_ty(place.llval)], builder.type_void());
                    builder.call(fn_ty, f, &[place.llval], None);
                }

                if adt_def.is_union() {
                    // There's no way of knowing which union field is active, so
                    // type recursion must terminate here. If the user cares
                    // about this, they should define what happens in an `impl
                    // Collectable` block on the union itself.
                    return;
                }

                if adt_def.is_enum() {
                    // This terminates the current basic block with a switch
                    // statement, so the builder must be updated to use the
                    // successor.
                    *builder = codegen_collectable_calls_for_enum(builder, place, substs, *adt_def);
                    return;
                }

                // Iterate over the adt fields to ensure that if their values
                // contain Collectable implementations, they are also called.
                for i in 0..place.layout.fields.count() {
                    let field = place.project_field(builder, i);
                    codegen_collectable_calls_for_value(builder, field, substs);
                }
            },
            ty::Tuple(..) | ty::Array(..) | ty::Slice(..) => {
                for i in 0..place.layout.fields.count() {
                    let field = place.project_field(builder, i);
                    codegen_collectable_calls_for_value(builder, field, substs);
                }
        }
        _ => todo!(),
    }
}

// If an enum variant contains type(s) which implement `Collectable`, the
// `set_collectable` method must only be called if that variant is instantiated.
// In general, this can only be known at runtime, so codegenning collectable
// calls for an enum requires inserting a switch statement which branches on the
// discriminant.
//
// The basic algorithm works as follows:
//
//    1. A successor basic block is created, where control flow converges after
//       the switch statement. This successor is the block associated with the
//       `builder` for all future codegen'd statements in the same
//       `make_collectable` intrinsic.
//
//    2. A basic block is created for each variant, these dominate the successor
//       block.
//
//    3. The fields of each variant are iterated over and have their types
//       recursed into in order to look for more `Collectable` impls. Once
//       found, calls to `set_collectable` methods are codegen'd to the
//       respective variant block.
//
//    4. The builder is "patched up" to use the successor block.
//
// This is recursive, and can contain nested switch statements of arbitrary
// depth where enum variants contain other enums. There's a potential here for
// an explosion in the complexity of the CFG because of all the layers of nested
// switches and blocks, but I think this goes away when the LLVM IR is lowered
// into the target backend.
fn codegen_collectable_calls_for_enum<'a, 'll, 'tcx>(
    builder: &mut Builder<'a, 'll, 'tcx>,
    place: PlaceRef<'tcx, &'ll Value>,
    substs: SubstsRef<'tcx>,
    adt_def: &'tcx AdtDef,
) -> Builder<'a, 'll, 'tcx> {
    // Codegen loading the enum discriminant.
    let discr_ty = adt_def.repr.discr_type().to_ty(builder.tcx);
    let discr = place.codegen_get_discr(builder, discr_ty);
    let discr_rv = OperandRef {
        val: OperandValue::Immediate(discr),
        layout: builder.cx().layout_of(discr_ty),
    };

    // Build the successor block.
    let name = format!("successor_{:?}", adt_def.did);
    let successor = builder.build_sibling_block(&name);

    // Codegen a basic block for each enum variant. Variants with multiple
    // fields use the same variant block.
    let mut variants = Vec::with_capacity(adt_def.variants.len());
    for (idx, discr) in adt_def.discriminants(builder.tcx) {
        let name = format!("adt_{:?}_variant_{}", adt_def.did, discr.val);
        let vbx = builder.build_sibling_block(&name);
        variants.push((idx, discr.val, vbx));
    }

    // Terminate the current block in the builder context with a switch on the
    // variants.
    builder.switch(
        discr_rv.immediate(),
        successor.llbb(),
        variants.iter().map(|v| (v.1, v.2.llbb())),
    );

    // Build set_collectable calls for each variant
    for (idx, _discr, vbx) in variants.iter_mut() {
        let place = place.project_downcast(vbx, *idx);
        let variant = &adt_def.variants[*idx];
        for (idx, _) in variant.fields.iter().enumerate() {
            let field = place.project_field(vbx, idx);
            codegen_collectable_calls_for_value(vbx, field, substs);
        }
        vbx.br(successor.llbb());
    }
    successor
}
