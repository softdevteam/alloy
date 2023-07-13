#![allow(missing_docs)]

#[cfg(not(test))]
use crate::boxed::Box;
#[cfg(test)]
use std::boxed::Box;

use core::{
    any::Any,
    fmt,
    hash::{Hash, Hasher},
    marker::{PhantomData, Unsize},
    mem::{ManuallyDrop, MaybeUninit},
    ops::{CoerceUnsized, Deref, DispatchFromDyn},
    ptr::{null_mut, NonNull},
};

use boehm::GcAllocator;

#[cfg(test)]
mod tests;

#[unstable(feature = "gc", issue = "none")]
static ALLOCATOR: GcAllocator = GcAllocator;

struct GcBox<T: ?Sized>(ManuallyDrop<T>);

/// A multi-threaded garbage collected pointer.
///
/// The type `Gc<T>` provides shared ownership of a value of type `T`,
/// allocted in the heap. `Gc` pointers are `Copyable`, so new pointers to
/// the same value in the heap can be produced trivially. The lifetime of
/// `T` is tracked automatically: it is freed when the application
/// determines that no references to `T` are in scope. This does not happen
/// deterministically, and no guarantees are given about when a value
/// managed by `Gc` is freed.
///
/// Shared references in Rust disallow mutation by default, and `Gc` is no
/// exception: you cannot generally obtain a mutable reference to something
/// inside an `Gc`. If you need mutability, put a `Cell` or `RefCell` inside
/// the `Gc`.
///
/// Unlike `Rc<T>`, cycles between `Gc` pointers are allowed and can be
/// deallocated without issue.
///
/// `Gc<T>` automatically dereferences to `T` (via the `Deref` trait), so
/// you can call `T`'s methods on a value of type `Gc<T>`.
///
/// `Gc<T>` will implement `Sync` as long as `T` implements `Sync`. `Gc<T>`
/// will always implement `Send` because it requires `T` to implement `Send`.
/// This is because if `T` has a finalizer, it will be run on a seperate thread.
#[unstable(feature = "gc", issue = "none")]
#[cfg_attr(all(not(bootstrap), not(test)), lang = "gc")]
#[derive(PartialEq, Eq)]
pub struct Gc<T: ?Sized + Send> {
    ptr: NonNull<GcBox<T>>,
    _phantom: PhantomData<T>,
}

unsafe impl<T: Send> Send for Gc<T> {}
unsafe impl<T: Sync + Send> Sync for Gc<T> {}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Unsize<U> + Send, U: ?Sized + Send> CoerceUnsized<Gc<U>> for Gc<T> {}
#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Unsize<U> + Send, U: ?Sized + Send> DispatchFromDyn<Gc<U>> for Gc<T> {}

impl<T: ?Sized + Send> Gc<T> {
    unsafe fn from_inner(ptr: NonNull<GcBox<T>>) -> Self {
        Self { ptr, _phantom: PhantomData }
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
}

impl Gc<dyn Any + Send> {
    #[unstable(feature = "gc", issue = "none")]
    pub fn downcast<T: Any + Send>(self) -> Result<Gc<T>, Gc<dyn Any + Send>> {
        if (*self).is::<T>() {
            unsafe {
                let ptr = self.ptr.cast::<GcBox<T>>();
                Ok(Gc::from_inner(ptr))
            }
        } else {
            Err(self)
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

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Send> Deref for Gc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.ptr.as_ptr() as *const T) }
    }
}

/// `Copy` and `Clone` are implemented manually because a reference to `Gc<T>`
/// should be copyable regardless of `T`. It differs subtly from `#[derive(Copy,
/// Clone)]` in that the latter only makes `Gc<T>` copyable if `T` is.
#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Send> Copy for Gc<T> {}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Send> Clone for Gc<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[unstable(feature = "gc", issue = "none")]
impl<T: ?Sized + Hash + Send> Hash for Gc<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}
