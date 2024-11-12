#![allow(rustc::untranslatable_diagnostic)]
#![allow(rustc::diagnostic_outside_of_impl)]
use rustc_data_structures::fx::FxHashSet;
use rustc_hir::def_id::DefId;
use rustc_hir::lang_items::LangItem;
use rustc_middle::mir::visit::PlaceContext;
use rustc_middle::mir::visit::Visitor;
use rustc_middle::mir::*;
use rustc_middle::ty::{self, ParamEnv, Ty, TyCtxt};
use rustc_span::symbol::sym;
use rustc_span::Span;
use std::collections::VecDeque;

#[derive(PartialEq)]
pub struct CheckFinalizers;

#[derive(Debug)]
enum FinalizerErrorKind<'tcx> {
    /// Does not implement `Send` + `Sync`
    NotSendAndSync(FnInfo<'tcx>, ProjInfo<'tcx>),
    /// Does not implement `FinalizerSafe`
    NotFinalizerSafe(FnInfo<'tcx>, ProjInfo<'tcx>),
    /// Contains a field projection where one of the projection elements is a reference.
    UnsoundReference(FnInfo<'tcx>, ProjInfo<'tcx>),
    /// Uses a trait object whose concrete type is unknown
    UnknownTraitObject(FnInfo<'tcx>),
    /// Calls a function whose definition is unavailable, so we can't be certain it's safe.
    MissingFnDef(FnInfo<'tcx>),
    /// The drop glue contains an unsound drop method from an external crate. This will have been
    /// caused by one of the above variants. However, it is confusing to propagate this to the user
    /// because they most likely won't be in a position to fix it from a downstream crate. Currently
    /// this only applies to types belonging to the standard library.
    UnsoundExternalDropGlue(FnInfo<'tcx>),
    /// Contains an inline assembly block, which can do anything, so we can't be certain it's safe.
    InlineAsm(FnInfo<'tcx>),
}

/// Information about the projection which caused the FSA error.
#[derive(Debug)]
struct ProjInfo<'tcx> {
    /// Span of the projection that caused an error.
    span: Span,
    /// Type of the projection that caused an error.
    ty: Ty<'tcx>,
}

impl<'tcx> ProjInfo<'tcx> {
    fn new(span: Span, ty: Ty<'tcx>) -> Self {
        Self { span, ty }
    }
}

/// Information about the function which caused the FSA error.
/// This could be the top level `drop` method, or a different function which was called (directly
/// or indirectly) from drop.
#[derive(Debug)]
struct FnInfo<'tcx> {
    /// Span of the function that caused an error.
    span: Span,
    /// Type of the value whose drop method the FSA error originated from.
    drop_ty: Ty<'tcx>,
}

impl<'tcx> FnInfo<'tcx> {
    fn new(span: Span, drop_ty: Ty<'tcx>) -> Self {
        Self { span, drop_ty }
    }
}

impl<'tcx> MirPass<'tcx> for CheckFinalizers {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        let param_env = tcx.param_env(body.source.def_id());

        if in_std_lib(tcx, body.source.def_id()) {
            // Do not check for FSA entry points if we're compiling the standard library. This is
            // because in practice, the only entry points would be `Gc` constructor calls in the
            // implementation of the `Gc` API (`library/std/gc.rs`), and we don't want to check
            // these.
            return;
        }

        for (func, args, source_info) in
            body.basic_blocks.iter().filter_map(|bb| match &bb.terminator().kind {
                TerminatorKind::Call { func, args, .. } => {
                    Some((func, args, bb.terminator().source_info))
                }
                _ => None,
            })
        {
            let fn_ty = func.ty(body, tcx);
            let ty::FnDef(fn_did, substs) = fn_ty.kind() else {
                // We don't care about function pointers, but we'll assert here incase there's
                // another kind of type we haven't accounted for.
                assert!(fn_ty.is_fn_ptr());
                continue;
            };

            let ret_ty = fn_ty.fn_sig(tcx).output().skip_binder();

            // The following is a gross hack for performance reasons!
            //
            // Calls in MIR which are trait method invocations point to the DefId
            // of the trait definition, and *not* the monomorphized concrete method definition.
            // This is a problem for us, because e.g. the `Gc::from` function definition will have the
            // `#[rustc_fsa_entry_point]` attribute, but the generic `T::from` definition will
            // not. This is a problem for us, because naively it means we must monomorphize
            // every single function call just to see if it points to a function somewhere inside
            // the `Gc` library with the desired attribute. This is painfully slow!
            //
            // To get around this, we can ignore all calls if they do not do both of the following:
            //
            //      a) point to some function in the standard library.
            //
            //      b) the generic substitution for the return type (which is readily available) is
            //      not a `Gc<T>`. In practice, this means we only actually end up having to
            //      resolve fn calls to their precise instance when they actually are some kind
            //      of `Gc` constructor (we still check for the attribute later on to make sure
            //      though!).
            if !in_std_lib(tcx, *fn_did)
                || !ret_ty.is_gc(tcx)
                || ty::Instance::expect_resolve(tcx, param_env, *fn_did, substs)
                    .def
                    .get_attrs(tcx, sym::rustc_fsa_entry_point)
                    .next()
                    .is_none()
            {
                continue;
            }
            FSAEntryPointCtxt::new(
                source_info.span,
                args[0].span,
                ret_ty.gced_ty(tcx),
                tcx,
                param_env,
            )
            .check_drop_glue();
        }
    }
}

/// The central data structure for performing FSA. Constructed and used each time a new FSA
/// entry-point is found in the MIR (e.g. a call to `Gc::new` or `Gc::from`).
struct FSAEntryPointCtxt<'tcx> {
    /// Span of the entry point.
    fn_span: Span,
    /// Span of the argument to the entry point.
    arg_span: Span,
    /// Type of the GC'd value created by the entry point.
    value_ty: Ty<'tcx>,
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
}

impl<'tcx> FSAEntryPointCtxt<'tcx> {
    fn new(
        fn_span: Span,
        arg_span: Span,
        value_ty: Ty<'tcx>,
        tcx: TyCtxt<'tcx>,
        param_env: ParamEnv<'tcx>,
    ) -> Self {
        Self { fn_span, arg_span, value_ty, tcx, param_env }
    }

    fn check_drop_glue(&self) {
        if !self.value_ty.needs_finalizer(self.tcx, self.param_env)
            || self.value_ty.is_finalize_unchecked(self.tcx)
        {
            return;
        }

        if self.value_ty.is_send(self.tcx, self.param_env)
            && self.value_ty.is_sync(self.tcx, self.param_env)
            && self.value_ty.is_finalizer_safe(self.tcx, self.param_env)
        {
            return;
        }

        let mut errors = Vec::new();
        let mut tys = vec![self.value_ty];

        loop {
            let Some(ty) = tys.pop() else {
                break;
            };

            // We must now identify every drop method in the drop glue for `ty`. This means looking
            // at each component type and adding those to the stack for later processing.
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
                | ty::RawPtr(..)
                | ty::Ref(..)
                | ty::Str
                | ty::Error(..)
                | ty::Foreign(..) => (),
                ty::Dynamic(..) => {
                    // Dropping a trait object uses a virtual call, so we can't
                    // work out which drop method to look at compile-time. This
                    // means we must be more conservative and bail with an error
                    // here, even if the drop impl itself would have been safe.
                    errors.push(FinalizerErrorKind::UnknownTraitObject(FnInfo::new(
                        rustc_span::DUMMY_SP,
                        ty,
                    )));
                }
                ty::Slice(ty) | ty::Array(ty, ..) => tys.push(*ty),
                ty::Tuple(fields) => {
                    for f in fields.iter() {
                        // Each tuple field must be individually checked for a `Drop` impl.
                        tys.push(f)
                    }
                }
                ty::Adt(def, substs) if !ty.is_copy_modulo_regions(self.tcx, self.param_env) => {
                    if def.is_box() {
                        // This is a special case because Box has an empty drop
                        // method which is filled in later by the compiler.
                        errors.push(FinalizerErrorKind::MissingFnDef(FnInfo::new(
                            rustc_span::DUMMY_SP,
                            ty,
                        )));
                    }
                    if def.has_dtor(self.tcx) {
                        let drop_trait_did = self.tcx.require_lang_item(LangItem::Drop, None);
                        let poly_drop_fn_did = self.tcx.associated_item_def_ids(drop_trait_did)[0];
                        let drop_instance = ty::Instance::expect_resolve(
                            self.tcx,
                            self.param_env,
                            poly_drop_fn_did,
                            self.tcx.mk_args_trait(ty, substs.into_iter()),
                        );
                        match DropCtxt::new(drop_instance, ty, self).check() {
                            Err(_) if in_std_lib(self.tcx, def.did()) => {
                                let fn_info = FnInfo::new(rustc_span::DUMMY_SP, ty);
                                errors.push(FinalizerErrorKind::UnsoundExternalDropGlue(fn_info));
                                // We skip checking the drop methods of this standard library
                                // type's fields -- we already know that it has an unsafe finaliser, so
                                // going over its fields serves no purpose other than to confuse users
                                // with extraneous FSA errors that they won't be able to fix anyway.
                                continue;
                            }
                            Err(ref mut e) => errors.append(e),
                            _ => (),
                        }
                    }

                    for field in def.all_fields() {
                        let field_ty = self.tcx.type_of(field.did).instantiate(self.tcx, substs);
                        tys.push(field_ty);
                    }
                }
                _ => (),
            }
        }
        errors.into_iter().for_each(|e| self.emit_error(e));
    }

    /// Attempts to load the monomorphized version of a MIR body for the given instance if it's
    /// available. If such MIR is not available then it will load the polymorphic MIR body.
    fn prefer_instantiated_mir(&self, instance: ty::Instance<'tcx>) -> Option<Body<'tcx>> {
        if !self.tcx.is_mir_available(instance.def_id()) {
            return None;
        }
        let mir = self.tcx.instance_mir(instance.def);
        instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                self.tcx,
                self.param_env,
                ty::EarlyBinder::bind(mir.clone()),
            )
            .ok()
    }

    /// For a given projection, extract the 'useful' type which needs checking for finalizer safety.
    ///
    /// Simplifying somewhat, a projection is a way of peeking into a place. For FSA, the
    /// projections that are interesting to us are struct/enum fields, and slice/array indices. When
    /// we find these, we want to extract the type of the field or slice/array element for further
    /// analysis. This is best explained with an example, the following shows the projection, and
    /// what type would be returned:
    ///
    /// a[i]    -> typeof(a[i])
    /// a.b[i]  -> typeof(a.b[i])
    /// a.b     -> typeof(b)
    /// a.b.c   -> typeof(c)
    ///
    /// In practice, this means that the type of the last projection is extracted and returned.
    fn extract_projection_ty(
        &self,
        body: &Body<'tcx>,
        base: PlaceRef<'tcx>,
        elem: ProjectionElem<Local, Ty<'tcx>>,
    ) -> Option<Ty<'tcx>> {
        match elem {
            ProjectionElem::Field(_, ty) => Some(ty),
            ProjectionElem::Index(_)
            | ProjectionElem::ConstantIndex { .. }
            | ProjectionElem::Subslice { .. } => {
                let array_ty = match base.last_projection() {
                    Some((last_base, last_elem)) => {
                        last_base.ty(body, self.tcx).projection_ty(self.tcx, last_elem).ty
                    }
                    None => base.ty(body, self.tcx).ty,
                };
                match array_ty.kind() {
                    ty::Array(ty, ..) | ty::Slice(ty) => Some(*ty),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn emit_error(&self, error_kind: FinalizerErrorKind<'tcx>) {
        let mut err;
        match error_kind {
            FinalizerErrorKind::NotSendAndSync(fi, pi) => {
                err = self.tcx.sess.psess.dcx.struct_span_err(
                    self.arg_span,
                    format!("The drop method for `{0}` cannot be safely finalized.", fi.drop_ty),
                );
                err.span_label(pi.span, format!("a finalizer cannot safely use this `{0}`", pi.ty));
                err.span_label(
                    pi.span,
                    "from a drop method because it does not implement `Send` + `Sync`.",
                );
                err.help("`Gc` runs finalizers on a separate thread, so drop methods\nmust only use values which are thread-safe.");
            }
            FinalizerErrorKind::NotFinalizerSafe(fi, pi) => {
                err = self.tcx.sess.psess.dcx.struct_span_err(
                    self.arg_span,
                    format!("The drop method for `{0}` cannot be safely finalized.", fi.drop_ty),
                );
                // Special-case `Gc` types for more friendly errors
                if pi.ty.is_gc(self.tcx) {
                    err.span_label(
                        pi.span,
                        format!("a finalizer cannot safely dereference this `{0}`", pi.ty),
                    );
                    err.span_label(
                        pi.span,
                        "from a drop method because it might have already been finalized.",
                    );
                } else {
                    err.span_label(
                        pi.span,
                        format!("a finalizer cannot safely use this `{0}`", pi.ty),
                    );
                    err.span_label(
                        pi.span,
                        "from a drop method because it does not implement `FinalizerSafe`.",
                    );
                    err.help("`Gc` runs finalizers on a separate thread, so drop methods\nmust only use values whose types implement `FinalizerSafe`.");
                }
            }
            FinalizerErrorKind::UnsoundReference(fi, pi) => {
                err = self.tcx.sess.psess.dcx.struct_span_err(
                    self.arg_span,
                    format!("The drop method for `{0}` cannot be safely finalized.", fi.drop_ty),
                );
                err.span_label(
                    pi.span,
                    format!("a finalizer cannot safely dereference this `{0}`", pi.ty),
                );
                err.span_label(pi.span, "because it might not live long enough.");
                err.help("`Gc` may run finalizers after the valid lifetime of this reference.");
            }
            FinalizerErrorKind::MissingFnDef(fi) => {
                err = self.tcx.sess.psess.dcx.struct_span_err(
                    self.arg_span,
                    format!("The drop method for `{0}` cannot be safely finalized.", fi.drop_ty),
                );
                err.span_label(fi.span, "this function call may be unsafe to use in a finalizer.");
            }
            FinalizerErrorKind::UnknownTraitObject(fi) => {
                err = self.tcx.sess.psess.dcx.struct_span_err(
                    self.arg_span,
                    format!("The drop method for `{0}` cannot be safely finalized.", fi.drop_ty),
                );
                err.span_label(
                    self.arg_span,
                    "contains a trait object whose implementation is unknown.",
                );
            }
            FinalizerErrorKind::UnsoundExternalDropGlue(fi) => {
                err = self.tcx.sess.psess.dcx.struct_span_err(
                    self.arg_span,
                    format!("The drop method for `{0}` cannot be safely finalized.", fi.drop_ty),
                );
                err.span_label(
                    fi.span,
                    format!("this `{0}` is not safe to be run as a finalizer", fi.drop_ty),
                );
            }
            FinalizerErrorKind::InlineAsm(fi) => {
                err = self.tcx.sess.psess.dcx.struct_span_err(
                    self.arg_span,
                    format!("The drop method for `{0}` cannot be safely finalized.", fi.drop_ty),
                );
                err.span_label(
                    fi.span,
                    format!("this assembly block is not safe to run in a finalizer"),
                );
            }
        }
        err.span_label(
            self.fn_span,
            format!("caused by trying to construct a `Gc<{}>` here.", self.value_ty),
        );
        err.emit();
    }
}

/// The central data structure for performing FSA on a particular type's drop method.
///
/// Constructed and used each time a new `T::drop()` is found in the MIR. Note that this *does not*
/// deal with drop glue. Instead, those drop methods which come from component types of `T` are
/// added in `FSAEntryPointCtxt::check_drop_glue()` to the stack of types to be processed
/// separately, where they get their own `DropCtxt`.
struct DropCtxt<'ecx, 'tcx> {
    /// Queue of function instances that need to be checked as part of this FSA pass. This is
    /// pushed to when a call is located in the MIR.
    callsites: VecDeque<ty::Instance<'tcx>>,
    /// The type of the value whose drop method we are currently checking. Used for emitting nicer,
    /// contextual FSA error messages.
    drop_ty: Ty<'tcx>,
    /// Context for the entry point (e.g `Gc::new` or `Gc::from`).
    ecx: &'ecx FSAEntryPointCtxt<'tcx>,
    /// The monomorphized function instances which have already been visited by FSA. This is a set
    /// because we want fast entry and fast lookup -- we don't care about ordering. This serves two
    /// purposes. First, as a cache to stop us unnecessarily checking (and thus emitting errors)
    /// for the same function definition more than once. Second, and more importantly, this allows
    /// us to deal with recursive function calls. Without this, recursive calls in `drop` would
    /// cause FSA to loop forever.
    visited_fns: FxHashSet<ty::Instance<'tcx>>,
}

impl<'ecx, 'tcx> DropCtxt<'ecx, 'tcx> {
    fn new(
        drop_instance: ty::Instance<'tcx>,
        drop_ty: Ty<'tcx>,
        ecx: &'ecx FSAEntryPointCtxt<'tcx>,
    ) -> Self {
        let mut callsites = VecDeque::default();
        callsites.push_back(drop_instance);
        Self { callsites, ecx, drop_ty, visited_fns: FxHashSet::default() }
    }

    fn check(mut self) -> Result<(), Vec<FinalizerErrorKind<'tcx>>> {
        let mut errors = Vec::new();
        loop {
            let Some(instance) = self.callsites.pop_front() else {
                break;
            };
            if self.visited_fns.contains(&instance) {
                // We've already checked this function. Ignore it!
                continue;
            }
            self.visited_fns.insert(instance);

            let Some(mir) = self.ecx.prefer_instantiated_mir(instance) else {
                bug!();
            };
            match FuncCtxt::new(&mir, &mut self).check() {
                Err(ref mut e) => errors.append(e),
                _ => (),
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}

struct FuncCtxt<'dcx, 'ecx, 'tcx> {
    body: &'dcx Body<'tcx>,
    dcx: &'dcx mut DropCtxt<'ecx, 'tcx>,
    errors: Vec<FinalizerErrorKind<'tcx>>,
    error_locs: FxHashSet<Location>,
}

impl<'dcx, 'ecx, 'tcx> FuncCtxt<'dcx, 'ecx, 'tcx> {
    fn new(body: &'dcx Body<'tcx>, dcx: &'dcx mut DropCtxt<'ecx, 'tcx>) -> Self {
        Self { body, dcx, errors: Vec::new(), error_locs: FxHashSet::default() }
    }

    fn check(mut self) -> Result<(), Vec<FinalizerErrorKind<'tcx>>> {
        self.visit_body(self.body);
        if self.errors.is_empty() { Ok(()) } else { Err(self.errors) }
    }

    fn push_error(&mut self, location: Location, error: FinalizerErrorKind<'tcx>) {
        if self.error_locs.contains(&location) {
            return;
        }

        self.errors.push(error);
        self.error_locs.insert(location);
    }

    fn tcx(&self) -> TyCtxt<'tcx> {
        self.dcx.ecx.tcx
    }

    fn ecx(&self) -> &'dcx FSAEntryPointCtxt<'tcx> {
        &self.dcx.ecx
    }
}

impl<'dcx, 'ecx, 'tcx> Visitor<'tcx> for FuncCtxt<'dcx, 'ecx, 'tcx> {
    fn visit_projection(
        &mut self,
        place_ref: PlaceRef<'tcx>,
        context: PlaceContext,
        location: Location,
    ) {
        // A single projection can be comprised of other 'inner' projections (e.g. self.a.b.c), so
        // this loop ensures that the types of each intermediate projection is extracted and then
        // checked.
        for ty in place_ref
            .iter_projections()
            .filter_map(|(base, elem)| self.ecx().extract_projection_ty(self.body, base, elem))
        {
            let fn_info = FnInfo::new(self.body.span, self.dcx.drop_ty);
            let proj_info = ProjInfo::new(self.body.source_info(location).span, ty);
            if ty.is_unsafe_ptr() {
                break;
            }
            if !ty.is_send(self.tcx(), self.ecx().param_env)
                || !ty.is_sync(self.tcx(), self.ecx().param_env)
            {
                self.push_error(location, FinalizerErrorKind::NotSendAndSync(fn_info, proj_info));
                break;
            }
            if ty.is_ref() {
                // Unfortunately, we can't relax this constraint to allow static refs for two
                // reasons:
                //      1. When this MIR transformation is called, all lifetimes have already
                //         been erased by borrow-checker.
                //      2. Unsafe code can and does transmute lifetimes up to 'static then use
                //         runtime properties to ensure that the reference is valid. FSA would
                //         not catch this and could allow unsound programs.
                self.push_error(location, FinalizerErrorKind::UnsoundReference(fn_info, proj_info));
                break;
            }
            if !ty.is_finalizer_safe(self.tcx(), self.ecx().param_env) {
                self.push_error(location, FinalizerErrorKind::NotFinalizerSafe(fn_info, proj_info));
                break;
            }
        }
        self.super_projection(place_ref, context, location);
    }

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
        let (instance, info) = match &terminator.kind {
            TerminatorKind::Call { func, fn_span, .. } => {
                match func.ty(self.body, self.tcx()).kind() {
                    ty::FnDef(fn_did, substs) => {
                        let info = FnInfo::new(*fn_span, self.dcx.drop_ty);
                        let Ok(instance) = ty::Instance::resolve(
                            self.tcx(),
                            self.ecx().param_env,
                            *fn_did,
                            substs,
                        ) else {
                            bug!();
                        };
                        (instance, info)
                    }
                    ty::FnPtr(..) => {
                        // FSA doesn't support function pointers so this will trigger an error down
                        // the line.
                        let span = terminator.source_info.span;
                        let info = FnInfo::new(span, self.dcx.drop_ty);
                        (None, info)
                    }
                    _ => bug!(),
                }
            }
            TerminatorKind::Drop { place, .. } => {
                let glue_ty = place.ty(self.body, self.tcx()).ty;
                let glue = ty::Instance::resolve_drop_in_place(self.tcx(), glue_ty);
                let ty::InstanceDef::DropGlue(_, ty) = glue.def else {
                    bug!();
                };

                if ty.is_none()
                    || ty.unwrap().ty_adt_def().map_or(true, |adt| !adt.has_dtor(self.tcx()))
                    || ty.unwrap().is_gc(self.tcx())
                {
                    // This check is necessary because FSA happens before optimisation passes like
                    // 'drop elaboration', so the MIR might contain drop terminators for types that
                    // don't actually have a drop method.
                    //
                    // In addition, we only care if the *top level* part of this type has a drop
                    // method. If any of its fields also require dropping then they will have
                    // separate MIR terminators because drop glue will have added them.
                    //
                    // We also have to check for, and ignore `Gc<T>`'s, because they have a
                    // destructor for the premature finalization barriers. This is FSA safe though.
                    self.super_terminator(terminator, location);
                    return;
                }
                let drop_trait_did = self.tcx().require_lang_item(LangItem::Drop, None);
                let poly_drop_fn_did = self.tcx().associated_item_def_ids(drop_trait_did)[0];
                let Ok(instance) = ty::Instance::resolve(
                    self.tcx(),
                    self.ecx().param_env,
                    poly_drop_fn_did,
                    self.tcx().mk_args(&[ty.unwrap().into()]),
                ) else {
                    bug!();
                };
                let span = terminator.source_info.span;
                let info = FnInfo::new(span, self.dcx.drop_ty);
                (instance, info)
            }
            TerminatorKind::InlineAsm { .. } => {
                let span = terminator.source_info.span;
                let info = FnInfo::new(span, self.dcx.drop_ty);
                self.push_error(location, FinalizerErrorKind::InlineAsm(info));
                return;
            }
            _ => {
                self.super_terminator(terminator, location);
                return;
            }
        };

        match instance {
            Some(instance) if self.tcx().is_mir_available(instance.def_id()) => {
                self.dcx.callsites.push_back(instance);
            }
            _ => self.push_error(location, FinalizerErrorKind::MissingFnDef(info)),
        };
        self.super_terminator(terminator, location);
    }
}

fn in_std_lib<'tcx>(tcx: TyCtxt<'tcx>, did: DefId) -> bool {
    let alloc_crate = tcx.get_diagnostic_item(sym::Rc).map_or(false, |x| did.krate == x.krate);
    let core_crate = tcx.get_diagnostic_item(sym::RefCell).map_or(false, |x| did.krate == x.krate);
    let std_crate = tcx.get_diagnostic_item(sym::Mutex).map_or(false, |x| did.krate == x.krate);
    alloc_crate || std_crate || core_crate
}
