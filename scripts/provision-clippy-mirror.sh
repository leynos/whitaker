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
# MIRROR_DIR must be `rust-clippy.git` directly inside the trusted cache
# root (`CLIPPY_MIRROR_ROOT`, defaulting to `~/.cache/whitaker-mirrors`).
# The root is the directory the cache step owns and the mirror is a child of
# it, never the root itself: a cold volume materializes the root as an empty
# directory, and discarding a stale generation must not remove the root. The
# argument is resolved to a physical path and compared with the resolved
# root, so a path that merely ends in `rust-clippy.git` is refused and a
# failed clone can never discard an unrelated directory.
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
MIRROR_BASENAME='rust-clippy.git'
CLONE_ATTEMPTS=3
CLONE_BACKOFF_SECONDS=5

# Inspection outcomes. `REBUILD` means "no usable mirror here, clone one";
# `ENVIRONMENT` means "the machine is wrong, not the cache generation", and
# is never repaired by discarding the path.
MIRROR_USABLE=0
MIRROR_REBUILD=1
MIRROR_ENVIRONMENT=2

if [[ "$#" -ne 1 ]]; then
    echo "usage: $0 MIRROR_DIR" >&2
    exit 2
fi

mirror=$1
mirror_root=${CLIPPY_MIRROR_ROOT:-${HOME}/.cache/whitaker-mirrors}

fail() {
    echo "provision-clippy-mirror: $*" >&2
    exit 1
}

# Resolve a directory to its physical path without requiring it to exist:
# the deepest existing ancestor is resolved and the remaining components are
# appended. A cold cache root does not exist yet, so `realpath -e` would
# reject the very case this guard has to police.
resolve_path() {
    local target=$1 suffix=''
    target=${target%/}
    [[ -n "${target}" ]] || target=/
    while [[ ! -d "${target}" ]]; do
        case "${target}" in
            */?*) ;;
            *) fail "cannot resolve ${1}" ;;
        esac
        suffix="/${target##*/}${suffix}"
        target=${target%/*}
        [[ -n "${target}" ]] || target=/
    done
    printf '%s%s\n' "$(cd "${target}" && pwd -P)" "${suffix}"
}

# Refuse to manage anything but the one mirror directory this script owns,
# so a mistyped argument can never discard a cache root or a home directory.
# Suffix matching is not enough: `/tmp/project/rust-clippy.git` ends in the
# expected name but belongs to nobody.
resolved_root=$(resolve_path "${mirror_root}")
resolved_mirror=$(resolve_path "${mirror}")
if [[ "${resolved_mirror}" != "${resolved_root}/${MIRROR_BASENAME}" ]]; then
    fail "refusing to manage ${mirror};" \
        "expected ${resolved_root}/${MIRROR_BASENAME}"
fi
mirror=${resolved_mirror}

# A restored cache generation, an empty mount point, and a half-written
# clone are all directories, so existence proves nothing. Classify the path
# rather than reducing it to one boolean: a bare repository pointed at the
# wrong upstream is a stale generation to replace, while an unreadable path
# is a machine fault the job must not paper over by deleting it.
inspect_mirror() {
    local candidate=$1 bare origin
    [[ -e "${candidate}" ]] || return "${MIRROR_REBUILD}"
    if [[ ! -d "${candidate}" ]]; then
        echo "${candidate} exists but is not a directory." >&2
        return "${MIRROR_ENVIRONMENT}"
    fi
    if [[ ! -r "${candidate}" || ! -x "${candidate}" ]]; then
        echo "${candidate} is not readable and searchable." >&2
        return "${MIRROR_ENVIRONMENT}"
    fi
    bare=$(git -C "${candidate}" rev-parse --is-bare-repository 2>/dev/null) ||
        return "${MIRROR_REBUILD}"
    case "${bare}" in
        true) ;;
        false) return "${MIRROR_REBUILD}" ;;
        *)
            echo "git reported an unparseable bare-repository state" \
                "'${bare}' for ${candidate}." >&2
            return "${MIRROR_ENVIRONMENT}"
            ;;
    esac
    origin=$(git -C "${candidate}" config --get remote.origin.url 2>/dev/null) ||
        origin=''
    case "${origin}" in
        "${CLIPPY_URL}" | "${CLIPPY_URL}.git") return "${MIRROR_USABLE}" ;;
        *) return "${MIRROR_REBUILD}" ;;
    esac
}

emit_hit() {
    if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
        printf 'clippy-mirror-hit=%s\n' "$1" >>"${GITHUB_OUTPUT}"
    fi
}

# Only ever reached for a path the startup guard proved is the cache-owned
# mirror, so the removal is bounded to this script's own generation.
discard_stale_mirror() {
    [[ -e "${mirror}" ]] || return 0
    rm -rf -- "${mirror}"
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

inspection=0
inspect_mirror "${mirror}" || inspection=$?
case "${inspection}" in
    "${MIRROR_USABLE}")
        echo "Reusing the cached Clippy mirror at ${mirror}"
        emit_hit true
        if ! git -C "${mirror}" remote update --prune; then
            echo "::warning::Could not refresh the cached Clippy mirror;" \
                "continuing with the restored generation."
        fi
        ;;
    "${MIRROR_REBUILD}")
        echo "No usable Clippy mirror at ${mirror}; cloning it once."
        emit_hit false
        mkdir -p "$(dirname "${mirror}")"
        clone_mirror
        ;;
    *)
        fail "cannot inspect ${mirror}; refusing to discard it."
        ;;
esac

# Rewrite both spellings the upstream build script could use. `insteadOf`
# is additive, so drop any previous values before setting ours to keep
# repeated invocations idempotent.
git config --global --unset-all "url.${mirror}.insteadOf" 2>/dev/null || true
git config --global --add "url.${mirror}.insteadOf" "${CLIPPY_URL}"
git config --global --add "url.${mirror}.insteadOf" "${CLIPPY_URL}.git"

echo "Clippy clones now resolve to ${mirror}"
