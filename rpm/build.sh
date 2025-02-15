#!/usr/bin/env bash

set -exEuo pipefail

NAME='mctl'
VERSION=$(
    cargo metadata --format-version 1 --no-deps --locked |
        jq --raw-output ".packages.[] | select(.name == \"$NAME\") | .version"
)

rpm_version=$(
    rpmspec --query rust-mctl.spec --queryformat '%{name}:%{version}\n' |
        head -n1 |
        cut -d':' -f 2
)

if [[ "$VERSION" != "$rpm_version" ]]; then
    echo "crate and rpm version differs"
    exit 1
fi

if rpmspec --query rust-mctl.spec |
    grep rust |
    xargs -I '{}' test -f '/home/joshuachp/rpmbuild/RPMS/x86_64/{}.rpm'; then
    echo "packeage already exists"
    exit 1
fi

cargo package --locked
cp -v "$CARGO_TARGET_DIR/package/$NAME-$VERSION.crate" .

tmpdir=$(mktemp -d)
cargo vendor --locked "$tmpdir/vendor"
tar -C "$tmpdir" -cf "rust-$NAME-$VERSION-vendor.tar.xz" "vendor"

cp -v "$NAME-$VERSION.crate" ~/rpmbuild/SOURCES/
cp -v "rust-$NAME-$VERSION-vendor.tar.xz" ~/rpmbuild/SOURCES/
cp -v "rust-$NAME.spec" ~/rpmbuild/SPECS/

if [[ ${1:-} == 's' ]]; then
    rpmbuild -bs "$HOME/rpmbuild/SPECS/rust-$NAME.spec"
else
    rpmbuild -ba "$HOME/rpmbuild/SPECS/rust-$NAME.spec"
fi
