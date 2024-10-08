//! Multi-threaded garbage-collected pointers. 'Gc' stands for 'Garbage
//! Collected'.
//!
//! The type [`Gc<T>`][`Gc`] provides shared ownership of a value of type `T`,
//! allocated in the heap. [`Gc`] pointers are copyable, with copied [`Gc`]s
//! pointing to the same allocation in the heap.
//!
//! The allocation referenced by a [`Gc`] pointer is guaranteed not to be
//! dropped while there are still references to it. When there are no longer any
//! references, the garbage collector will drop it, calling any finalisers (in
//! non-deterministic order) in another thread. The garbage collector runs
//! intermittently in the background, so [`Gc`] pointers may live longer than
//! they need to, and cannot be relied on to drop values deterministically.
//!
//! Shared references in Rust disallow mutation by default, and [`Gc`] is no
//! exception: you cannot generally obtain a mutable reference to something
//! inside an [`Gc`]. If you need mutability, put a [`Cell`] or [`RefCell`]
//! inside the [`Gc`].
//!
//! Unlike [`Rc`], cycles between [`Gc`] pointers are allowed and can be
//! deallocated without issue.
//!
//! If the `T` in a [`Gc`] has a [`Drop`] method, it will be run using a
//! finalizer before being deallocated.
//!
//! `Gc<T>` automatically dereferences to `T` (via the [`Deref`] trait), so you
//! can call `T`'s methods on a value of type [`Gc<T>`][`Gc`]. To avoid name
//! clashes with `T`'s methods, the methods of [`Gc<T>`][`Gc`] itself are
//! associated functions, called using [fully qualified syntax].
//!
//! [`Cell`]: core::cell::Cell
//! [`RefCell`]: core::cell::RefCell
//! [send]: core::marker::Send
//! [`Rc`]: crate::rc::Rc
//! [`Deref`]: core::ops::Deref
//! [mutability]: core::cell#introducing-mutability-inside-of-something-immutable
//! [fully qualified syntax]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#fully-qualified-syntax-for-disambiguation-calling-methods-with-the-same-name
#![allow(missing_docs)]

use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    any::Any,
    cmp::{self, Ordering},
    fmt,
    hash::{Hash, Hasher},
    marker::Unsize,
    mem::{align_of_val_raw, MaybeUninit},
    ops::{CoerceUnsized, Deref, DispatchFromDyn, Receiver},
    ptr::{self, drop_in_place, NonNull},
};

pub use core::gc::*;

#[cfg(profile_gc)]
use core::sync::atomic::{self, AtomicU64};

use core::sync::atomic::{self, AtomicU64};

#[cfg(test)]
mod tests;

#[derive(Default)]
struct GcCounters {
    finalizers_registered: AtomicU64,
    allocated_gc: AtomicU64,
    allocated_normal: AtomicU64,
}

#[derive(Debug, Copy, Clone)]
pub struct GcStats {
    pub finalizers_registered: u64,
    pub finalizers_completed: u64,
    pub allocated_gc: u64,
    pub allocated_normal: u64,
    pub num_gcs: u64,
}

static GC_COUNTERS: GcCounters = GcCounters {
    finalizers_registered: AtomicU64::new(0),
    allocated_gc: AtomicU64::new(0),
    allocated_normal: AtomicU64::new(0),
};

#[cfg(profile_gc)]
static FINALIZERS_REGISTERED: AtomicU64 = AtomicU64::new(0);
#[cfg(profile_gc)]
static FINALIZERS_COMPLETED: AtomicU64 = AtomicU64::new(0);

////////////////////////////////////////////////////////////////////////////////
// BDWGC Allocator
////////////////////////////////////////////////////////////////////////////////

// Fast-path for low alignment values
pub const MIN_ALIGN: usize = 8;

#[derive(Debug)]
pub struct GcAllocator;

unsafe impl GlobalAlloc for GcAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        GC_COUNTERS.allocated_normal.fetch_add(1, atomic::Ordering::Relaxed);
        unsafe { gc_malloc(layout) }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { gc_free(ptr, layout) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { gc_realloc(ptr, layout, new_size) }
    }
}

#[inline]
unsafe fn gc_malloc(layout: Layout) -> *mut u8 {
    if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
        unsafe { bdwgc::GC_malloc(layout.size()) as *mut u8 }
    } else {
        let mut out = ptr::null_mut();
        // posix_memalign requires that the alignment be a multiple of `sizeof(void*)`.
        // Since these are all powers of 2, we can just use max.
        unsafe {
            let align = layout.align().max(core::mem::size_of::<usize>());
            let ret = bdwgc::GC_posix_memalign(&mut out, align, layout.size());
            if ret != 0 { ptr::null_mut() } else { out as *mut u8 }
        }
    }
}

#[inline]
unsafe fn gc_realloc(ptr: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
    if old_layout.align() <= MIN_ALIGN && old_layout.align() <= new_size {
        unsafe { bdwgc::GC_realloc(ptr, new_size) as *mut u8 }
    } else {
        unsafe {
            let new_layout = Layout::from_size_align_unchecked(new_size, old_layout.align());

            let new_ptr = gc_malloc(new_layout);
            if !new_ptr.is_null() {
                let size = cmp::min(old_layout.size(), new_size);
                ptr::copy_nonoverlapping(ptr, new_ptr, size);
                gc_free(ptr, old_layout);
            }
            new_ptr
        }
    }
}

#[inline]
unsafe fn gc_free(ptr: *mut u8, _: Layout) {
    unsafe {
        bdwgc::GC_free(ptr);
    }
}

unsafe impl Allocator for GcAllocator {
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        GC_COUNTERS.allocated_gc.fetch_add(1, atomic::Ordering::Relaxed);
        match layout.size() {
            0 => Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0)),
            size => unsafe {
                let ptr = gc_malloc(layout);
                let ptr = NonNull::new(ptr).ok_or(AllocError)?;
                Ok(NonNull::slice_from_raw_parts(ptr, size))
            },
        }
    }

    unsafe fn deallocate(&self, _: NonNull<u8>, _: Layout) {}
}

impl GcAllocator {
    pub fn force_gc() {
        unsafe { bdwgc::GC_gcollect() }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Free functions
////////////////////////////////////////////////////////////////////////////////
pub fn stats() -> GcStats {
    GcStats {
        finalizers_registered: GC_COUNTERS.finalizers_registered.load(atomic::Ordering::Relaxed),
        finalizers_completed: unsafe { bdwgc::GC_finalized_total() },
        allocated_gc: GC_COUNTERS.allocated_gc.load(atomic::Ordering::Relaxed),
        allocated_normal: GC_COUNTERS.allocated_normal.load(atomic::Ordering::Relaxed),
        num_gcs: unsafe { bdwgc::GC_get_gc_no() },
    }
}

pub fn init() {
    unsafe { bdwgc::GC_set_markers_count(1) }
    unsafe { bdwgc::GC_init() }
}

pub fn suppress_warnings() {
    unsafe { bdwgc::GC_set_warn_proc(&bdwgc::GC_ignore_warn_proc as *const _ as *mut u8) };
}

pub fn thread_registered() -> bool {
    unsafe { bdwgc::GC_thread_is_registered() != 0 }
}

pub fn keep_alive<T>(ptr: *mut T) {
    unsafe { bdwgc::GC_keep_alive(ptr as *mut u8) }
}

////////////////////////////////////////////////////////////////////////////////
// GC API
////////////////////////////////////////////////////////////////////////////////

struct GcBox<T: ?Sized> {
    /// The object being garbage collected.
    value: T,
}

/// A multi-threaded garbage collected pointer.
///
/// See the [module-level documentation](./index.html) for more details.
#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(all(not(bootstrap), not(test)), lang = "gc")]
#[cfg_attr(not(test), rustc_diagnostic_item = "gc")]
pub struct Gc<T: ?Sized> {
    ptr: NonNull<GcBox<T>>,
}

unsafe impl<T: ?Sized + Send> Send for Gc<T> {}
unsafe impl<T: ?Sized + Sync + Send> Sync for Gc<T> {}

// In non-topological finalization, it is unsound to deref any fields of type
// `Gc` from within a finalizer. This is because it could have been finalized
// first, thus resulting in a dangling reference. Marking this as
// `!FinalizerSafe` will give a nice compiler error if the user does so.
//
// FIXME: Make this conditional based on whether -DTOPOLOGICAL_FINALIZATION flag
// is passed to the compiler.
impl<T: ?Sized> !FinalizerSafe for Gc<T> {}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Gc<U>> for Gc<T> {}
#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Unsize<U>, U: ?Sized> DispatchFromDyn<Gc<U>> for Gc<T> {}

/// A compiler barrier to prevent finalizers running before the last reference to
/// an object is dead.
///
/// The compiler is free to optimise away the stack or register location holding
/// a GC reference if it's no longer used. This means that sometimes, at
/// runtime, a reference is cleaned up earlier than its source-level lifetime to
/// free up the register for something else. This is fine (and usually
/// desirable!) because it doesn't have any observable difference in behaviour.
///
/// However, things get complicated when a garbage collector is involved. In
/// very rare cases, this optimisation, followed by an unfortunately timed
/// collection, may cause the value the reference points to to be freed earlier
/// than expected - and thus finalized earlier than it should be. This can cause
/// deadlocks, races, and even use-after-frees.
///
/// Adding a compiler barrier to `Gc`'s drop prevents the compiler from optimizing
/// away the reference too soon. This is a special implementation with compiler
/// support, because it is usually impossible to allow both `Drop` and `Copy`
/// traits to be implemented on a type simultaneously.
#[cfg(all(not(bootstrap), not(test)))]
impl<T: ?Sized> Drop for Gc<T> {
    fn drop(&mut self) {
        keep_alive(self);
    }
}

impl<T: ?Sized> Gc<T> {
    #[inline(always)]
    fn inner(&self) -> &GcBox<T> {
        // This unsafety is ok because while this Gc is alive we're guaranteed
        // that the inner pointer is valid.
        unsafe { self.ptr.as_ref() }
    }

    unsafe fn from_inner(ptr: NonNull<GcBox<T>>) -> Self {
        Self { ptr }
    }

    unsafe fn from_ptr(ptr: *mut GcBox<T>) -> Self {
        unsafe { Self::from_inner(NonNull::new_unchecked(ptr)) }
    }

    /// Get a `Gc<T>` from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `raw` was allocated with `Gc::new()`.
    ///
    /// It is legal for `raw` to be an interior pointer if `T` is valid for the
    /// size and alignment of the originally allocated block.
    #[unstable(feature = "gc", issue = "none")]
    pub fn from_raw(raw: *const T) -> Gc<T> {
        let layout = Layout::new::<GcBox<()>>();
        // Align the unsized value to the end of the GcBox.
        // Because GcBox is repr(C), it will always be the last field in memory.
        // SAFETY: since the only unsized types for T possible are slices, trait objects,
        // and extern types, the input safety requirement is currently enough to
        // satisfy the requirements of align_of_val_raw.
        let raw_align = unsafe { align_of_val_raw(raw) };
        let offset = layout.size() + layout.padding_needed_for(raw_align);
        // Reverse the offset to find the original GcBox.
        let box_ptr = unsafe { raw.byte_sub(offset) as *mut GcBox<T> };
        unsafe { Self::from_ptr(box_ptr) }
    }

    /// Get a raw pointer to the underlying value `T`.
    #[unstable(feature = "gc", issue = "none")]
    pub fn into_raw(this: Self) -> *const T {
        Self::as_ptr(&this)
    }

    /// Get a raw pointer to the underlying value `T`.
    #[unstable(feature = "gc", issue = "none")]
    pub fn as_ptr(this: &Self) -> *const T {
        let ptr: *mut GcBox<T> = NonNull::as_ptr(this.ptr);
        unsafe { ptr::addr_of_mut!((*ptr).value) }
    }

    #[unstable(feature = "gc", issue = "none")]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        crate::ptr::addr_eq(this.ptr.as_ptr(), other.ptr.as_ptr())
    }
}

impl<T> Gc<T> {
    /// Constructs a new `Gc<T>`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::gc::Gc;
    ///
    /// let five = Gc::new(5);
    /// ```
    #[cfg(not(no_global_oom_handling))]
    #[unstable(feature = "gc", issue = "none")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "gc_ctor")]
    pub fn new(value: T) -> Self {
        unsafe { Self::new_internal(value) }
    }
}

impl<T> Gc<T> {
    /// Constructs a new `Gc<T>` which will never finalize the value of `T`.
    /// This means that if `T` implements [`Drop`], its [drop method] will never
    /// be called.
    ///
    /// This is useful when you need a `Gc<T>` where `T` does not implement
    /// [`Send`]. The requirement that `T: Send` is only necessary for
    /// finalization because the garbage collector finalizes values on a
    /// separate thread.
    ///
    /// This method should be used with caution: while it is safe to omit
    /// running `drop`, it is a common way to unintentionally cause memory
    /// leaks.
    ///
    /// [`Drop`]: core::ops::Drop
    /// [`drop method`]: core::ops::Drop#tymethod.drop
    /// [`Send`]: core::marker::Send
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// # #![feature(negative_impls)]
    /// use std::gc::Gc;
    ///
    /// struct Unsend(usize);
    ///
    /// impl !Send for Unsend {}
    ///
    /// let five = Gc::new_unfinalizable(Unsend(5));
    /// ```
    #[cfg(not(no_global_oom_handling))]
    #[unstable(feature = "gc", issue = "none")]
    pub fn new_unfinalizable(value: T) -> Self {
        unsafe { Self::new_internal(value) }
    }

    /// Constructs a new `Gc<T>` which will finalize the value of `T` (if it
    /// needs dropping) on a separate thread, even if `T` does not implement
    /// [`Sync`].
    ///
    /// This is useful for when you need a `Gc<T>` with interior mutabilty, but
    /// do not want to use the more expensive mutabilty containers such as
    /// `RWLock` or `Mutex`.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the drop implementation can not introduce
    /// a race condition. If the allocation points to shared data (e.g. via a
    /// field of type `Arc<RefCell<U>>`), then that field cannot be used inside
    /// the drop implementation. This is because the finalisation thread could
    /// run concurrently while that shared data is accessed without
    /// synchronisation elsewhere.
    ///
    /// [`Sync`]: core::marker::Sync
    #[cfg(not(no_global_oom_handling))]
    #[unstable(feature = "gc", issue = "none")]
    pub unsafe fn new_unsynchronised(value: T) -> Self {
        unsafe { Self::new_internal(value) }
    }

    #[inline(always)]
    #[cfg(not(no_global_oom_handling))]
    unsafe fn new_internal(value: T) -> Self {
        #[cfg(not(bootstrap))]
        if !crate::mem::needs_finalizer::<T>() {
            return Self::from_inner(Box::leak(Box::new_in(GcBox { value }, GcAllocator)).into());
        }

        unsafe extern "C" fn finalizer_shim<T>(obj: *mut u8, _: *mut u8) {
            let drop_fn = drop_in_place::<GcBox<T>>;
            drop_fn(obj as *mut GcBox<T>);
        }

        // By explicitly using type parameters here, we force rustc to compile monomorphised drop
        // glue for `GcBox<T>`. This ensures that the fn pointer points to the correct drop method
        // (or chain of drop methods) for the type `T`.
        //
        // Note that even though `GcBox` has no drop implementation, we still reify a
        // `drop_in_place` for `GcBox<T>`, and not`T`. This is because `T` may have an alignment
        // such that it is stored at some offset inside `GcBox`. Calling `drop_in_place::<GcBox<T>>`
        // will therefore generates drop glue for `T` which offsets the object pointer by the
        // required amount of padding for `T` if necessary. If we did not do this, we'd have to
        // manually ensure that the object pointer is correctly offset before the collector calls
        // the finaliser.
        let ptr = Box::leak(Box::new_in(GcBox { value }, GcAllocator));
        unsafe {
            bdwgc::GC_register_finalizer_no_order(
                ptr as *mut _ as *mut u8,
                Some(finalizer_shim::<T>),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
        }
        GC_COUNTERS.finalizers_registered.fetch_add(1, atomic::Ordering::Relaxed);
        Self::from_inner(ptr.into())
    }
}

#[cfg(profile_gc)]
#[derive(Debug)]
pub struct FinalizerInfo {
    pub registered: u64,
    pub completed: u64,
}

#[cfg(profile_gc)]
impl FinalizerInfo {
    pub fn finalizer_info() -> FinalizerInfo {
        FinalizerInfo {
            registered: FINALIZERS_REGISTERED.load(atomic::Ordering::Relaxed),
            completed: FINALIZERS_COMPLETED.load(atomic::Ordering::Relaxed),
        }
    }
}

impl Gc<dyn Any> {
    /// Attempt to downcast the `Gc<dyn Any>` to a concrete type.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::any::Any;
    /// use std::gc::Gc;
    ///
    /// fn print_if_string(value: Gc<dyn Any>) {
    ///     if let Ok(string) = value.downcast::<String>() {
    ///         println!("String ({}): {}", string.len(), string);
    ///     }
    /// }
    ///
    /// let my_string = "Hello World".to_string();
    /// print_if_string(Gc::new(my_string));
    /// print_if_string(Gc::new(0i8));
    /// ```
    #[inline]
    #[unstable(feature = "gc", issue = "none")]
    pub fn downcast<T: Any>(self) -> Result<Gc<T>, Gc<dyn Any>> {
        if (*self).is::<T>() {
            unsafe {
                let ptr = self.ptr.cast::<GcBox<T>>();
                Ok(Gc::from_inner(ptr))
            }
        } else {
            Err(self)
        }
    }

    /// Downcasts the `Gc<dyn Any>` to a concrete type.
    ///
    /// For a safe alternative see [`downcast`].
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    /// use std::gc::Gc;
    ///
    /// let x: Gc<dyn Any> = Gc::new(1_usize);
    ///
    /// unsafe {
    ///     assert_eq!(*x.downcast_unchecked::<usize>(), 1);
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    ///
    ///
    /// [`downcast`]: Self::downcast
    #[inline]
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    pub unsafe fn downcast_unchecked<T: Any>(self) -> Gc<T> {
        unsafe {
            let ptr = self.ptr.cast::<GcBox<T>>();
            Gc::from_inner(ptr)
        }
    }
}

impl<T: Send + Sync> Gc<MaybeUninit<T>> {
    /// As with `MaybeUninit::assume_init`, it is up to the caller to guarantee
    /// that the inner value really is in an initialized state. Calling this
    /// when the content is not yet fully initialized causes immediate undefined
    /// behaviour.
    #[unstable(feature = "gc", issue = "none")]
    pub unsafe fn assume_init(self) -> Gc<T> {
        let ptr = self.ptr.as_ptr() as *mut GcBox<MaybeUninit<T>>;
        unsafe { Gc::from_inner((&mut *ptr).assume_init()) }
        // Now that T is initialized, we must make sure that it's dropped when
        // `GcBox<T>` is freed.
    }
}

impl<T> GcBox<MaybeUninit<T>> {
    unsafe fn assume_init(&mut self) -> NonNull<GcBox<T>> {
        unsafe {
            let init = self as *mut GcBox<MaybeUninit<T>> as *mut GcBox<T>;
            NonNull::new_unchecked(init)
        }
    }
}

#[cfg(not(no_global_oom_handling))]
#[unstable(feature = "gc", issue = "none")]
impl<T: Default + Send + Sync + ReferenceFree> Default for Gc<T> {
    /// Creates a new `Gc<T>`, with the `Default` value for `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::gc::Gc;
    ///
    /// let x: Gc<i32> = Default::default();
    /// assert_eq!(*x, 0);
    /// ```
    #[inline]
    fn default() -> Gc<T> {
        Gc::new(Default::default())
    }
}

impl<T: ?Sized + PartialEq> PartialEq for Gc<T> {
    /// Equality for two `Gc`s.
    ///
    /// Two `Gc`s are equal if their inner values are equal, even if they are
    /// stored in different allocations.
    ///
    /// If `T` also implements `Eq` (implying reflexivity of equality),
    /// two `Gc`s that point to the same allocation are
    /// always equal.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::gc::Gc;
    ///
    /// let five = Gc::new(5);
    ///
    /// assert!(five == Gc::new(5));
    /// ```
    #[inline]
    fn eq(&self, other: &Gc<T>) -> bool {
        **self == **other
    }

    /// Inequality for two `Gc`s.
    ///
    /// Two `Gc`s are unequal if their inner values are unequal.
    ///
    /// If `T` also implements `Eq` (implying reflexivity of equality),
    /// two `Gc`s that point to the same allocation are
    /// never unequal.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::gc::Gc;
    ///
    /// let five = Gc::new(5);
    ///
    /// assert!(five != Gc::new(6));
    /// ```
    #[inline]
    fn ne(&self, other: &Gc<T>) -> bool {
        **self != **other
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Eq> Eq for Gc<T> {}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + PartialOrd> PartialOrd for Gc<T> {
    /// Partial comparison for two `Gc`s.
    ///
    /// The two are compared by calling `partial_cmp()` on their inner values.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::gc::Gc;
    /// use std::cmp::Ordering;
    ///
    /// let five = Gc::new(5);
    ///
    /// assert_eq!(Some(Ordering::Less), five.partial_cmp(&Gc::new(6)));
    /// ```
    #[inline(always)]
    fn partial_cmp(&self, other: &Gc<T>) -> Option<Ordering> {
        (**self).partial_cmp(&**other)
    }

    /// Less-than comparison for two `Gc`s.
    ///
    /// The two are compared by calling `<` on their inner values.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::gc::Gc;
    ///
    /// let five = Gc::new(5);
    ///
    /// assert!(five < Gc::new(6));
    /// ```
    #[inline(always)]
    fn lt(&self, other: &Gc<T>) -> bool {
        **self < **other
    }

    /// 'Less than or equal to' comparison for two `Gc`s.
    ///
    /// The two are compared by calling `<=` on their inner values.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::gc::Gc;
    ///
    /// let five = Gc::new(5);
    ///
    /// assert!(five <= Gc::new(5));
    /// ```
    #[inline(always)]
    fn le(&self, other: &Gc<T>) -> bool {
        **self <= **other
    }

    /// Greater-than comparison for two `Gc`s.
    ///
    /// The two are compared by calling `>` on their inner values.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::gc::Gc;
    ///
    /// let five = Gc::new(5);
    ///
    /// assert!(five > Gc::new(4));
    /// ```
    #[inline(always)]
    fn gt(&self, other: &Gc<T>) -> bool {
        **self > **other
    }

    /// 'Greater than or equal to' comparison for two `Gc`s.
    ///
    /// The two are compared by calling `>=` on their inner values.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::gc::Gc;
    ///
    /// let five = Gc::new(5);
    ///
    /// assert!(five >= Gc::new(5));
    /// ```
    #[inline(always)]
    fn ge(&self, other: &Gc<T>) -> bool {
        **self >= **other
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Ord> Ord for Gc<T> {
    /// Comparison for two `Gc`s.
    ///
    /// The two are compared by calling `cmp()` on their inner values.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(gc)]
    /// use std::gc::Gc;
    /// use std::cmp::Ordering;
    ///
    /// let five = Gc::new(5);
    ///
    /// assert_eq!(Ordering::Less, five.cmp(&Gc::new(6)));
    /// ```
    #[inline]
    fn cmp(&self, other: &Gc<T>) -> Ordering {
        (**self).cmp(&**other)
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + fmt::Display> fmt::Display for Gc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for Gc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized> fmt::Pointer for Gc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&(&**self as *const T), f)
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized> Deref for Gc<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.inner().value
    }
}

#[unstable(feature = "receiver_trait", issue = "none")]
impl<T: ?Sized> Receiver for Gc<T> {}

/// `Copy` and `Clone` are implemented manually because a reference to `Gc<T>`
/// should be copyable regardless of `T`. It differs subtly from `#[derive(Copy,
/// Clone)]` in that the latter only makes `Gc<T>` copyable if `T` is.
#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized> Copy for Gc<T> {}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized> Clone for Gc<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Hash> Hash for Gc<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized> core::borrow::Borrow<T> for Gc<T> {
    fn borrow(&self) -> &T {
        &**self
    }
}

impl<T: ?Sized> AsRef<T> for Gc<T> {
    fn as_ref(&self) -> &T {
        &**self
    }
}
