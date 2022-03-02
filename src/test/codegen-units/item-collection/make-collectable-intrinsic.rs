// compile-flags:-Zprint-mono-items=eager

#![deny(dead_code)]
#![feature(start)]
#![feature(gc)]

// aux-build:cgu_export_collect_trait_impl.rs
extern crate cgu_export_collect_trait_impl;

use std::mem::make_collectable;
use cgu_export_collect_trait_impl::{E,EA,S,T,U,V,X,TupA,TupB,ArrElem,SliceElem};

//~ MONO_ITEM fn start
#[start]
fn start(_: isize, _: *const *const u8) -> isize {
    let s = S(123);
    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::S as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn std::mem::make_collectable::<cgu_export_collect_trait_impl::S>
    unsafe { make_collectable(&s) };

    let t = T(U(123));
    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::U as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn std::mem::make_collectable::<cgu_export_collect_trait_impl::T>
    unsafe { make_collectable(&t) };

    let v = V(&X(123) as *const X);
    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::V as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::X as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn std::mem::make_collectable::<cgu_export_collect_trait_impl::V>
    unsafe { make_collectable(&v) };

    let e = E::EA(EA);
    // Even though a single enum variant is instantiated, the mono collector
    // must collect all variants since either could be possible at runtime.

    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::EA as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::EB as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::EC as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn std::mem::make_collectable::<cgu_export_collect_trait_impl::E>
    unsafe { make_collectable(&e) };

    let tup = (TupA, TupB);
    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::TupA as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::TupB as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn std::mem::make_collectable::<(cgu_export_collect_trait_impl::TupA, cgu_export_collect_trait_impl::TupB)>
    unsafe { make_collectable(&tup) };

    let array = [ArrElem; 5];
    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::ArrElem as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn std::mem::make_collectable::<[cgu_export_collect_trait_impl::ArrElem; 5]>
    unsafe { make_collectable(&array) };

    let slice = &[SliceElem];
    //~ MONO_ITEM fn <cgu_export_collect_trait_impl::SliceElem as std::gc::Collectable>::set_collectable
    //~ MONO_ITEM fn std::mem::make_collectable::<&[cgu_export_collect_trait_impl::SliceElem; 1]>
    unsafe { make_collectable(&slice) };
    0
}
