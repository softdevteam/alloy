#![unstable(feature = "gc", issue = "none")]
#![allow(missing_docs)]
use crate::ops::{Deref, DerefMut};

/// Prevents a type from being finalized by GC if none of the component types
/// need dropping.
///
/// # Safety
///
/// Unsafe because this should be used with care. Preventing drop from
/// running can lead to surprising behaviour.
#[rustc_diagnostic_item = "finalizer_optional"]
#[cfg_attr(not(bootstrap), lang = "finalizer_optional")]
pub unsafe trait FinalizerOptional {}

/// A wrapper which prevents `T` from being finalized when used in a `Gc`.
///
/// This is useful for when its not possible to implement `FinalizerOptional`
/// because of the orphan rule. However, if `NonFinalizable<T>` is used as a
/// field type of another type which is finalizable, then `T` will also be
/// finalized.
#[derive(Debug, PartialEq, Eq, Clone)]
#[rustc_diagnostic_item = "non_finalizable"]
pub struct NonFinalizable<T: ?Sized>(T);

impl<T> NonFinalizable<T> {
    /// Wrap a value to prevent finalization in `Gc`.
    pub fn new(value: T) -> NonFinalizable<T> {
        NonFinalizable(value)
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized> Deref for NonFinalizable<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        &self.0
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized> DerefMut for NonFinalizable<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

/// A wrapper to prevent alloy from performing Finaliser Safety Analysis (FSA)
/// on `T`.
///
/// FSA is a compile-time analysis performed by alloy which checks whether it is
/// sound to call a type's drop method by a garbage collection finaliser. It
/// works by looking at each line in T's drop method for potential soundness
/// violations.
///
/// However, where this is too strict -- and the user knows T::drop to be sound
/// -- `FinalizeUnchecked` can be used to opt-out of FSA. This is preferable to
/// implementing the `FinalizerSafe` trait for `T` as `FinalizeUnchecked`
/// applies only to individual uses of `T`.
#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(not(test), rustc_diagnostic_item = "FinalizeUnchecked")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FinalizeUnchecked<T: ?Sized>(T);

impl<T> FinalizeUnchecked<T> {
    pub unsafe fn new(value: T) -> Self {
        FinalizeUnchecked(value)
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized> Deref for FinalizeUnchecked<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        &self.0
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized> DerefMut for FinalizeUnchecked<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

#[cfg(not(bootstrap))]
unsafe impl<T> FinalizerSafe for FinalizeUnchecked<T> {}

#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(not(test), rustc_diagnostic_item = "ReferenceFree")]
pub auto trait ReferenceFree {}

impl<T> !ReferenceFree for &T {}
impl<T> !ReferenceFree for &mut T {}
