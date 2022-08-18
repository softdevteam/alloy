#!/bin/sh
#
# Build script for continuous integration.

set -e

export CARGO_HOME="`pwd`/.cargo"
export RUSTUP_HOME="`pwd`/.rustup"

COMMIT_HASH=$(git rev-parse --short HEAD)
TARBALL_TOPDIR=`pwd`/build/rustgc-stage2-latest
TARBALL_NAME=rustgc-stage2-${COMMIT_HASH}.tar.bz2
SYMLINK_NAME=rustc-stage2-latest.tar.bz2
SNAP_DIR=/opt/rustgc-bin-snapshots

# Ensure the build fails if it uses excessive amounts of memory.
ulimit -d $((1024 * 1024 * 8)) # 8 GiB

/usr/bin/time -v python3 x.py test --stage 2 --config .buildbot.config.toml --exclude rustdoc-json --exclude debuginfo

# Build extended tools and install into TARBALL_TOPDIR.
mkdir -p ${TARBALL_TOPDIR}
/usr/bin/time -v ./x.py install --config .buildbot.config.toml

# Check that the install looks feasible.
for i in rustc cargo rustdoc; do
    test -e ${TARBALL_TOPDIR}/bin/${i}
done

# Archive the build and put it in /opt
git show -s HEAD > ${TARBALL_TOPDIR}/VERSION
cd build
tar jcf ${TARBALL_NAME} `basename ${TARBALL_TOPDIR}`
chmod 775 ${TARBALL_NAME}
mv ${TARBALL_NAME} ${SNAP_DIR}
ln -sf ${SNAP_DIR}/${TARBALL_NAME} ${SNAP_DIR}/${SYMLINK_NAME}

# Remove all but the 10 latest builds
cd ${SNAP_DIR}
sh -c "ls -tp | grep -v '/$' | tail -n +2 | xargs -I {} rm -- {}"
