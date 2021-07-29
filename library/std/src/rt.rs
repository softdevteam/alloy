//! Runtime services
//!
//! The `rt` module provides a narrow set of runtime services,
//! including the global heap (exported in `heap`) and unwinding and
//! backtrace support. The APIs in this module are highly unstable,
//! and should be considered as private implementation details for the
//! time being.

#![unstable(
    feature = "rt",
    reason = "this public module should not exist and is highly likely \
              to disappear",
    issue = "none"
)]
#![doc(hidden)]

// Re-export some of our utilities which are expected by other crates.
pub use crate::panicking::{begin_panic, begin_panic_fmt, panic_count};

// To reduce the generated code of the new `lang_start`, this function is doing
// the real work.
#[cfg(not(test))]
fn lang_start_internal(
    main: &(dyn Fn() -> i32 + Sync + crate::panic::RefUnwindSafe),
    argc: isize,
    argv: *const *const u8,
) -> isize {
    use crate::alloc::GcAllocator;
    use crate::panic;
    use crate::sys_common;

    // SAFETY: Only called once during runtime initialization.
    unsafe { sys_common::rt::init(argc, argv) };

    unsafe {
        // Internally, this registers a SIGSEGV handler to compute the start and
        // end bounds of the data segment. This means it *MUST* be called before
        // rustc registers its own SIGSEGV stack overflow handler.
        //
        // Rust's stack overflow handler will unregister and return if there is
        // no stack overflow, allowing the fault to "fall-through" to Boehm's
        // handler next time. The is not true in the reverse case.
        GcAllocator::init();
        let main_guard = sys::thread::guard::init();
        sys::stack_overflow::init();
    }
    let exit_code = panic::catch_unwind(main);

    sys_common::rt::cleanup();

    exit_code.unwrap_or(101) as isize
}

#[cfg(not(test))]
#[lang = "start"]
fn lang_start<T: crate::process::Termination + 'static>(
    main: fn() -> T,
    argc: isize,
    argv: *const *const u8,
) -> isize {
    lang_start_internal(
        &move || crate::sys_common::backtrace::__rust_begin_short_backtrace(main).report(),
        argc,
        argv,
    )
}
