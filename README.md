# Alloy: opt-in tracing garbage collection for Rust

Alloy is a fork of the Rust language with support for opt-in tracing garbage
collection (GC) using the `Gc<T>` type. It is a research project designed to
help make **writing cyclic data structures easier** in Rust.

Alloy is not production-ready. However, it is sufficiently polished to be usable
for real programs: it supports GC across multiple threads; has high-quality
error messages; and reasonable performance.

## Using Alloy to write a doubly-linked list

The following example program shows how we can use Alloy's `Gc<T>` smart pointer
to write a doubly-linked list with three nodes:

```rust
use std::gc::Gc;
use std::cell::RefCell;

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

