use std::env;
use std::path::Path;
use std::process::Command;
#[cfg(not(all(target_pointer_width = "64", target_arch = "x86_64")))]
compile_error!("Requires x86_64 with 64 bit pointer width.");

#[cfg(not(feature = "link-shared"))]
fn build_bdwgc() {
    use std::env;
    use std::path::PathBuf;
    use std::process::Command;

    const BDWGC_REPO: &str = "../../src/bdwgc";
    const BDWGC_BUILD_DIR: &str = "lib";

    let out_dir = env::var("OUT_DIR").unwrap();
    let bdwgc_src = PathBuf::from(BDWGC_REPO);

    if bdwgc_src.read_dir().unwrap().count() == 0 {
        Command::new("git")
            .args(["submodule", "update", "--init", BDWGC_REPO])
            .output()
            .expect("Failed to clone BDWGC repo");
    }

    let mut build_dir = PathBuf::from(&out_dir);
    build_dir.push(BDWGC_BUILD_DIR);

    let mut build = cmake::Config::new(&bdwgc_src);
    build
        .pic(true)
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("enable_parallel_mark", "Off")
        .cflag("-DGC_ALWAYS_MULTITHREADED");

    #[cfg(feature = "gc-assertions")]
    build.define("enable_gc_assertions", "ON");

    #[cfg(not(feature = "gc-debug"))]
    build.profile("Release");

    #[cfg(feature = "gc-debug")]
    build.profile("Debug");

    build.build();

    println!("cargo:rustc-link-search=native={}", &build_dir.display());
    println!("cargo:rustc-link-lib=static=gc");
}

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
    #[cfg(not(feature = "link-shared"))]
    build_bdwgc();
    #[cfg(feature = "link-shared")]
    println!("cargo:rustc-link-lib=dylib=gc");
}
