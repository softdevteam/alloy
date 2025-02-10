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
    mem::{self, align_of_val_raw, size_of_val, MaybeUninit},
    ops::{CoerceUnsized, Deref, DispatchFromDyn, Receiver},
    ptr::{self, drop_in_place, NonNull},
};

#[cfg(not(no_global_oom_handling))]
use crate::alloc::{handle_alloc_error, Global};
#[cfg(not(no_global_oom_handling))]
use core::slice::from_raw_parts_mut;

pub use core::gc::*;

#[cfg(feature = "log-stats")]
use core::sync::atomic;

#[cfg(feature = "log-stats")]
use crate::alloc::GC_COUNTERS;

#[cfg(test)]
mod tests;

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
        #[cfg(feature = "log-stats")]
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

#[cfg(feature = "log-stats")]
#[derive(Debug, Copy, Clone)]
pub struct GcStats {
    pub elision_enabled: u8,
    pub prem_enabled: u8,
    pub premopt_enabled: u8,
    pub num_finalizers_registered: u64,
    pub num_finalizers_completed: u64,
    pub num_allocated_gc: u64,
    pub num_allocated_boxed: u64,
    pub num_allocated_arc: u64,
    pub num_allocated_rc: u64,
    pub num_cycles: u64,
}

////////////////////////////////////////////////////////////////////////////////
// Free functions
////////////////////////////////////////////////////////////////////////////////
#[cfg(feature = "log-stats")]
pub fn stats() -> GcStats {
    #[cfg(feature = "finalizer-elision")]
    let elision_enabled = 1;
    #[cfg(not(feature = "finalizer-elision"))]
    let elision_enabled = 0;
    #[cfg(feature = "premature-finalizer-prevention")]
    let prem_enabled = 1;
    #[cfg(not(feature = "premature-finalizer-prevention"))]
    let prem_enabled = 0;
    #[cfg(feature = "premature-finalizer-prevention-optimize")]
    let premopt_enabled = 1;
    #[cfg(not(feature = "premature-finalizer-prevention-optimize"))]
    let premopt_enabled = 0;

    GcStats {
        elision_enabled,
        prem_enabled,
        premopt_enabled,
        num_finalizers_registered: GC_COUNTERS
            .finalizers_registered
            .load(atomic::Ordering::Relaxed),
        num_finalizers_completed: unsafe { bdwgc::GC_finalized_total() },
        num_allocated_gc: GC_COUNTERS.allocated_gc.load(atomic::Ordering::Relaxed),
        num_allocated_boxed: GC_COUNTERS.allocated_boxed.load(atomic::Ordering::Relaxed),
        num_allocated_rc: GC_COUNTERS.allocated_rc.load(atomic::Ordering::Relaxed),
        num_allocated_arc: GC_COUNTERS.allocated_arc.load(atomic::Ordering::Relaxed),
        num_cycles: unsafe { bdwgc::GC_get_gc_no() },
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

/// Calculate layout for `GcBox<T>` using the inner value's layout
fn gcbox_layout_for_value_layout(layout: Layout) -> Layout {
    // Calculate layout using the given value layout.
    Layout::new::<GcBox<()>>().extend(layout).unwrap().0.pad_to_align()
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
impl<T: ?Sized> !core::marker::FinalizerSafe for Gc<T> {}

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
#[cfg(all(not(bootstrap), not(test), feature = "premature-finalizer-prevention"))]
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

    /// Allocates a `GcBox<T>` with sufficient space for a possibly-unsized inner value where the
    /// value has the layout provided.
    ///
    /// The function `mem_to_gcbox` is called with the data pointer and must return back a
    /// (potentially fat)-pointer for the `GcBox<T>`.
    #[cfg(not(no_global_oom_handling))]
    unsafe fn allocate_for_layout(
        value_layout: Layout,
        allocate: impl FnOnce(Layout) -> Result<NonNull<[u8]>, AllocError>,
        mem_to_gcbox: impl FnOnce(*mut u8) -> *mut GcBox<T>,
    ) -> *mut GcBox<T> {
        let layout = gcbox_layout_for_value_layout(value_layout);
        unsafe {
            Gc::try_allocate_for_layout(value_layout, allocate, mem_to_gcbox)
                .unwrap_or_else(|_| handle_alloc_error(layout))
        }
    }

    /// Allocates an `GcBox<T>` with sufficient space for a possibly-unsized inner value where the
    /// value has the layout provided, returning an error if allocation fails.
    ///
    /// The function `mem_to_gcbox` is called with the data pointer and must return back a
    /// (potentially fat)-pointer for the `GcBox<T>`.
    #[inline]
    unsafe fn try_allocate_for_layout(
        value_layout: Layout,
        allocate: impl FnOnce(Layout) -> Result<NonNull<[u8]>, AllocError>,
        mem_to_gcbox: impl FnOnce(*mut u8) -> *mut GcBox<T>,
    ) -> Result<*mut GcBox<T>, AllocError> {
        let layout = gcbox_layout_for_value_layout(value_layout);

        // Allocate for the layout.
        let ptr = allocate(layout)?;

        // Initialize the GcBox
        let inner = mem_to_gcbox(ptr.as_non_null_ptr().as_ptr());
        unsafe {
            debug_assert_eq!(Layout::for_value_raw(inner), layout);
        }
        Ok(inner)
    }

    #[cfg(not(no_global_oom_handling))]
    unsafe fn allocate_for_ptr(ptr: *const T) -> *mut GcBox<T> {
        // Allocate for the `GcBox<T>` using the given value.
        unsafe {
            Gc::<T>::allocate_for_layout(
                Layout::for_value_raw(ptr),
                |layout| GcAllocator.allocate(layout),
                |mem| mem.with_metadata_of(ptr as *const GcBox<T>),
            )
        }
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

    #[cfg(not(no_global_oom_handling))]
    fn from_box<A: Allocator>(src: Box<T, A>) -> Gc<T> {
        unsafe {
            let value_size = size_of_val(&*src);
            let ptr = Self::allocate_for_ptr(&*src);

            // Copy value as bytes
            ptr::copy_nonoverlapping(
                core::ptr::addr_of!(*src) as *const u8,
                ptr::addr_of_mut!((*ptr).value) as *mut u8,
                value_size,
            );

            // Free the allocation without dropping its contents
            let (bptr, alloc) = Box::into_raw_with_allocator(src);
            let src = Box::from_raw_in(bptr as *mut mem::ManuallyDrop<T>, alloc.by_ref());
            drop(src);

            Self::from_ptr(ptr)
        }
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
    #[cfg_attr(not(bootstrap), rustc_fsa_entry_point)]
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
        {
            #[cfg(feature = "finalizer-elision")]
            let needs_finalizer = crate::mem::needs_finalizer::<T>();
            #[cfg(not(feature = "finalizer-elision"))]
            let needs_finalizer = crate::mem::needs_drop::<T>();

            if !needs_finalizer {
                return Self::from_inner(
                    Box::leak(Box::new_in(GcBox { value }, GcAllocator)).into(),
                );
            }

            #[cfg(feature = "log-stats")]
            {
                GC_COUNTERS.finalizers_registered.fetch_add(1, atomic::Ordering::Relaxed);
            }
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
        #[cfg(feature = "log-stats")]
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
impl<T: Default + Send + Sync> Default for Gc<T> {
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

#[cfg(not(no_global_oom_handling))]
impl<T> From<T> for Gc<T> {
    /// Converts a `T` into an `Gc<T>`
    ///
    /// The conversion moves the value into a newly allocated `Gc`. It is equivalent to calling
    /// `Gc::new(t)`.
    ///
    /// # Example
    /// ```rust
    /// # #![feature(gc)]
    /// # use std::gc::Gc;
    /// let x = 5;
    /// let arc = Gc::new(5);
    ///
    /// assert_eq!(Gc::from(x), arc);
    /// ```
    #[cfg_attr(not(bootstrap), rustc_fsa_entry_point)]
    fn from(t: T) -> Self {
        unsafe { Gc::new_internal(t) }
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T: ?Sized> From<Box<T>> for Gc<T> {
    /// Move a boxed object to a new, garbage-collected, allocation.
    ///
    /// # Example
    ///
    /// ```
    /// # #![feature(gc)]
    /// # use std::gc::Gc;
    /// let original: Box<i32> = Box::new(1);
    /// let shared: Gc<i32> = Gc::from(original);
    /// assert_eq!(1, *shared);
    /// ```
    #[inline]
    #[cfg_attr(not(bootstrap), rustc_fsa_entry_point)]
    fn from(v: Box<T>) -> Gc<T> {
        Gc::from_box(v)
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T, const N: usize> From<[T; N]> for Gc<[T]> {
    /// Converts a [`[T; N]`](prim@array) into an `Gc<[T]>`.
    ///
    /// The conversion moves the array into a newly allocated `Gc`.
    ///
    /// # Example
    ///
    /// ```
    /// # #![feature(gc)]
    /// # use std::gc::Gc;
    /// let original: [i32; 3] = [1, 2, 3];
    /// let shared: Gc<[i32]> = Gc::from(original);
    /// assert_eq!(&[1, 2, 3], &shared[..]);
    /// ```
    #[inline]
    fn from(v: [T; N]) -> Gc<[T]> {
        Gc::<[T; N]>::from(v)
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T: Clone> From<&[T]> for Gc<[T]> {
    /// Allocate a garbage-collected slice and fill it by cloning `v`'s items.
    ///
    /// # Example
    ///
    /// ```
    /// # #![feature(gc)]
    /// # use std::gc::Gc;
    /// let original: &[i32] = &[1, 2, 3];
    /// let shared: Gc<[i32]> = Gc::from(original);
    /// assert_eq!(&[1, 2, 3], &shared[..]);
    /// ```
    #[inline]
    #[cfg_attr(not(bootstrap), rustc_fsa_entry_point)]
    fn from(v: &[T]) -> Gc<[T]> {
        <Self as GcFromSlice<T>>::from_slice(v)
    }
}

#[cfg(not(no_global_oom_handling))]
impl From<&str> for Gc<str> {
    /// Allocate a garbage-collected `str` and copy `v` into it.
    ///
    /// # Example
    ///
    /// ```
    /// # #![feature(gc)]
    /// # use std::gc::Gc;
    /// let shared: Gc<str> = Gc::from("eggplant");
    /// assert_eq!("eggplant", &shared[..]);
    /// ```
    #[inline]
    fn from(v: &str) -> Gc<str> {
        let arc = Gc::<[u8]>::from(v.as_bytes());
        Gc::from_raw(Gc::into_raw(arc) as *const str)
    }
}

#[cfg(not(no_global_oom_handling))]
impl From<String> for Gc<str> {
    /// Allocate a garbage-collected `str` and copy `v` into it.
    ///
    /// # Example
    ///
    /// ```
    /// # #![feature(gc)]
    /// # use std::gc::Gc;
    /// let unique: String = "eggplant".to_owned();
    /// let shared: Gc<str> = Gc::from(unique);
    /// assert_eq!("eggplant", &shared[..]);
    /// ```
    #[inline]
    fn from(v: String) -> Gc<str> {
        Gc::from(&v[..])
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T> From<Vec<T>> for Gc<[T]> {
    /// Allocate a garbage-collected slice and move `v`'s items into it.
    ///
    /// # Example
    ///
    /// ```
    /// # #![feature(gc)]
    /// # use std::gc::Gc;
    /// let unique: Vec<i32> = vec![1, 2, 3];
    /// let shared: Gc<[i32]> = Gc::from(unique);
    /// assert_eq!(&[1, 2, 3], &shared[..]);
    /// ```
    #[inline]
    #[cfg_attr(not(bootstrap), rustc_fsa_entry_point)]
    fn from(v: Vec<T>) -> Gc<[T]> {
        unsafe {
            let (vec_ptr, len, cap) = v.into_raw_parts();

            let gc_ptr = Self::allocate_for_slice(len);
            ptr::copy_nonoverlapping(vec_ptr, ptr::addr_of_mut!((*gc_ptr).value) as *mut T, len);

            // Create a `Vec<T, &A>` with length 0, to deallocate the buffer
            // without dropping its contents or the allocator
            let _ = Vec::from_raw_parts(vec_ptr, 0, cap);

            Self::from_ptr(gc_ptr)
        }
    }
}

/// Specialization trait used for `From<&[T]>`.
#[cfg(not(no_global_oom_handling))]
trait GcFromSlice<T> {
    fn from_slice(slice: &[T]) -> Self;
}

#[cfg(not(no_global_oom_handling))]
impl<T: Clone> GcFromSlice<T> for Gc<[T]> {
    #[inline]
    default fn from_slice(v: &[T]) -> Self {
        unsafe { Self::from_iter_exact(v.iter().cloned(), v.len()) }
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T: Copy> GcFromSlice<T> for Gc<[T]> {
    #[inline]
    fn from_slice(v: &[T]) -> Self {
        unsafe { Gc::copy_from_slice(v) }
    }
}

impl<T> Gc<[T]> {
    /// Allocates an `GcBox<[T]>` with the given length.
    #[cfg(not(no_global_oom_handling))]
    unsafe fn allocate_for_slice(len: usize) -> *mut GcBox<[T]> {
        unsafe {
            Self::allocate_for_layout(
                Layout::array::<T>(len).unwrap(),
                |layout| Global.allocate(layout),
                |mem| ptr::slice_from_raw_parts_mut(mem.cast::<T>(), len) as *mut GcBox<[T]>,
            )
        }
    }

    /// Copy elements from slice into newly allocated `Gc<[T]>`
    ///
    /// Unsafe because the caller must either take ownership or bind `T: Copy`.
    #[cfg(not(no_global_oom_handling))]
    unsafe fn copy_from_slice(v: &[T]) -> Gc<[T]> {
        unsafe {
            let ptr = Self::allocate_for_slice(v.len());

            ptr::copy_nonoverlapping(
                v.as_ptr(),
                ptr::addr_of_mut!((*ptr).value) as *mut T,
                v.len(),
            );

            Self::from_ptr(ptr)
        }
    }

    /// Constructs an `Gc<[T]>` from an iterator known to be of a certain size.
    ///
    /// Behavior is undefined should the size be wrong.
    #[cfg(not(no_global_oom_handling))]
    unsafe fn from_iter_exact(iter: impl Iterator<Item = T>, len: usize) -> Gc<[T]> {
        // Panic guard while cloning T elements.
        // In the event of a panic, elements that have been written
        // into the new GcBox will be dropped, then the memory freed.
        struct Guard<T> {
            mem: NonNull<u8>,
            elems: *mut T,
            layout: Layout,
            n_elems: usize,
        }

        impl<T> Drop for Guard<T> {
            fn drop(&mut self) {
                unsafe {
                    let slice = from_raw_parts_mut(self.elems, self.n_elems);
                    ptr::drop_in_place(slice);

                    Global.deallocate(self.mem, self.layout);
                }
            }
        }

        unsafe {
            let ptr = Self::allocate_for_slice(len);

            let mem = ptr as *mut _ as *mut u8;
            let layout = Layout::for_value_raw(ptr);

            // Pointer to first element
            let elems = ptr::addr_of_mut!((*ptr).value) as *mut T;

            let mut guard = Guard { mem: NonNull::new_unchecked(mem), elems, layout, n_elems: 0 };

            for (i, item) in iter.enumerate() {
                ptr::write(elems.add(i), item);
                guard.n_elems += 1;
            }

            // All clear. Forget the guard so it doesn't free the new GcBox.
            mem::forget(guard);

            Self::from_ptr(ptr)
        }
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
