#!/bin/sh
#
# Build script for continuous integration.

set -e

# This is needed because Alloy is rebased on top of rustc, and we need enough
# depth for the bootstrapper to find the correct llvm sha
git fetch --unshallow

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rustup.sh
sh rustup.sh --default-host x86_64-unknown-linux-gnu \
    --default-toolchain nightly \
    --no-modify-path \
    --profile minimal \
    -y
export PATH=`pwd`/.cargo/bin/:$PATH

export CARGO_HOME="`pwd`/.cargo"
export RUSTUP_HOME="`pwd`/.rustup"

# Ensure the build fails if it uses excessive amounts of memory.
ulimit -d $((1024 * 1024 * 8)) # 8 GiB

/usr/bin/time -v python3 x.py test --stage 2 --config .buildbot.config.toml --exclude rustdoc-json --exclude debuginfo

# Build and test yksom
rustup toolchain link alloy build/x86_64-unknown-linux-gnu/stage1
git clone --recursive https://github.com/softdevteam/yksom
cd yksom
cargo +alloy test
cargo +alloy test --release
