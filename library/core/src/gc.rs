#![unstable(feature = "gc", issue = "none")]
#![allow(missing_docs)]

#[cfg(not(bootstrap))]
static MAX_LAYOUT: usize = crate::mem::size_of::<usize>() * 64;

#[unstable(feature = "gc", issue = "none")]
/// A type that implements this trait will be conservatively marked by the
/// collector. This takes precedence over `NoTrace`.
#[cfg_attr(not(bootstrap), lang = "conservative")]
pub trait Conservative {}

#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(not(bootstrap), lang = "no_finalize")]
pub trait NoFinalize {}

#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(not(bootstrap), lang = "notrace")]
pub auto trait NoTrace {}

#[unstable(feature = "gc", issue = "none")]
#[derive(Debug, PartialEq, Eq)]
pub struct Trace {
    pub bitmap: u64,
    pub size: u64,
}

#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(not(bootstrap), lang = "gcsp")]
/// An internal trait which is only implemented by the `Gc` type. This exists so
/// that the `Gc` type -- which is defined externally -- can be used as part of
/// static analysis in the compiler. Evenually, this won't be necessary because
/// `Gc` will be defined in the standard library.
pub unsafe trait GcSmartPointer {}

#[unstable(feature = "gc", issue = "none")]
impl Trace {
    #[inline]
    /// Returns true if rustgc wasn't able to create a precise descriptor for
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
