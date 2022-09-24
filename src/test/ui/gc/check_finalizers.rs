// check-fail
#![feature(gc)]

use std::gc::Gc;

struct Hello(*mut u8);

impl Drop for Hello {
    fn drop(&mut self) {
        println!("Dropping Hello");
    }
}

fn main() {
    Gc::new(Hello(123 as *mut u8)); //~ ERROR `Hello(123 as *mut u8)` cannot be safely finalized.
    //~| ERROR `Hello(123 as *mut u8)` cannot be safely finalized.
}
