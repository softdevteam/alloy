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
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unused_macros)]

// Re-export some of our utilities which are expected by other crates.
pub use crate::panicking::{begin_panic, panic_count};
pub use core::panicking::{panic_display, panic_fmt};

use crate::sync::Once;
use crate::sys;
use crate::thread::{self, Thread};

// Prints to the "panic output", depending on the platform this may be:
// - the standard error output
// - some dedicated platform specific output
// - nothing (so this macro is a no-op)
macro_rules! rtprintpanic {
    ($($t:tt)*) => {
        if let Some(mut out) = crate::sys::stdio::panic_output() {
            let _ = crate::io::Write::write_fmt(&mut out, format_args!($($t)*));
        }
    }
}

macro_rules! rtabort {
    ($($t:tt)*) => {
        {
            rtprintpanic!("fatal runtime error: {}\n", format_args!($($t)*));
            crate::sys::abort_internal();
        }
    }
}

macro_rules! rtassert {
    ($e:expr) => {
        if !$e {
            rtabort!(concat!("assertion failed: ", stringify!($e)));
        }
    };
}

macro_rules! rtunwrap {
    ($ok:ident, $e:expr) => {
        match $e {
            $ok(v) => v,
            ref err => {
                let err = err.as_ref().map(drop); // map Ok/Some which might not be Debug
                rtabort!(concat!("unwrap failed: ", stringify!($e), " = {:?}"), err)
            }
        }
    };
}

// One-time runtime initialization.
// Runs before `main`.
// SAFETY: must be called only once during runtime initialization.
// NOTE: this is not guaranteed to run, for example when Rust code is called externally.
//
// # The `sigpipe` parameter
//
// Since 2014, the Rust runtime on Unix has set the `SIGPIPE` handler to
// `SIG_IGN`. Applications have good reasons to want a different behavior
// though, so there is a `#[unix_sigpipe = "..."]` attribute on `fn main()` that
// can be used to select how `SIGPIPE` shall be setup (if changed at all) before
// `fn main()` is called. See <https://github.com/rust-lang/rust/issues/97889>
// for more info.
//
// The `sigpipe` parameter to this function gets its value via the code that
// rustc generates to invoke `fn lang_start()`. The reason we have `sigpipe` for
// all platforms and not only Unix, is because std is not allowed to have `cfg`
// directives as this high level. See the module docs in
// `src/tools/tidy/src/pal.rs` for more info. On all other platforms, `sigpipe`
// has a value, but its value is ignored.
//
// Even though it is an `u8`, it only ever has 4 values. These are documented in
// `compiler/rustc_session/src/config/sigpipe.rs`.
#[cfg_attr(test, allow(dead_code))]
unsafe fn init(argc: isize, argv: *const *const u8, sigpipe: u8) {
    unsafe {
        // Internally, this registers a SIGSEGV handler to compute the start and
        // end bounds of the data segment. This means it *MUST* be called before
        // rustc registers its own SIGSEGV stack overflow handler.
        //
        // Rust's stack overflow handler will unregister and return if there is
        // no stack overflow, allowing the fault to "fall-through" to Boehm's
        // handler next time. The is not true in the reverse case.
        crate::gc::init();

        sys::init(argc, argv, sigpipe);

        // Set up the current thread to give it the right name.
        let thread = Thread::new_main();
        thread::set_current(thread);
    }
}

#[cfg(feature = "log-stats")]
pub(crate) fn log_stats() {
    if crate::env::var("ALLOY_LOG").is_err() {
        return;
    }

    use crate::io::Write;
    let mut filename = crate::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(crate::env::var("ALLOY_LOG").unwrap())
        .unwrap();

    let headers = "elision enabled,\
        premature finalizer prevention enabled,\
        premopt enabled,\
        finalizers registered,\
        finalizers completed,\
        barriers visited,\
        Gc allocated,\
        Box allocated,\
        Rc allocated,\
        Arc allocated,\
        STW pauses";
    let stats = crate::gc::stats();
    let stats = format!(
        "{},{},{},{},{},{},{},{},{},{},{},{}\n",
        stats.elision_enabled,
        stats.prem_enabled,
        stats.premopt_enabled,
        stats.num_finalizers_registered,
        stats.num_finalizers_completed,
        stats.num_finalizers_elidable,
        stats.num_barriers_visited,
        stats.num_allocated_gc,
        stats.num_allocated_boxed,
        stats.num_allocated_rc,
        stats.num_allocated_arc,
        stats.num_cycles
    );
    write!(filename, "{}", format!("{headers}\n{stats}")).unwrap();
}

// One-time runtime cleanup.
// Runs after `main` or at program exit.
// NOTE: this is not guaranteed to run, for example when the program aborts.
pub(crate) fn cleanup() {
    static CLEANUP: Once = Once::new();
    CLEANUP.call_once(|| unsafe {
        // Flush stdout and disable buffering.
        crate::io::cleanup();
        // SAFETY: Only called once during runtime cleanup.
        sys::cleanup();
    });
}

// To reduce the generated code of the new `lang_start`, this function is doing
// the real work.
#[cfg(not(test))]
fn lang_start_internal(
    main: &(dyn Fn() -> i32 + Sync + crate::panic::RefUnwindSafe),
    argc: isize,
    argv: *const *const u8,
    sigpipe: u8,
) -> Result<isize, !> {
    use crate::{mem, panic};
    let rt_abort = move |e| {
        mem::forget(e);
        rtabort!("initialization or cleanup bug");
    };
    // Guard against the code called by this function from unwinding outside of the Rust-controlled
    // code, which is UB. This is a requirement imposed by a combination of how the
    // `#[lang="start"]` attribute is implemented as well as by the implementation of the panicking
    // mechanism itself.
    //
    // There are a couple of instances where unwinding can begin. First is inside of the
    // `rt::init`, `rt::cleanup` and similar functions controlled by bstd. In those instances a
    // panic is a std implementation bug. A quite likely one too, as there isn't any way to
    // prevent std from accidentally introducing a panic to these functions. Another is from
    // user code from `main` or, more nefariously, as described in e.g. issue #86030.
    // SAFETY: Only called once during runtime initialization.
    panic::catch_unwind(move || unsafe { init(argc, argv, sigpipe) }).map_err(rt_abort)?;
    let ret_code = panic::catch_unwind(move || panic::catch_unwind(main).unwrap_or(101) as isize)
        .map_err(move |e| {
            mem::forget(e);
            rtabort!("drop of the panic payload panicked");
        });
    panic::catch_unwind(cleanup).map_err(rt_abort)?;
    ret_code
}

#[cfg(not(any(test, doctest)))]
#[lang = "start"]
fn lang_start<T: crate::process::Termination + 'static>(
    main: fn() -> T,
    argc: isize,
    argv: *const *const u8,
    sigpipe: u8,
) -> isize {
    let Ok(v) = lang_start_internal(
        &move || crate::sys_common::backtrace::__rust_begin_short_backtrace(main).report().to_i32(),
        argc,
        argv,
        sigpipe,
    );
    #[cfg(feature = "log-stats")]
    crate::rt::log_stats();
    v
}
