use cmake;
use std::env;
use std::path::PathBuf;
use std::process::Command;

const BOEHM_REPO: &str = "https://github.com/softdevteam/bdwgc.git";
const BOEHM_ATOMICS_REPO: &str = "https://github.com/ivmai/libatomic_ops.git";
const BOEHM_DIR: &str = "bdwgc";
const BUILD_DIR: &str = "lib";

#[cfg(not(all(target_pointer_width = "64", target_arch = "x86_64")))]
compile_error!("Requires x86_64 with 64 bit pointer width.");

fn run<F>(name: &str, mut configure: F)
where
    F: FnMut(&mut Command) -> &mut Command,
{
    let mut command = Command::new(name);
    let configured = configure(&mut command);
    if !configured.status().is_ok() {
        panic!("failed to execute {:?}", configured);
    }
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let mut boehm_src = PathBuf::from(&out_dir);
    boehm_src.push(BOEHM_DIR);
    let mut build_dir = PathBuf::from(&out_dir);
    build_dir.push(BUILD_DIR);

    if !boehm_src.exists() {
        run("git", |cmd| cmd.arg("clone").arg(BOEHM_REPO).arg(&boehm_src));
        run("git", |cmd| cmd.arg("clone").arg(BOEHM_ATOMICS_REPO).current_dir(&boehm_src));
    }

    cmake::Config::new(&boehm_src)
        .pic(true)
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("enable_allocation_switching", "ON")
        .cflag("-DGC_IGNORE_WARN")
        .cflag("-DGC_ALWAYS_MULTITHREADED")
        .build();

    println!("cargo:rustc-link-search=native={}", &build_dir.display());
    println!("cargo:rustc-link-lib=static=gc");
}
