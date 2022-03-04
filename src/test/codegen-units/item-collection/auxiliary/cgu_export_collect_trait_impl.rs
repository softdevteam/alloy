#![crate_type = "lib"]
#![feature(gc)]

use std::gc::Collectable;

pub struct S(pub usize);
pub struct T(pub U);
pub struct U(pub usize);
pub struct V(pub *const X);
pub struct X(pub usize);

unsafe impl Collectable for S {
    unsafe fn set_collectable(&self) {}
}

unsafe impl Collectable for U {
    unsafe fn set_collectable(&self) {}
}

unsafe impl Collectable for V {
    unsafe fn set_collectable(&self) {
        // Explicit call of inner X's `set_collectable`
        unsafe { (&*self.0).set_collectable() };
    }
}

unsafe impl Collectable for X {
    unsafe fn set_collectable(&self) {}
}

pub struct EA;
pub struct EB;
pub struct EC;
pub enum E{EA(EA), EB(EB), EC(EC)}

unsafe impl Collectable for EA {
    unsafe fn set_collectable(&self) {}
}

unsafe impl Collectable for EB {
    unsafe fn set_collectable(&self) {}
}

unsafe impl Collectable for EC {
    unsafe fn set_collectable(&self) {}
}

pub struct TupA;
pub struct TupB;

unsafe impl Collectable for TupA {
    unsafe fn set_collectable(&self) {}
}

unsafe impl Collectable for TupB {
    unsafe fn set_collectable(&self) {}
}

#[derive(Copy, Clone)]
pub struct ArrElem;

unsafe impl Collectable for ArrElem {
    unsafe fn set_collectable(&self) {}
}

pub struct SliceElem;

unsafe impl Collectable for SliceElem {
    unsafe fn set_collectable(&self) {}
}
