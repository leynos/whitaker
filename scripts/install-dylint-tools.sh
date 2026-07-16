#!/usr/bin/env sh
# install-dylint-tools.sh — Ensure the pinned cargo-dylint and dylint-link
# versions are available, installing into an isolated root when the
# system-wide binaries are missing or the wrong version.
#
# Usage:
#   scripts/install-dylint-tools.sh TOOLS_ROOT CARGO_DYLINT_VERSION DYLINT_LINK_VERSION [CARGO]
#
# TOOLS_ROOT is used as the cargo install --root; binaries land in
# TOOLS_ROOT/bin, which the caller should prepend to PATH when it exists.
# The root is only created when an install is needed, so callers can use
# its absence to mean "the system tools already match".
#
# cargo-dylint is probed via its --version output. dylint-link cannot be
# probed that way: it is a linker shim whose --version is forwarded to
# cc, so the installed version is read from `cargo install --list`.
#
# Exits non-zero if any required install fails, so callers never proceed
# with stale tools.
set -eu

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
    echo "usage: $0 TOOLS_ROOT CARGO_DYLINT_VERSION DYLINT_LINK_VERSION [CARGO]" >&2
    exit 2
fi

tools_root=$1
cargo_dylint_version=$2
dylint_link_version=$3
cargo=${4:-cargo}

installed_cargo_dylint=$(cargo-dylint --version 2>/dev/null | awk '{print $2}' || true)
if [ "$installed_cargo_dylint" != "$cargo_dylint_version" ]; then
    "$cargo" install --locked --version "$cargo_dylint_version" \
        --root "$tools_root" cargo-dylint
fi

if ! "$cargo" install --list 2>/dev/null |
    grep -q "^dylint-link v$dylint_link_version:"; then
    "$cargo" install --locked --version "$dylint_link_version" \
        --root "$tools_root" dylint-link
fi
