#!/bin/sh
#
# Build script for continuous integration.

set -e

# Ensure the build fails if it uses excessive amounts of memory.
ulimit -d $((1024 * 1024 * 8)) # 8 GiB

./x.py test --stage 1 --exclude rustdoc-json --exclude debuginfo
