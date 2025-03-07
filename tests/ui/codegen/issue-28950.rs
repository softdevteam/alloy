//@ run-pass
//@ needs-threads
//@ compile-flags: -O

// Tests that the `vec!` macro does not overflow the stack when it is
// given data larger than the stack.

// FIXME(eddyb) Improve unoptimized codegen to avoid the temporary,
// and thus run successfully even when compiled at -C opt-level=0.

// For Alloy, this must be larger than 1 << 16 because very small threads fail when
// doing stack scanning work for GC.
const LEN: usize = 1 << 17;

use std::thread::Builder;

fn main() {
    assert!(Builder::new().stack_size(LEN / 2).spawn(|| {
        // FIXME(eddyb) this can be vec![[0: LEN]] pending
        // https://llvm.org/bugs/show_bug.cgi?id=28987
        let vec = vec![unsafe { std::mem::zeroed::<[u8; LEN]>() }];
        assert_eq!(vec.len(), 1);
    }).unwrap().join().is_ok());
}
