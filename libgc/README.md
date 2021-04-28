# libgc

[![Bors enabled](https://bors.tech/images/badge_small.svg)](https://bors.tech)

_libgc_ is a garbage collection library for Rust. It can be used as a standalone
library, but it is highly recommended that programs are compiled with the
companion [rustc fork](https://github.com/softdevteam/rustgc), which offers
language support for much better performance.

_libgc_ is in active development - there will be bugs!

## Example

_libgc_ provides a smart pointer, `Gc<T>`, which can be used to make garbage
collected values. An example, with the necessary global allocator setup, looks
as follows:

```rust
use libgc::{Gc, GcAllocator};

#[global_allocator]
static ALLOCATOR: GcAllocator = GcAllocator;


fn foo() -> Gc<Vec<usize>> {
    let foo = Gc::new(vec![1,2,3]);  
    let a = foo; // GC pointers are copyable
    let b = foo;

    foo 
}

fn main() {
    let gc = foo();
}
```

## Overview

If you want to write code with shared ownership in Rust, `Rc` makes this
possible. Unfortunately, managing cyclic data structures with reference counting
is hard: weak pointers are needed to break strong cycles and thus prevent memory
leaks. In programs where these sorts of structures are common,
garbage collection is a natural fit.

_libgc_ is not a replacement to the single ownership model - it is intended to
complement it by providing a garbage collected alternative for values which
might be too difficult to manage with `Rc`. Values must opt-in to using
garbage collection with the `Gc::new(x)` constructor. This tells _libgc_ to heap
allocate `x`, and GC it for you when you're done with it. `Gc` can be thought of
as a special `Box` type, where `x`'s lifetime is unknown at compile-time.
Periodically, the garbage collector will interrupt the program (known as
"stopping the world") to see which `Gc` values are still in use, and drop those
which aren't. 

Garbage collection involves scanning parts of the stack and heap to look for
live references to `Gc` values. This means that _libgc_ must be aware of all heap
allocated values, even those which aren't `Gc`. To do this, _libgc_ has its own
allocator, `GcAllocator`, which must be set as the global allocator when using
_libgc_.

```rust
use libgc::GcAllocator;

#[global_allocator]
static ALLOCATOR: GcAllocator = GcAllocator;
```

### Finalization

A `Gc` can be used to manage values which have a `drop` method. Like all tracing
garbage collectors, _libgc_ can not provide any guarantees about exactly when a
'dead' value is dropped. Instead, once _libgc_ has determined that a value is
unreachable, its `drop` method is added to a drop queue, which is ran on a
parallel finalization thread at some point in the future. The order of
finalization is intentionally undefined to allow _libgc_ to run `drop` methods on
values which contain cycles of `Gc`.

:warning: You must not dereference a field of type `Gc<T>` inside `Drop::drop`.
Doing so is unsound and can lead to dangling pointers. TODO: Add a lint for this
and explain why in further details.

## Implementation

__libgc__ is implemented using the [Boehm-Demers-Weiser
collector](https://github.com/https://github.com/ivmai/bdwgc).  It is a
conservative, stop-the-world, parallel, mark-sweep collector.

TODO: Expand

## Known Issues

* Single-threaded support only.
* No Drop Lint to prevent unsound dereferencing of `Gc` typed fields.

## Using libgc with rustgc

There are two repositories which make up the gc infrastructure:

* **libgc** the main library which provides the `Gc<T>` smart pointer and its
      API.
* **rustgc** a fork of rustc with GC-aware optimisations. This can be used to
      compile user programs which use libgc, giving them better GC
      performance. Use of rustgc is not mandated, but it enables further
      optimisations for programs which use libgc.

This seperation between libgc and rustgc exists so that a stripped-down form of
garbage collection can be used without compiler support. 

TODO: Explain rustgc and it's optimizations.
