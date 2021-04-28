use std::{env, path::PathBuf, process::Command};

use lang_tester::LangTester;
use tempfile::TempDir;

fn deps_path() -> String {
    let mut path = PathBuf::new();
    path.push(env::var("CARGO_MANIFEST_DIR").unwrap());
    path.push("target");
    #[cfg(debug_assertions)]
    path.push("debug");
    #[cfg(not(debug_assertions))]
    path.push("release");
    path.push("deps");

    path.to_str().unwrap().to_owned()
}

fn main() {
    let rustc = env::var("RUSTGC").expect("RUSTGC environment var not specified");
    // We grab the rlibs from `target/<debug | release>/` but in order
    // for them to exist here, they must have been moved from `deps/`.
    // Simply running `cargo test` will not do this, instead, we must
    // ensure that `cargo build` has been run before running the tests.

    #[cfg(debug_assertions)]
    let build_mode = "--debug";
    #[cfg(not(debug_assertions))]
    let build_mode = "--release";

    Command::new("cargo")
        .args(&["build", build_mode])
        .env("RUSTC", &rustc.as_str())
        .output()
        .expect("Failed to build libs");

    let tempdir = TempDir::new().unwrap();
    LangTester::new()
        .test_dir("gc_tests/tests")
        .test_file_filter(|p| p.extension().unwrap().to_str().unwrap() == "rs")
        .test_extract(|s| {
            Some(
                s.lines()
                    .take_while(|l| l.starts_with("//"))
                    .map(|l| &l[2..])
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        })
        .test_cmds(move |p| {
            let mut exe = PathBuf::new();
            exe.push(&tempdir);
            exe.push(p.file_stem().unwrap());

            let mut compiler = Command::new(&rustc);
            compiler.args(&[
                "-L",
                deps_path().as_str(),
                p.to_str().unwrap(),
                "-o",
                exe.to_str().unwrap(),
            ]);

            vec![("Compiler", compiler), ("Run-time", Command::new(exe))]
        })
        .run();
}
