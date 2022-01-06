use rustc_middle::mir::interpret::Allocation;
use rustc_middle::ty::{ParamEnv, Ty, TyCtxt};

/// Directly returns an `Allocation` containing bitmap of a type's GC layout.
crate fn alloc_gc_layout<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    param_env: ParamEnv<'tcx>,
) -> &'tcx Allocation {
    if ty.ty_adt_def().is_none() {
        unimplemented!("core::intrinsics::gc_layout::<T>() is currently only supported for ADTs");
    }

    let (bitmap, bitmap_size) = ty.gc_layout(tcx, param_env);
    let mut pair = Vec::with_capacity(16);
    pair.extend_from_slice(&bitmap.to_le_bytes());
    pair.extend_from_slice(&bitmap_size.to_le_bytes());
    let alloc = Allocation::from_bytes_byte_aligned_immutable(pair);
    tcx.intern_const_alloc(alloc)
}
