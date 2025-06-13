#!/bin/sh
#
# Build script for continuous integration.

YKSOMV=4d3679fca1139876adccdbd9a27f110b91b229fa
GRMTOOLSV=7fff436c3659f103eac658e68808990410d2594b
set -e

export CARGO_HOME="$(pwd)/.cargo"
export RUSTUP_HOME="$(pwd)/.rustup"

# Ensure the build fails if it uses excessive amounts of memory.
ulimit -d $((1024 * 1024 * 18)) # 18 GiB

if $(git rev-parse --is-shallow-repository); then
    git fetch --unshallow
fi

/usr/bin/time -v python3 x.py test --stage 2 --config .buildbot.config.toml --exclude rustdoc-json --exclude debuginfo --exclude run-make

# Install rustup

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs >rustup.sh
sh rustup.sh --default-host x86_64-unknown-linux-gnu \
	--default-toolchain nightly \
	--no-modify-path \
	--profile minimal \
	-y
export PATH=$(pwd)/.cargo/bin/:$PATH

rustup toolchain link alloy build/x86_64-unknown-linux-gnu/stage2

# Build and test yksom
git clone --recursive https://github.com/softdevteam/yksom
cd yksom && git checkout $YKSOMV

# Annoying hack needed in order to build a non-workspace crate inside alloy.
echo "[workspace]" >>Cargo.toml

cargo +alloy test
cargo +alloy test --release

cargo +alloy run -- --cp SOM/Smalltalk SOM/TestSuite/TestHarness.som
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
cd grmtools && git checkout $GRMTOOLSV

cargo +alloy test
cargo +alloy test --release

cargo +alloy test --lib cfgrammar --features serde
cargo +alloy test --lib lrpar --features serde
