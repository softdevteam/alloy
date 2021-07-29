#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unused_macros)]

use crate::sync::Once;
use crate::sys;
use crate::sys_common::thread_info;
use crate::thread::Thread;

// One-time runtime initialization.
// Runs before `main`.
// SAFETY: must be called only once during runtime initialization.
// NOTE: this is not guaranteed to run, for example when Rust code is called externally.
#[cfg_attr(test, allow(dead_code))]
pub unsafe fn init(argc: isize, argv: *const *const u8) {
    unsafe {
        use crate::alloc::GcAllocator;

        // Internally, this registers a SIGSEGV handler to compute the start and
        // end bounds of the data segment. This means it *MUST* be called before
        // rustc registers its own SIGSEGV stack overflow handler.
        //
        // Rust's stack overflow handler will unregister and return if there is
        // no stack overflow, allowing the fault to "fall-through" to Boehm's
        // handler next time. The is not true in the reverse case.
        GcAllocator::init();

        sys::init(argc, argv);

        let main_guard = sys::thread::guard::init();
        // Next, set up the current Thread with the guard information we just
        // created. Note that this isn't necessary in general for new threads,
        // but we just do this to name the main thread and to give it correct
        // info about the stack bounds.
        let thread = Thread::new(Some("main".to_owned()));
        thread_info::set(main_guard, thread);
    }
}

// One-time runtime cleanup.
// Runs after `main` or at program exit.
// NOTE: this is not guaranteed to run, for example when the program aborts.
#[cfg_attr(test, allow(dead_code))]
pub fn cleanup() {
    static CLEANUP: Once = Once::new();
    CLEANUP.call_once(|| unsafe {
        // Flush stdout and disable buffering.
        crate::io::cleanup();
        // SAFETY: Only called once during runtime cleanup.
        sys::cleanup();
    });
}

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
