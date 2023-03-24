#![crate_name = "foo"]

#![feature(negative_impls)]

pub struct Foo;

// @has foo/struct.Foo.html
// @!has - 'Auto Trait Implementations'
impl !Send for Foo {}
impl !Sync for Foo {}
impl !FinalizerSafe for Foo {}
impl !std::marker::Unpin for Foo {}
impl !std::panic::RefUnwindSafe for Foo {}
impl !std::panic::UnwindSafe for Foo {}
impl !std::gc::NoTrace for Foo {}
impl std::gc::ReferenceFree for Foo {}
