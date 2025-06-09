//! Compilation of native BDWGC
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(feature = "tracing")]
use tracing::instrument;

use crate::core::build_steps::llvm;
use crate::core::builder::{Builder, RunConfig, ShouldRun, Step};
use crate::core::config::TargetSelection;
use crate::trace;
use crate::utils::build_stamp::{BuildStamp, generate_smart_stamp_hash};
use crate::utils::helpers::{self, t};

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct Bdwgc {
    pub target: TargetSelection,
}

fn libgc_built_path(out_dir: &Path) -> PathBuf {
    out_dir.join("lib")
}

impl Step for Bdwgc {
    type Output = PathBuf;
    const ONLY_HOSTS: bool = true;

    fn should_run(run: ShouldRun<'_>) -> ShouldRun<'_> {
        run.path("src/bdwgc")
    }

    fn make_run(run: RunConfig<'_>) {
        run.builder.ensure(Bdwgc { target: run.target });
    }

    fn run(self, builder: &Builder<'_>) -> PathBuf {
        builder.require_submodule("src/bdwgc", None);
        let target = self.target;
        let out_dir = builder.bdwgc_out(target);
        let libgc = libgc_built_path(&out_dir);
        if builder.config.dry_run() {
            return libgc;
        }

        static STAMP_HASH_MEMO: OnceLock<String> = OnceLock::new();
        let smart_stamp_hash = STAMP_HASH_MEMO.get_or_init(|| {
            generate_smart_stamp_hash(
                builder,
                &builder.config.src.join("src/bdwgc"),
                builder.bdwgc_info.sha().unwrap_or_default(),
            )
        });

        let stamp = BuildStamp::new(&out_dir).add_stamp(smart_stamp_hash);

        trace!("checking build stamp to see if we need to rebuild BDWGC artifacts");
        if stamp.is_up_to_date() {
            trace!(?out_dir, "bdwgc build artifacts are up to date");
            if stamp.stamp().is_empty() {
                builder.info(
                    "Could not determine the BDWGC submodule commit hash. \
                     Assuming that an BDWGC rebuild is necessary.",
                );
            } else {
                builder.info(&format!(
                    "To force BDWGC to rebuild, remove the file `{}`",
                    stamp.path().display()
                ));
                return libgc;
            }
        }

        trace!(?target, "(re)building BDWGC artifacts");
        builder.info(&format!("Building BDWGC for {}", target));
        t!(stamp.remove());
        let _time = helpers::timeit(builder);
        t!(fs::create_dir_all(&out_dir));

        builder.config.update_submodule(Path::new("src").join("bdwgc").to_str().unwrap());
        let mut cfg = cmake::Config::new(builder.src.join("src/bdwgc"));
        llvm::configure_cmake(builder, target, &mut cfg, true, llvm::LdFlags::default(), &[]);

        let profile = match (builder.config.bdwgc_debug, builder.config.bdwgc_assertions) {
            (true, _) => "Debug",
            (false, false) => "Release",
            (false, true) => "RelWithDebInfo",
        };
        trace!(?profile);

        let assertions = if builder.config.bdwgc_assertions { "ON" } else { "OFF" };

        cfg.out_dir(&out_dir)
            .pic(true)
            .profile(profile)
            .define("BUILD_SHARED_LIBS", "ON")
            .define("enable_parallel_mark", "OFF")
            .define("enable_gc_assertions", assertions)
            .cflag("-DGC_ALWAYS_MULTITHREADED");

        cfg.build();

        t!(stamp.write());
        libgc
    }
}
