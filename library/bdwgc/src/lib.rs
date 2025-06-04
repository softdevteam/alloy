#![no_std]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[repr(C)]
#[derive(Default)]
pub struct ProfileStats {
    /// Heap size in bytes (including area unmapped to OS).
    pub heapsize_full: usize,
    /// Total bytes contained in free and unmapped blocks.
    pub free_bytes_full: usize,
    /// Amount of memory unmapped to OS.
    pub unmapped_bytes: usize,
    /// Number of bytes allocated since the recent collection.
    pub bytes_allocd_since_gc: usize,
    /// Number of bytes allocated before the recent collection.
    /// The value may wrap.
    pub allocd_bytes_before_gc: usize,
    /// Number of bytes not considered candidates for garbage collection.
    pub non_gc_bytes: usize,
    /// Garbage collection cycle number.
    /// The value may wrap.
    pub gc_no: usize,
    /// Number of marker threads (excluding the initiating one).
    pub markers_m1: usize,
    /// Approximate number of reclaimed bytes after recent collection.
    pub bytes_reclaimed_since_gc: usize,
    /// Approximate number of bytes reclaimed before the recent collection.
    /// The value may wrap.
    pub reclaimed_bytes_before_gc: usize,
    /// Number of bytes freed explicitly since the recent GC.
    pub expl_freed_bytes_since_gc: usize,
}
