use std::env;
use std::path::PathBuf;
use std::process::Command;

const BDWGC_REPO: &str = "../../src/bdwgc";
const BDWGC_BUILD_DIR: &str = "lib";

#[cfg(not(all(target_pointer_width = "64", target_arch = "x86_64")))]
compile_error!("Requires x86_64 with 64 bit pointer width.");

fn main() {
    if env::var("GC_LINK_DYNAMIC").map_or(false, |v| v == "true") {
        println!("cargo:rustc-link-lib=dylib=gc");
        return;
    }

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
        .cflag("-DGC_ALWAYS_MULTITHREADED")
        .cflag("-DGC_JAVA_FINALIZATION");

    if env::var("ENABLE_GC_ASSERTIONS").map_or(false, |v| v == "true") {
        build.define("enable_gc_assertions", "ON");
    }

    if env::var("ENABLE_GC_DEBUG").map_or(false, |v| v == "true") {
        build.profile("Debug");
    } else {
        build.profile("Release");
    }

    build.build();

    println!("cargo:rustc-link-search=native={}", &build_dir.display());
    println!("cargo:rustc-link-lib=static=gc");
}
