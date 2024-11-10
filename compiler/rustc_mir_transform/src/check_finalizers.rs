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

#[derive(PartialEq)]
pub struct CheckFinalizers;

#[derive(Debug)]
enum FinalizerErrorKind<'tcx> {
    /// Does not implement `Send` + `Sync`
    NotSendAndSync(Span),
    /// Does not implement `FinalizerSafe`
    NotFinalizerSafe(Ty<'tcx>, Span),
    /// Contains a field projection where one of the projection elements is a reference.
    UnsoundReference(Ty<'tcx>, Span),
    /// Uses a trait object whose concrete type is unknown
    UnknownTraitObject,
    /// Calls a function whose definition is unavailable, so we can't be certain it's safe.
    MissingFnDef,
    /// The drop glue contains an unsound drop method from an external crate. This will have been
    /// caused by one of the above variants. However, it is confusing to propagate this to the user
    /// because they most likely won't be in a position to fix it from a downstream crate. Currently
    /// this only applies to types belonging to the standard library.
    UnsoundExternalDropGlue(Span),
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
            if !in_std_lib(tcx, *fn_did) || !fn_ty.fn_sig(tcx).output().skip_binder().is_gc(tcx) {
                continue;
            }
            let mono_fn_did = ty::Instance::resolve(tcx, param_env, *fn_did, substs)
                .unwrap()
                .unwrap()
                .def
                .def_id();
            if !tcx.has_attr(mono_fn_did, sym::rustc_fsa_entry_point) {
                // Skip over any call that's not marked #[rustc_fsa_entry_point]
                continue;
            }

            assert_eq!(args.len(), 1);
            let arg_ty = args[0].node.ty(body, tcx);
            FSAEntryPointCtxt::new(source_info.span, args[0].span, arg_ty, tcx, param_env)
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
    /// Type of the arg to the entry point. This could be deduced from the field above but it is
    /// inconvenient.
    arg_ty: Ty<'tcx>,
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
}

impl<'tcx> FSAEntryPointCtxt<'tcx> {
    fn new(
        fn_span: Span,
        arg_span: Span,
        arg_ty: Ty<'tcx>,
        tcx: TyCtxt<'tcx>,
        param_env: ParamEnv<'tcx>,
    ) -> Self {
        Self { fn_span, arg_span, arg_ty, tcx, param_env }
    }

    fn check_drop_glue(&self) {
        if !self.arg_ty.needs_finalizer(self.tcx, self.param_env)
            || self.arg_ty.is_finalize_unchecked(self.tcx)
        {
            return;
        }

        if self.arg_ty.is_send(self.tcx, self.param_env)
            && self.arg_ty.is_sync(self.tcx, self.param_env)
            && self.arg_ty.is_finalizer_safe(self.tcx, self.param_env)
        {
            return;
        }

        let mut errors = Vec::new();
        let mut tys = vec![self.arg_ty];

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
                    errors.push(FinalizerErrorKind::UnknownTraitObject);
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
                        errors.push(FinalizerErrorKind::MissingFnDef);
                    }
                    if def.has_dtor(self.tcx) {
                        match DropMethodChecker::new(self.drop_mir(ty), self).check() {
                            Err(_) if in_std_lib(self.tcx, def.did()) => {
                                errors.push(FinalizerErrorKind::UnsoundExternalDropGlue(
                                    self.drop_mir(ty).span,
                                ));
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

    fn drop_mir<'a>(&self, ty: Ty<'tcx>) -> &'a Body<'tcx> {
        let ty::Adt(_, substs) = ty.kind() else {
            bug!();
        };
        let dt = self.tcx.require_lang_item(LangItem::Drop, None);
        let df = self.tcx.associated_item_def_ids(dt)[0];
        let s = self.tcx.mk_args_trait(ty, substs.into_iter());
        let i = ty::Instance::resolve(self.tcx, self.param_env, df, s).unwrap().unwrap();
        self.tcx.instance_mir(i.def)
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
        let snippet = self.tcx.sess.source_map().span_to_snippet(self.arg_span).unwrap();
        let mut err = self.tcx.sess.psess.dcx.struct_span_err(
            self.arg_span,
            format!("`{snippet}` has a drop method which cannot be safely finalized."),
        );
        match error_kind {
            FinalizerErrorKind::NotSendAndSync(span) => {
                err.span_label(span, "caused by the expression in `fn drop(&mut)` here because");
                err.span_label(span, "it uses a type which is not safe to use in a finalizer.");
                err.help("`Gc` runs finalizers on a separate thread, so drop methods\nmust only use values whose types implement `Send` + `Sync`.");
            }
            FinalizerErrorKind::NotFinalizerSafe(ty, span) => {
                // Special-case `Gc` types for more friendly errors
                if ty.is_gc(self.tcx) {
                    err.span_label(
                        span,
                        "caused by the expression here in `fn drop(&mut)` because",
                    );
                    err.span_label(span, "it uses another `Gc` type.");
                    err.span_label(
                        self.fn_span,
                        format!("Finalizers cannot safely dereference other `Gc`s, because they might have already been finalised."),
                    );
                } else {
                    err.span_label(
                        span,
                        "caused by the expression in `fn drop(&mut)` here because",
                    );
                    err.span_label(
                        span,
                        "it uses a type which is not safe to use in a finalizer.",
                    );
                    err.help("`Gc` runs finalizers on a separate thread, so drop methods\nmust only use values whose types implement `FinalizerSafe`.");
                    err.span_label(
                        self.fn_span,
                        format!(
                            "`Gc::new` requires that {ty} implements the `FinalizeSafe` trait.",
                        ),
                    );
                }
            }
            FinalizerErrorKind::UnsoundReference(ty, span) => {
                err.span_label(span, "caused by the expression here in `fn drop(&mut)` because");
                err.span_label(
                    span,
                    format!("it is a reference ({ty}) which is not safe to use in a finalizer."),
                );
                err.help("`Gc` may run finalizers after the valid lifetime of this reference.");
            }
            FinalizerErrorKind::MissingFnDef => {
                err.span_label(self.arg_span, "contains a function call which may be unsafe.");
            }
            FinalizerErrorKind::UnknownTraitObject => {
                err.span_label(
                    self.arg_span,
                    "contains a trait object whose implementation is unknown.",
                );
            }
            FinalizerErrorKind::UnsoundExternalDropGlue(span) => {
                err.span_label(span, "is not safe to be run as a finalizer");
            }
        }
        err.emit();
    }
}

struct DropMethodChecker<'ecx, 'tcx> {
    body: &'ecx Body<'tcx>,
    ecx: &'ecx FSAEntryPointCtxt<'tcx>,
    errors: Vec<FinalizerErrorKind<'tcx>>,
    error_locs: FxHashSet<Location>,
}

impl<'ecx, 'tcx> DropMethodChecker<'ecx, 'tcx> {
    fn new(body: &'ecx Body<'tcx>, ecx: &'ecx FSAEntryPointCtxt<'tcx>) -> Self {
        Self { body, ecx, errors: Vec::new(), error_locs: FxHashSet::default() }
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
}

impl<'ecx, 'tcx> Visitor<'tcx> for DropMethodChecker<'ecx, 'tcx> {
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
            .filter_map(|(base, elem)| self.ecx.extract_projection_ty(self.body, base, elem))
        {
            let span = self.body.source_info(location).span;
            if !ty.is_send(self.ecx.tcx, self.ecx.param_env)
                || !ty.is_sync(self.ecx.tcx, self.ecx.param_env)
            {
                self.push_error(location, FinalizerErrorKind::NotSendAndSync(span));
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
                self.push_error(location, FinalizerErrorKind::UnsoundReference(ty, span));
                break;
            }
            if !ty.is_finalizer_safe(self.ecx.tcx, self.ecx.param_env) {
                self.push_error(location, FinalizerErrorKind::NotFinalizerSafe(ty, span));
                break;
            }
        }
        self.super_projection(place_ref, context, location);
    }

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, _: Location) {
        if let TerminatorKind::Call { ref args, .. } = terminator.kind {
            for caller_arg in self.body.args_iter() {
                let recv_ty = self.body.local_decls()[caller_arg].ty;
                for arg in args.iter() {
                    let arg_ty = arg.node.ty(self.body, self.ecx.tcx);
                    if arg_ty == recv_ty {
                        // Currently, we do not recurse into function calls
                        // to see whether they access `!FinalizerSafe`
                        // fields, so we must throw an error in `drop`
                        // methods which call other functions and pass
                        // `self` as an argument.
                        //
                        // Here, we throw an error if `drop(&mut self)`
                        // calls a function with an argument that has the
                        // same type as the drop receiver (e.g. foo(x:
                        // &Self)). This approximation will always prevent
                        // unsound `drop` methods, however, it is overly
                        // conservative and will prevent correct examples
                        // like below from compiling:
                        //
                        // ```
                        // fn drop(&mut self) {
                        //   let x = Self { ... };
                        //   x.foo();
                        // }
                        // ```
                        //
                        // This example is sound, because `x` is a local
                        // that was instantiated on the finalizer thread, so
                        // its fields are always safe to access from inside
                        // this drop method.
                        //
                        // However, this will not compile, because the
                        // receiver for `x.foo()` is the same type as the
                        // `self` reference. To fix this, we would need to
                        // do a def-use analysis on the self reference to
                        // find every MIR local which refers to it that ends
                        // up being passed to a call terminator. This is not
                        // trivial to do at the moment.
                        self.errors.push(FinalizerErrorKind::MissingFnDef);
                    }
                }
            }
        }
    }
}

fn in_std_lib<'tcx>(tcx: TyCtxt<'tcx>, did: DefId) -> bool {
    let alloc_crate = tcx.get_diagnostic_item(sym::Rc).map_or(false, |x| did.krate == x.krate);
    let core_crate = tcx.get_diagnostic_item(sym::RefCell).map_or(false, |x| did.krate == x.krate);
    let std_crate = tcx.get_diagnostic_item(sym::Mutex).map_or(false, |x| did.krate == x.krate);
    alloc_crate || std_crate || core_crate
}
