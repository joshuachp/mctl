#!/usr/bin/env bash

set -exEuo pipefail

VERSION="0.1.0"

cargo package --locked
cp -vT "$CARGO_TARGET_DIR/package/mctl-$VERSION.crate" "$HOME/rpmbuild/SOURCES/rust-mctl-$VERSION.crate"

tmp=$(mktemp -d)
cargo vendor --locked "$tmp/vendor"
tar -C "$tmp" -cf "$HOME/rpmbuild/SOURCES/rust-mctl-$VERSION-vendor.tar.xz" vendor

cp -v rpm/rust-mctl.spec ~/rpmbuild/SPECS

rpmbuild -ba ~/rpmbuild/SPECS/rust-mctl.spec
