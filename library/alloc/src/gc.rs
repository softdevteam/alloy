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
//! [`Rc`]: core::rc::Rc
//! [`Deref`]: core::ops::Deref
//! [mutability]: core::cell#introducing-mutability-inside-of-something-immutable
//! [fully qualified syntax]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#fully-qualified-syntax-for-disambiguation-calling-methods-with-the-same-name
#![allow(missing_docs)]
use crate::alloc::{Allocator, Layout};

#[cfg(not(test))]
#[cfg(not(no_global_oom_handling))]
use crate::boxed::Box;
#[cfg(test)]
#[cfg(not(no_global_oom_handling))]
use std::boxed::Box;

use core::{
    any::Any,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    marker::{PhantomData, Unsize},
    mem::{ManuallyDrop, MaybeUninit},
    ops::{CoerceUnsized, Deref, DispatchFromDyn},
    ptr::{null_mut, NonNull},
};

use core::gc::{NoFinalize, NoTrace};

use boehm::GcAllocator;

#[cfg(test)]
mod tests;

#[unstable(feature = "gc", issue = "none")]
static ALLOCATOR: GcAllocator = GcAllocator;

struct GcBox<T: ?Sized>(ManuallyDrop<T>);

/// A multi-threaded garbage collected pointer.
///
/// See the [module-level documentation](./index.html) for more details.
#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(all(not(bootstrap), not(test)), lang = "gc")]
pub struct Gc<T: ?Sized> {
    ptr: NonNull<GcBox<T>>,
    _phantom: PhantomData<T>,
}

unsafe impl<T: Send> Send for Gc<T> {}
unsafe impl<T: Sync + Send> Sync for Gc<T> {}

impl<T: ?Sized> !NoTrace for Gc<T> {}

unsafe impl<T: ?Sized> NoFinalize for Gc<T> {}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Gc<U>> for Gc<T> {}
#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Unsize<U>, U: ?Sized> DispatchFromDyn<Gc<U>> for Gc<T> {}

impl<T: ?Sized> Gc<T> {
    unsafe fn from_inner(ptr: NonNull<GcBox<T>>) -> Self {
        Self { ptr, _phantom: PhantomData }
    }

    /// Get a `Gc<T>` from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `raw` was allocated with `Gc::new()` or
    /// u8 `Gc::new_from_layout()`.
    ///
    /// It is legal for `raw` to be an interior pointer if `T` is valid for the
    /// size and alignment of the originally allocated block.
    #[unstable(feature = "gc", issue = "none")]
    pub fn from_raw(raw: *const T) -> Gc<T> {
        Gc { ptr: unsafe { NonNull::new_unchecked(raw as *mut GcBox<T>) }, _phantom: PhantomData }
    }

    /// Get a raw pointer to the underlying value `T`.
    #[unstable(feature = "gc", issue = "none")]
    pub fn into_raw(this: Self) -> *const T {
        this.ptr.as_ptr() as *const T
    }

    #[unstable(feature = "gc", issue = "none")]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.ptr.as_ptr() == other.ptr.as_ptr()
    }
}

impl<T: Send> Gc<T> {
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
    pub fn new(value: T) -> Self {
        let mut gc = unsafe {
            Self::from_inner(
                Box::leak(Box::new_in(GcBox(ManuallyDrop::new(value)), GcAllocator)).into(),
            )
        };
        gc.register_finalizer();
        gc
    }

    fn register_finalizer(&mut self) {
        #[cfg(feature = "gc_stats")]
        crate::stats::NUM_REGISTERED_FINALIZERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        #[cfg(not(bootstrap))]
        if !core::mem::needs_finalizer::<T>() {
            return;
        }

        unsafe extern "C" fn fshim<T>(obj: *mut u8, _meta: *mut u8) {
            unsafe { ManuallyDrop::drop(&mut *(obj as *mut ManuallyDrop<T>)) };
        }

        unsafe {
            ALLOCATOR.register_finalizer(
                self as *mut _ as *mut u8,
                Some(fshim::<T>),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        }
    }

    #[unstable(feature = "gc", issue = "none")]
    pub fn unregister_finalizer(&mut self) {
        let ptr = self.ptr.as_ptr() as *mut GcBox<T> as *mut u8;
        ALLOCATOR.unregister_finalizer(ptr);
    }

    /// Constructs a new `Gc<MaybeUninit<T>>` which is capable of storing data
    /// up-to the size permissible by `layout`.
    ///
    /// This can be useful if you want to store a value with a custom layout,
    /// but have the collector treat the value as if it were T.
    ///
    /// # Panics
    ///
    /// If `layout` is smaller than that required by `T` and/or has an alignment
    /// which is smaller than that required by `T`.
    #[unstable(feature = "gc", issue = "none")]
    pub fn new_from_layout(layout: Layout) -> Gc<MaybeUninit<T>> {
        let tl = Layout::new::<T>();
        if layout.size() < tl.size() || layout.align() < tl.align() {
            panic!(
                "Requested layout {:?} is either smaller than size {} and/or not aligned to {}",
                layout,
                tl.size(),
                tl.align()
            );
        }
        unsafe { Gc::new_from_layout_unchecked(layout) }
    }

    /// Constructs a new `Gc<MaybeUninit<T>>` which is capable of storing data
    /// up-to the size permissible by `layout`.
    ///
    /// This can be useful if you want to store a value with a custom layout,
    /// but have the collector treat the value as if it were T.
    ///
    /// # Safety
    ///
    /// The caller is responsible for ensuring that both `layout`'s size and
    /// alignment must match or exceed that required to store `T`.
    #[unstable(feature = "gc", issue = "none")]
    pub unsafe fn new_from_layout_unchecked(layout: Layout) -> Gc<MaybeUninit<T>> {
        unsafe { Gc::from_inner(GcBox::new_from_layout(layout)) }
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
        unsafe {
            Self::from_inner(
                Box::leak(Box::new_in(GcBox(ManuallyDrop::new(value)), GcAllocator)).into(),
            )
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

impl<T: Send> Gc<MaybeUninit<T>> {
    /// As with `MaybeUninit::assume_init`, it is up to the caller to guarantee
    /// that the inner value really is in an initialized state. Calling this
    /// when the content is not yet fully initialized causes immediate undefined
    /// behaviour.
    #[unstable(feature = "gc", issue = "none")]
    pub unsafe fn assume_init(self) -> Gc<T> {
        let ptr = self.ptr.as_ptr() as *mut GcBox<MaybeUninit<T>>;
        let mut gc = unsafe { Gc::from_inner((&mut *ptr).assume_init()) };
        // Now that T is initialized, we must make sure that it's dropped when
        // `GcBox<T>` is freed.
        gc.register_finalizer();
        gc
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
impl<T: Default + Send> Default for Gc<T> {
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
impl<T: ?Sized + fmt::Display + Send> fmt::Display for Gc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + fmt::Debug + Send> fmt::Debug for Gc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Send> fmt::Pointer for Gc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&(&**self as *const T), f)
    }
}

impl<T> GcBox<T> {
    fn new_from_layout(layout: Layout) -> NonNull<GcBox<MaybeUninit<T>>> {
        unsafe {
            let base_ptr = ALLOCATOR.allocate(layout).unwrap().as_ptr() as *mut usize;
            NonNull::new_unchecked(base_ptr as *mut GcBox<MaybeUninit<T>>)
        }
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized> Deref for Gc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.ptr.as_ptr() as *const T) }
    }
}

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
