#![allow(dead_code)]
#![allow(unused_must_use)]
use rustc_hir as hir;
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_session::{declare_lint, declare_lint_pass};

use crate::lints::MisalignedGcPointers as MisalignedLint;
use crate::{LateContext, LateLintPass, LintContext};

declare_lint! {
    /// The `misaligned_gc_pointers` lint checks that packed structs have no
    /// traceable fields.
    ///
    /// ### Example
    ///
    /// ```rust,compile_fail
    /// #[repr(packed)]
    /// struct S(u8, *mut u8);
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// Packed structs have a min alignment of 1 byte, so fields which need to
    /// be traced by the GC --  such as `*mut u8` -- are not guaranteed to be
    /// word-aligned. This can cause fields to be missed by the collector and is
    /// undefined behaviour.
    pub MISALIGNED_GC_POINTERS,
    Deny,
    "Packed structs should not contain traceable values",
}

declare_lint_pass!(MisalignedGcPointers => [MISALIGNED_GC_POINTERS]);

const MIN_ALIGN: usize = std::mem::align_of::<usize>();

impl<'tcx> LateLintPass<'tcx> for MisalignedGcPointers {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        match item.kind {
            hir::ItemKind::Struct(..) => {
                let mut offset = 0;
                let adt_def = cx.tcx.adt_def(item.owner_id.to_def_id());
                if !adt_def.repr().packed() {
                    return;
                }

                let ty = cx.tcx.type_of(adt_def.did()).skip_binder();
                match has_correct_alignment(&mut offset, cx.tcx, ty) {
                    Ok(()) => (),
                    Err(err) => {
                        cx.emit_span_lint(
                            MISALIGNED_GC_POINTERS,
                            item.span,
                            MisalignedLint { ty: err.ty() },
                        );
                    }
                }
            }
            _ => (),
        }
    }
}

#[derive(Debug)]
enum AlignError<'tcx> {
    MisalignedPointer(Ty<'tcx>),
    MisalignedUInt(Ty<'tcx>),
    MisalignedInt(Ty<'tcx>),
    TySizeUnavailable(Ty<'tcx>),
}

impl<'tcx> AlignError<'tcx> {
    fn ty(&self) -> Ty<'tcx> {
        match self {
            AlignError::MisalignedPointer(ty)
            | AlignError::MisalignedUInt(ty)
            | AlignError::MisalignedInt(ty)
            | AlignError::TySizeUnavailable(ty) => *ty,
        }
    }
}

fn has_correct_alignment<'tcx>(
    offset: &mut usize,
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Result<(), AlignError<'tcx>> {
    match ty.kind() {
        ty::Ref(..) | ty::RawPtr(..) if *offset % MIN_ALIGN != 0 => {
            return Err(AlignError::MisalignedPointer(ty));
        }
        ty::Uint(i)
            if ((i.bit_width().is_none()
                || i.bit_width().unwrap() == u64::try_from(size_of::<usize>()).unwrap() * 8)
                && *offset % MIN_ALIGN != 0) =>
        {
            return Err(AlignError::MisalignedUInt(ty));
        }
        ty::Int(i)
            if ((i.bit_width().is_none()
                || i.bit_width().unwrap() == u64::try_from(size_of::<usize>()).unwrap() * 8)
                && *offset % MIN_ALIGN != 0) =>
        {
            return Err(AlignError::MisalignedInt(ty));
        }
        ty::Tuple(tys) => tys.iter().try_for_each(|t| has_correct_alignment(offset, tcx, t))?,
        ty::Array(t, length) => {
            has_correct_alignment(offset, tcx, *t)?;
            *offset = type_size(tcx, *t)? * (length.try_to_target_usize(tcx).unwrap() as usize - 1);
        }
        ty::Adt(adt_def, _) => {
            for field_def in adt_def.all_fields() {
                let field_ty = tcx.type_of(field_def.did).skip_binder();
                has_correct_alignment(offset, tcx, field_ty)?;
            }
        }
        _ => *offset += type_size(tcx, ty)?,
    }
    return Ok(());
}

fn type_size<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Result<usize, AlignError<'tcx>> {
    match tcx.layout_of(ty::TypingEnv::fully_monomorphized().as_query_input(ty)) {
        Ok(layout) => {
            let size = layout.size;
            Ok(size.bytes_usize())
        }
        Err(_) => Err(AlignError::TySizeUnavailable(ty)),
    }
}
