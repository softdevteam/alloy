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

# Install rustup

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rustup.sh
sh rustup.sh --default-host x86_64-unknown-linux-gnu \
    --default-toolchain nightly \
    --no-modify-path \
    --profile minimal \
    -y
export PATH=`pwd`/.cargo/bin/:$PATH

rustup toolchain link alloy build/x86_64-unknown-linux-gnu/stage1

# Build and test yksom
git clone --recursive https://github.com/softdevteam/yksom
cd yksom

# Annoying hack needed in order to build a non-workspace crate inside alloy.
echo "[workspace]" >> Cargo.toml

cargo +alloy test
cargo +alloy test --release

cargo +alloy run  -- --cp SOM/Smalltalk SOM/TestSuite/TestHarness.som
cargo +alloy run --release -- --cp SOM/Smalltalk SOM/TestSuite/TestHarness.som

cargo +alloy run --release -- --cp SOM/Smalltalk:lang_tests hello_world1

cd SOM
cargo +alloy run --release -- \
  --cp Smalltalk:TestSuite:SomSom/src/compiler:SomSom/src/vm:SomSom/src/vmobjects:SomSom/src/interpreter:SomSom/src/primitives \
  SomSom/tests/SomSomTests.som
cargo +alloy run --release -- \
  --cp Smalltalk:Examples/Benchmarks/GraphSearch \
  Examples/Benchmarks/BenchmarkHarness.som GraphSearch 10 4

# Build and test grmtools
cd ../
git clone https://github.com/softdevteam/grmtools
cd grmtools

cargo +alloy test
cargo +alloy test --release

cargo +alloy test --lib cfgrammar --features serde
cargo +alloy test --lib lrpar --features serde
