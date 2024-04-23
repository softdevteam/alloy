//@ run-fail
// ignore-tidy-linelength
#![feature(gc)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use std::gc::Gc;
use std::ffi::{CString, NulError};
use std::io::{self, Write};

fn fallible() -> Result<(), NulError> {
    let e: NulError = CString::new(b"f\0oo".to_vec()).unwrap_err();
    Err(e)
}

fn main() -> io::Result<()> {
    let mut _stdout = io::stdout().lock();
    fallible()?;
    let gc = Gc::new(123);
    fallible()?;
    Ok(())
}
