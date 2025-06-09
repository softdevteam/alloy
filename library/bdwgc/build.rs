use std::env;
use std::path::Path;
use std::process::Command;
#[cfg(not(all(target_pointer_width = "64", target_arch = "x86_64")))]
compile_error!("Requires x86_64 with 64 bit pointer width.");

fn main() {
    let cwd = env::var("CARGO_MANIFEST_DIR").unwrap();
    let header = Path::new(&cwd)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("src")
        .join("bdwgc")
        .join("include")
        .join("gc.h");

    let bindgen = std::env::var("RUSTC_BINDGEN").unwrap();
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("bindings.rs");
    let status = Command::new(bindgen)
        .args(&[
            "--ctypes-prefix",
            "libc",
            "--use-core",
            "-o",
            out.to_str().unwrap(),
            header.to_str().unwrap(),
            "--",
            "-DGC_THREADS",
        ])
        .status()
        .unwrap();

    if !status.success() {
        panic!("bindgen failed with status: {:?}", status);
    }
    println!("cargo::rustc-link-lib=dylib=gc");
}
