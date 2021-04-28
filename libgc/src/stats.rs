use std::sync::atomic::{AtomicUsize, Ordering};

pub static NUM_REGISTERED_FINALIZERS: AtomicUsize = AtomicUsize::new(0);
