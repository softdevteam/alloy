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

