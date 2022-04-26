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

/// Prevents a type from being finalized when used in `Gc`.
///
/// If a type `T` implements `NoFinalize`, a finalizer will not be registered to
/// call its drop method when passed to `Gc::new`, regardless of whether its
/// component types require finalization.
///
/// # Safety
///
/// Unsafe because this should be used with care. Preventing drop from
/// running can lead to surprising behaviour. In particular, this will also
/// prevent the finalization of all component types of T.
#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(not(bootstrap), lang = "no_finalize")]
pub unsafe trait NoFinalize {}

#[cfg_attr(not(bootstrap), lang = "flz_comps")]
/// Prevents a type from being finalized by GC if none of the component types
/// need dropping. This can be thought of as a weaker version of `NoFinalize`.
///
/// # Safety
///
/// Unsafe because this should be used with care. Preventing drop from
/// running can lead to surprising behaviour.
pub unsafe trait OnlyFinalizeComponents {}

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
/// This has the same effect as implementing `NoFinalize` trait on `T`, however,
/// due to the orphan rule this is not always possible. `NonFinalizable` acts as
/// a convenience wrapper.
#[derive(Debug)]
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

#[cfg_attr(not(bootstrap), lang = "collectable")]
/// A type that implements this trait provides a way to dynamically change its
/// heap allocation from non-GC'd to GC'd.
///
/// # Safety
///
/// Setting a value to collectable means that the GC will deallocate it when it
/// is no longer reachable. Once set, the value must not be accessed via an
/// obfuscated pointer which was hidden from the GC (e.g. as part of a XOR
/// linked list)
pub unsafe trait Collectable {
    #[cfg_attr(not(bootstrap), lang = "set_collectable")]
    unsafe fn set_collectable(&self);
}

unsafe impl<T> NoFinalize for NonFinalizable<T> {}

mod impls {
    use super::NoFinalize;

    macro_rules! impl_nofinalize {
        ($($t:ty)*) => (
            $(
                #[unstable(feature = "gc", issue = "none")]
                unsafe impl NoFinalize for $t {}
            )*
        )
    }

    impl_nofinalize! {
        usize u8 u16 u32 u64 u128
            isize i8 i16 i32 i64 i128
            f32 f64
            bool char
    }

    #[unstable(feature = "never_type", issue = "35121")]
    unsafe impl NoFinalize for ! {}
}
