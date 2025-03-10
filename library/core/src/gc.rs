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
#[rustc_diagnostic_item = "drop_method_finalizer_elidable"]
#[cfg_attr(not(bootstrap), lang = "drop_method_finalizer_elidable")]
pub unsafe trait DropMethodFinalizerElidable {}

/// A wrapper which prevents `T` from being finalized when used in a `Gc`.
///
/// This is useful for when its not possible to implement `DropMethodFinalizerElidable`
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

