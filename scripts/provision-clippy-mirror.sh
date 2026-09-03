#!/usr/bin/env bash
# provision-clippy-mirror.sh — Give the Clippy source fetch one cache owner.
#
# Usage:
#   scripts/provision-clippy-mirror.sh MIRROR_DIR
#
# `dylint_driver`'s build script reads `clippy_utils/src/sym.rs` at the
# revision matching the pinned dated nightly. It obtains that file by
# running `git clone https://github.com/rust-lang/rust-clippy` into a
# temporary directory, so a cold build of `dylint_driver` performs a full
# clone of an unrelated upstream repository over the network. That fetch
# has no cache owner, costs minutes of paid runner time on every cold
# build, and fails outright when GitHub rejects the unauthenticated clone:
#
#   fatal: could not read Username for 'https://github.com'
#
# This script makes a durable bare mirror the single owner of that content
# and rewrites the upstream URL to it with `url.<mirror>.insteadOf`, so the
# build script's clone becomes a local, hard-linking copy.
#
# MIRROR_DIR must be a child of the cached directory, never the cache mount
# point itself: a cold volume materializes the mount point as a directory,
# and removing a stale generation must not attempt to remove a live mount.
#
# The script writes `clippy-mirror-hit=true|false` to `GITHUB_OUTPUT` when
# that variable is set, so the caller can record an observable hit or miss.
#
# A cold clone failure is fatal: without the mirror the build falls back to
# the unowned network clone this script exists to remove. A refresh failure
# on a warm mirror is only a warning, because the revision the build script
# needs is historical and is already present.
set -euo pipefail

CLIPPY_URL='https://github.com/rust-lang/rust-clippy'
CLONE_ATTEMPTS=3
CLONE_BACKOFF_SECONDS=5

if [[ "$#" -ne 1 ]]; then
    echo "usage: $0 MIRROR_DIR" >&2
    exit 2
fi

mirror=$1

# A restored cache generation and an empty mount point are both directories,
# so existence proves nothing. Ask git whether this is a usable mirror.
is_mirror() { git -C "$1" rev-parse --is-bare-repository >/dev/null 2>&1; }

emit_hit() {
    if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
        printf 'clippy-mirror-hit=%s\n' "$1" >>"${GITHUB_OUTPUT}"
    fi
}

# GitHub intermittently rejects unauthenticated clones from shared CI egress
# ranges, which surfaces as `could not read Username for 'https://github.com'`.
# That is rate limiting rather than a permission failure, so try anonymously
# first: the clone usually succeeds, and an anonymous request never sends the
# job's token to an unrelated upstream repository. Retry with the token when
# one is available, because an authenticated request is not subject to the
# anonymous limit, and back off between attempts rather than failing the job on
# a transient rejection.
clone_mirror() {
    local attempt
    local -a auth=()
    local token="${GH_TOKEN:-${GITHUB_TOKEN:-}}"
    for ((attempt = 1; attempt <= CLONE_ATTEMPTS; attempt++)); do
        if [[ "${attempt}" -gt 1 && -n "${token}" ]]; then
            auth=(-c "http.${CLIPPY_URL}.extraheader=Authorization: Bearer ${token}")
        else
            auth=()
        fi
        discard_stale_mirror
        if git "${auth[@]}" clone --mirror "${CLIPPY_URL}" "${mirror}"; then
            return 0
        fi
        echo "Clippy mirror clone attempt ${attempt} failed." >&2
        sleep "$((CLONE_BACKOFF_SECONDS * attempt))"
    done
    echo "Could not clone ${CLIPPY_URL} after ${CLONE_ATTEMPTS} attempts." >&2
    return 1
}

# Refuse to touch anything but the mirror directory this script owns, so a
# mistyped argument can never delete a cache mount point or a home directory.
discard_stale_mirror() {
    case "${mirror}" in
        */rust-clippy.git) ;;
        *)
            echo "Refusing to manage ${mirror}: expected a" \
                "'rust-clippy.git' directory." >&2
            return 1
            ;;
    esac
    if [[ -e "${mirror}" ]]; then
        find "${mirror}" -mindepth 1 -delete
        rmdir "${mirror}"
    fi
}

if is_mirror "${mirror}"; then
    echo "Reusing the cached Clippy mirror at ${mirror}"
    emit_hit true
    if ! git -C "${mirror}" remote update --prune; then
        echo "::warning::Could not refresh the cached Clippy mirror;" \
            "continuing with the restored generation."
    fi
else
    echo "No cached Clippy mirror at ${mirror}; cloning it once."
    emit_hit false
    mkdir -p "$(dirname "${mirror}")"
    clone_mirror
fi

# Rewrite both spellings the upstream build script could use. `insteadOf`
# is additive, so drop any previous values before setting ours to keep
# repeated invocations idempotent.
git config --global --unset-all "url.${mirror}.insteadOf" 2>/dev/null || true
git config --global --add "url.${mirror}.insteadOf" "${CLIPPY_URL}"
git config --global --add "url.${mirror}.insteadOf" "${CLIPPY_URL}.git"

echo "Clippy clones now resolve to ${mirror}"
