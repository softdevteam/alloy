#![unstable(feature = "gc", issue = "none")]
#![allow(missing_docs)]
use crate::ops::{Deref, DerefMut};

#[cfg(not(bootstrap))]
static MAX_LAYOUT: usize = crate::mem::size_of::<usize>() * 64;

#[unstable(feature = "gc", issue = "none")]
/// A type that implements this trait will be conservatively marked by the
/// collector. This takes precedence over `NoTrace`.
#[cfg_attr(not(bootstrap), lang = "conservative")]
pub trait Conservative {}

/// Prevents a type from being finalized by GC if none of the component types
/// need dropping.
///
/// # Safety
///
/// Unsafe because this should be used with care. Preventing drop from
/// running can lead to surprising behaviour.
#[rustc_diagnostic_item = "finalizer_optional"]
pub unsafe trait FinalizerOptional {}

#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(not(bootstrap), lang = "notrace")]
pub auto trait NoTrace {}

#[unstable(feature = "gc", issue = "none")]
#[derive(Debug, PartialEq, Eq)]
pub struct Trace {
    pub bitmap: u64,
    pub size: u64,
}

/// A wrapper which prevents `T` from being finalized when used in a `Gc`.
///
/// This is useful for when its not possible to implement `FinalizerOptional`
/// because of the orphan rule. However, if `NonFinalizable<T>` is used as a
/// field type of another type which is finalizable, then `T` will also be
/// finalized.
#[derive(Debug, PartialEq, Eq)]
#[rustc_diagnostic_item = "non_finalizable"]
pub struct NonFinalizable<T: ?Sized>(T);

impl<T> NonFinalizable<T> {
    /// Wrap a value to prevent finalization in `Gc`.
    pub fn new(value: T) -> NonFinalizable<T> {
        NonFinalizable(value)
    }
}

#[unstable(feature = "gc", issue = "none")]
impl Trace {
    #[inline]
    /// Returns true if Alloy wasn't able to create a precise descriptor for
    /// the type.
    pub fn must_use_conservative(&self) -> bool {
        self.bitmap == 1 && self.size == 0
    }
}

#[unstable(feature = "gc", issue = "none")]
#[cfg(not(bootstrap))]
pub const fn needs_tracing<T>() -> bool {
    crate::intrinsics::needs_tracing::<T>()
}

#[unstable(feature = "gc", issue = "none")]
#[cfg(not(bootstrap))]
pub fn can_trace_precisely<T>() -> bool {
    crate::intrinsics::can_trace_precisely::<T>()
}

#[unstable(feature = "gc", issue = "none")]
#[cfg(not(bootstrap))]
/// Returns a pair describing the layout of the type for use by the collector.
///
/// # Safety
///
/// The type T must be smaller or equal in size to `size_of::<usize> * 64`.
pub unsafe fn gc_layout<T>() -> Trace {
    debug_assert!(crate::mem::size_of::<T>() <= MAX_LAYOUT);
    let layout = crate::intrinsics::gc_layout::<T>();
    Trace { bitmap: layout[0], size: layout[1] }
}

impl<T: ?Sized> !NoTrace for *mut T {}
impl<T: ?Sized> !NoTrace for *const T {}

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

#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(not(test), rustc_diagnostic_item = "ReferenceFree")]
pub auto trait ReferenceFree {}

impl<T> !ReferenceFree for &T {}
impl<T> !ReferenceFree for &mut T {}
