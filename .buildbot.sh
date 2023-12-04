#!/bin/sh
#
# Build script for continuous integration.

set -e

# This is needed because Alloy is rebased on top of rustc, and we need enough
# depth for the bootstrapper to find the correct llvm sha
git fetch --unshallow

export CARGO_HOME="`pwd`/.cargo"
export RUSTUP_HOME="`pwd`/.rustup"

# Ensure the build fails if it uses excessive amounts of memory.
ulimit -d $((1024 * 1024 * 8)) # 8 GiB

ENABLE_GC_ASSERTIONS=true /usr/bin/time -v python3 x.py test --stage 2 --config .buildbot.config.toml --exclude rustdoc-json --exclude debuginfo

