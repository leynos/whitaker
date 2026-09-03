#!/usr/bin/env sh
# install-dylint-tools.sh — Ensure the pinned cargo-dylint and dylint-link
# versions are available, installing them into an isolated root when the
# system-wide binaries are missing or the wrong version.
#
# Usage:
#   scripts/install-dylint-tools.sh TOOLS_ROOT CARGO_DYLINT_VERSION DYLINT_LINK_VERSION [CARGO] [TOOLCHAIN]
#
# Provenance model
# ----------------
# Both tools are fetched as upstream prebuilt release archives from the
# trailofbits/dylint GitHub release matching the requested version, and
# each archive is verified against a SHA-256 digest pinned in this file.
# Nothing is built from source: a CI job must never spend paid minutes
# recompiling a host tool that upstream already publishes as a trusted
# binary. Only versions with a pinned digest can be installed; a request
# for any other version is a hard error rather than an unverified
# download or a silent fall back to a source build.
#
# TOOLS_ROOT owns the installed binaries; they land in TOOLS_ROOT/bin,
# which the caller should prepend to PATH when it exists. The root is
# only created once a download has been verified, so callers can use its
# absence to mean "the system tools already match" and a cache keyed on
# it observes a clean hit or miss.
#
# Version probes
# --------------
# cargo-dylint is probed via `cargo-dylint dylint --version` (the
# subcommand form: since 6.x the binary rejects a bare --version), and
# the version is field 2 of that output. dylint-link cannot be probed
# the same way: it is a linker shim that forwards --version to cc.
#
# Presence is therefore not proof of the requested pin. TOOLS_ROOT is a
# durable, unversioned cache directory that `publish-check` prepends to
# PATH, so a binary installed for an older DYLINT_LINK_VERSION would
# otherwise survive a version bump indefinitely and be paired with a newer
# cargo-dylint. Every install records the version it wrote in a marker file
# beside the binary, and a cached dylint-link is reused only when that
# marker matches the requested version. A system copy on PATH is accepted
# only when TOOLS_ROOT holds no dylint-link at all: it predates this script,
# carries no provenance, and reinstalling over it is not this script's job.
#
# Testing hook
# ------------
# DYLINT_TOOLS_SHA256_CARGO_DYLINT and DYLINT_TOOLS_SHA256_DYLINT_LINK
# override the pinned digest for the matching tool. They exist solely so
# the behavioural tests can verify a locally generated fixture archive;
# they replace the expected digest and can never disable verification.
#
# Exits non-zero if any required install fails, so callers never proceed
# with stale or unverified tools.
set -eu

if [ "$#" -lt 3 ] || [ "$#" -gt 5 ]; then
    echo "usage: $0 TOOLS_ROOT CARGO_DYLINT_VERSION DYLINT_LINK_VERSION [CARGO] [TOOLCHAIN]" >&2
    exit 2
fi

tools_root=$1
cargo_dylint_version=$2
dylint_link_version=$3
# CARGO and TOOLCHAIN are retained only so the caller's contract is
# unchanged now that no Cargo build occurs; neither participates in the
# download-and-verify install path.
# shellcheck disable=SC2034
cargo=${4:-cargo}
# shellcheck disable=SC2034
toolchain=${5:-}

# The single release whose archive digests are pinned below. Bumping the
# tool versions means bumping this constant and every digest with it.
pinned_release_version=6.0.1
release_base_url=https://github.com/trailofbits/dylint/releases/download

fail() {
    echo "install-dylint-tools: $*" >&2
    exit 1
}

# Map the host to the release target triple, rejecting anything upstream
# does not publish a prebuilt archive for.
resolve_target() {
    kernel=$(uname -s)
    if [ "$kernel" != Linux ]; then
        fail "no prebuilt dylint release archive for kernel '$kernel'"
    fi
    machine=$(uname -m)
    case "$machine" in
        x86_64) echo x86_64-unknown-linux-gnu ;;
        aarch64 | arm64) echo aarch64-unknown-linux-gnu ;;
        *) fail "unsupported architecture '$machine'" ;;
    esac
}

# Digests for the v6.0.1 archives, verified by download on 2026-09-03.
pinned_sha256() {
    case "$1:$2" in
        cargo-dylint:x86_64-unknown-linux-gnu)
            echo 9f130d915efbfd1d04160ac9874c617a5d74b48971881e25b5ea6c69e74597f7
            ;;
        cargo-dylint:aarch64-unknown-linux-gnu)
            echo b22864164bfeb6faa391f034dc11d9537c7d9c8c3286bb528219f29bad35c603
            ;;
        dylint-link:x86_64-unknown-linux-gnu)
            echo c47c31479a44ed6d6c8aaf43dfe6a1db65f5e4c4b834c7e7365a1d309e7c1bfd
            ;;
        dylint-link:aarch64-unknown-linux-gnu)
            echo 0d4d9d2e3154a02be9d44383d6ff794a5618b27323b968424b70d00ec2a282ea
            ;;
    esac
}

digest_override() {
    case "$1" in
        cargo-dylint) printf '%s' "${DYLINT_TOOLS_SHA256_CARGO_DYLINT:-}" ;;
        dylint-link) printf '%s' "${DYLINT_TOOLS_SHA256_DYLINT_LINK:-}" ;;
    esac
}

expected_sha256() {
    override=$(digest_override "$1")
    if [ -n "$override" ]; then
        printf '%s\n' "$override"
        return 0
    fi
    pinned_sha256 "$1" "$2"
}

# Provenance marker beside the installed binary. A dotted name keeps it out
# of the way of anything that iterates over TOOLS_ROOT/bin looking for
# executables.
version_marker() {
    printf '%s\n' "$tools_root/bin/.$1.version"
}

record_installed_version() {
    printf '%s\n' "$2" >"$(version_marker "$1")"
}

recorded_version() {
    marker=$(version_marker "$1")
    if [ -f "$marker" ]; then
        cat "$marker"
    fi
}

verify_sha256() {
    archive=$1
    expected=$2
    manifest="${archive}.sha256"
    printf '%s  %s\n' "$expected" "$archive" >"$manifest"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum --check --status "$manifest"
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 -c "$manifest" >/dev/null
    else
        fail "neither sha256sum nor shasum is available to verify $archive"
    fi
}

# Download, verify, and install one tool. TOOLS_ROOT is only touched
# after the digest check passes, and the binary is staged under a .new
# name so a partial write is never observable on the final path.
install_tool() {
    tool=$1
    version=$2

    if [ "$version" != "$pinned_release_version" ]; then
        fail "$tool $version has no pinned SHA-256; only $pinned_release_version is pinned"
    fi

    target=$(resolve_target)
    expected=$(expected_sha256 "$tool" "$target")
    if [ -z "$expected" ]; then
        fail "no pinned SHA-256 for $tool $version on $target"
    fi

    stem="${tool}-${target}-v${version}"
    workdir=$(mktemp -d)
    trap 'rm -rf -- "$workdir"' 0 INT TERM HUP

    archive="$workdir/${stem}.tar.gz"
    curl --fail --location --proto '=https' --tlsv1.2 \
        --silent --show-error \
        --output "$archive" \
        "${release_base_url}/v${version}/${stem}.tar.gz"
    # `sha256sum --check --status` is deliberately silent, so name the
    # offending archive here rather than aborting without a diagnostic.
    if ! verify_sha256 "$archive" "$expected"; then
        fail "SHA-256 mismatch for ${stem}.tar.gz; expected $expected"
    fi

    tar -xzf "$archive" -C "$workdir"
    staged="$workdir/$stem/$tool"
    if [ ! -f "$staged" ]; then
        fail "archive ${stem}.tar.gz did not contain $stem/$tool"
    fi

    mkdir -p "$tools_root/bin"
    cp "$staged" "$tools_root/bin/${tool}.new"
    chmod 0755 "$tools_root/bin/${tool}.new"
    mv "$tools_root/bin/${tool}.new" "$tools_root/bin/$tool"
    record_installed_version "$tool" "$version"

    rm -rf -- "$workdir"
    trap - 0 INT TERM HUP
}

probe_cargo_dylint() {
    "$@" dylint --version 2>/dev/null | awk '{print $2}' || true
}

# Provision dylint-link, whose version cannot be probed from the binary.
# Ordered most to least trustworthy: a marked isolated copy is reused, an
# unmarked or stale isolated copy is replaced, and an unattributable system
# copy is accepted only when this script owns nothing.
provision_dylint_link() {
    version=$1
    if [ -x "$tools_root/bin/dylint-link" ]; then
        if [ "$(recorded_version dylint-link)" = "$version" ]; then
            return 0
        fi
        echo "install-dylint-tools: replacing dylint-link in $tools_root/bin;" \
            "it is not recorded as version $version" >&2
        install_tool dylint-link "$version"
        return 0
    fi
    if command -v dylint-link >/dev/null 2>&1; then
        echo "install-dylint-tools: using the system dylint-link;" \
            "its version cannot be probed and is not verified" >&2
        return 0
    fi
    install_tool dylint-link "$version"
}

installed_cargo_dylint=$(probe_cargo_dylint cargo-dylint)
if [ "$installed_cargo_dylint" != "$cargo_dylint_version" ]; then
    install_tool cargo-dylint "$cargo_dylint_version"
    installed_cargo_dylint=$(probe_cargo_dylint "$tools_root/bin/cargo-dylint")
    if [ "$installed_cargo_dylint" != "$cargo_dylint_version" ]; then
        fail "installed cargo-dylint reports '$installed_cargo_dylint', expected '$cargo_dylint_version'"
    fi
fi

provision_dylint_link "$dylint_link_version"
