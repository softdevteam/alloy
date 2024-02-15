# Alloy: opt-in tracing garbage collection for Rust

Alloy is a fork of the Rust language with support for opt-in tracing garbage
collection (GC) using the `Gc<T>` type. It is a research project designed to
help make **writing cyclic data structures easier** in Rust.

Alloy is not production-ready. However, it is sufficiently polished to be usable
for real programs: it supports GC across multiple threads; has high-quality
error messages; and reasonable performance.

> :warning: Alloy won't be able trace objects for garbage collection unless you
> set the `#[global_allocator]` to use `std::gc::GcAllocator`.

## Using Alloy to write a doubly-linked list

The following example program shows how we can use Alloy's `Gc<T>` smart pointer
to write a doubly-linked list with three nodes:

```rust
#![feature(gc)]
use std::gc::{Gc, GcAllocator};
use std::cell::RefCell;

#[global_allocator]
static A: GcAllocator = GcAllocator;

struct Node {
    name: &'static str,
    prev: Option<Gc<RefCell<Node>>>,
    next: Option<Gc<RefCell<Node>>>,
}

fn main() {
    let c = Gc::new(RefCell::new(Node { name: "c", prev: None, next: None}));
    let b = Gc::new(RefCell::new(Node { name: "b", prev: None, next: Some(c)}));
    let a = Gc::new(RefCell::new(Node { name: "a", prev: None, next: Some(b)}));

    // Now patch in the previous nodes
    c.borrow_mut().next = Some(b);
    b.borrow_mut().next = Some(a);
}
```

This is similar to using Rust's `Rc` smart pointer, but instead, there is a
garbage collector running in the background which will automatically free the
`Gc` values when they're no longer used. There are two main ergonomic benefits
to using Alloy:

1. The `Gc` type is `Copy`, so new pointers can be created easily without
   needing to `clone` them.
2. Alloy supports cyclic references by design, so there's no need to use `Weak`
   references.

## Building Alloy

### Dependencies

Make sure you have installed the dependencies:

* `rustup`
* `python` 3 or 2.7
* `git`
* A C compiler (when building for the host, `cc` is enough; cross-compiling may
  need additional compilers)
* `curl`
* `pkg-config` if you are compiling on Linux and targeting Linux
* `libiconv` (already included with glibc on Debian-based distros)
* `g++`, `clang++`, or MSVC with versions listed on
  [LLVM's documentation](https://llvm.org/docs/GettingStarted.html#host-c-toolchain-both-compiler-and-standard-library)
* `ninja`, or GNU `make` 3.81 or later (Ninja is recommended, especially on
  Windows)
* `cmake` 3.13.4 or later
* `libstdc++-static` may be required on some Linux distributions such as Fedora
  and Ubuntu

### Build steps

[installation guide]: https://github.com/rust-lang/rust#installing-from-source

Building Alloy from source is the same process as building the official Rust
compiler from source. For a more detailed guide on how this is done, along with
the different configuration options, follow the [installation guide] from the
official Rust repository.

1. Clone the [source] with `git`:

   ```sh
   git clone https://github.com/softdevteam/alloy.git
   cd rust
   ```

[source]: https://github.com/softdevteam/alloy

2. Configure the build settings:

   ```sh
   ./configure
   ```

   If you plan to use `x.py install` to create an installation, it is
   recommended that you set the `prefix` value in the `[install]` section to a
   directory: `./configure --set install.prefix=<path>`

3. Build and install:

   ```sh
   ./x.py build && ./x.py install
   ```

   When complete, `./x.py install` will place several programs into
   `$PREFIX/bin`: `rustc`, the Rust compiler, and `rustdoc`, the
   API-documentation tool. By default, it will also include [Cargo], Rust's
   package manager. You can disable this behavior by passing
   `--set build.extended=false` to `./configure`.

4. Add the Alloy toolchain to rustup:

   ```sh
   rustup toolchain link alloy /path/to/alloy/rustc
   ```

Rust programs which use cargo can now be built and run using Alloy instead of
the official Rust compiler:

   ```sh
   cargo +alloy build
   ```

## How it works

[Boehm Demers Weiser GC (BDWGC)]: https://github.com/ivmai/bdwgc

### The `Gc<T>` smart pointer

Alloy provides a new smart pointer type, `Gc<T>`, which allows for shared
ownership of a value of type `T` allocated in the heap and managed by a garbage
collector. 

```rust
use std::gc::Gc;

fn main() {
    let a = Gc::new(123);
}
```

This creates a garbage collected object containing the `u64` value `123`.

### Interior mutability

There is no way to mutate, or obtain a mutable reference (`&mut T`) to the
contents of a `Gc<T>` once it has been allocated. This is because mutable
references must not alias with any other references, and there is no way to know
at compile-time whether there is only one `Gc` reference to the data.

As with other shared ownership types in Rust, interior mutability (e.g.
`RefCell`, `Mutex`, etc) must be used when mutating the contents inside a `Gc`:

```rust
fn main() {
    let a = Gc::new(RefCell::new(123));
    *a.borrow_mut() = 456; // Mutate the value inside the GC
}
```

### The collector

Alloy uses _conservative_ garbage collection. This means that it does not have
any specific knowledge about where references to objects are located. Instead,
Alloy will assume that an object is still alive if it can be reached by a value on
the stack (or in a register) which, if treated like a pointer, points to an
object in the heap. The fields of those objects are then traced using the same
approach, until all live objects in the program have been discovered. 

This tends to work well in practice, however, it comes with an important caveat:
you must not hide references from the GC. For example, data structures
such as XOR lists are unsound because Alloy will never be able to reach their
objects.

Behind the scenes, Alloy uses the [Boehm Demers Weiser GC (BDWGC)] for its
garbage collection implementation. This supports incremental, generational,
parallel (but not concurrent!)[^1] collection.

[^1]: A _concurrent_ collector is one where threads doing GC work can run at the
    same time as normal program (i.e. mutator) threads. A _parallel_ garbage
    collector simply means that the garbage collection workload can be
    parallelised across multiple worker threads.

### Finalisation

Finalisers are a common component of most tracing GCs which are used to run code
for cleanup once an object dies (e.g.~closing a file handle or a database
connection).

Alloy takes a novel approach to finalisation compared to previous Rust GCs in
that it uses existing drop methods as garbage collection finalisers, saving
users the potentially error-prone task of manually writing both destructor and
finaliser methods for GC managed objects.

When a `Gc`'s underlying contents becomes unreachable, Alloy will call its
finaliser, which means that `drop` is called on all the component types (in the
same way that Rust automatically calls `drop` in an RAII context).

#### Finalisation order

To achieve Alloy's goal of making cyclic data structures easier to write, we
made the decision to support the finalisation of objects with cycles. This means
that no guarantees are made about the order in which finalisers are run. In
order for this to be sound, you must not access other `Gc` objects from inside
the `drop` method a `Gc`. Alloy ensures that this rule is followed by checking
for potential misuses of `drop` at compile-time with _Finaliser
Safety Analysis_ (FSA). Consider the following example:

```rust
struct Node {
    next: Gc<usize>,
}

impl Drop for Node {
    fn drop(&mut self) {
        *self.next;
    }
}

fn main {
    let x = Gc::new(Node { Gc::new(123) });
}
```

If at any point you try to create `Gc<Node>`, such as in `main` above, Alloy will throw the following
compiler error:

```
error: `Node { next: Gc::new(123) }` cannot be safely finalized.
  --> src/main.rs
   |
12 |     let x = Gc::new(Node { next: Gc::new(123) });
   |                     ^^^^^^^^^^^^^^^^^^^^^^^^^ has a drop method which cannot be safely finalized.
...
2  |         *self.next;
   |          ---------
   |          |
   |          caused by the expression here in `fn drop(&mut)` because
   |          it uses another `Gc` type.
   |
   = help: `Gc` finalizers are unordered, so this field may have already been dropped.
     It is not safe to dereference.
```

As this suggests, Finaliser Safety Analysis is only applied to a given `drop`
method if its parent type is used in a `Gc`.

FSA is conservative and can rule out drop methods that a human can determine are
in fact safe to be used as finalisers. For those situations you can `unsafe
impl` the `FinalizerSafe` trait, which overrides FSA for a given type.

#### Concurrency-safe finalisation

Alloy runs finalisers on a dedicated finalisation thread. This is because
finalisers can potentially run at any time during the execution of a Rust
program, and if they try to acquire locks to shared data which is already being
held, the program could crash or even deadlock. Finalising on a dedicated thread
means that finalisation can wait safely until the user program no longer holds
exclusive access. (See [Destructors, Finalizers, and Synchronization](https://dl.acm.org/doi/pdf/10.1145/604131.604153) for a more
in-depth discussion of this issue).

However, in order to safely finalise objects on a separate thread, finalisers
must access data in a thread-safe way. Alloy ensures that this happens at
compile-time by using finaliser safety analysis to check that only values marked
`Send` or `Sync` are used inside a `drop` method when used during finalisation.

## Known limitations

* Alloy is limited to x86-64 architectures.
* Alloy uses the BDWGC's handlers for the SIGXCPU and SIGPWR signals to
  co-ordinate pausing threads so that GC can happen. It cannot be used with
  programs which also catch these signals.
* Alloy does not support semi-conservative collection (i.e. precise
  tracing through heap allocated struct / enum fields).
* Alloy has only been tested on Linux.

## License

Rust is primarily distributed under the terms of both the MIT license and the
Apache License (Version 2.0), with portions covered by various BSD-like
licenses.

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT), and
[COPYRIGHT](COPYRIGHT) for details.

