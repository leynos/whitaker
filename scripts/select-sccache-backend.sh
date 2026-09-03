#!/usr/bin/env bash
# select-sccache-backend.sh — Route sccache at exactly one storage backend.
#
# Usage:
#   SCCACHE_BACKEND=gha|local scripts/select-sccache-backend.sh
#
# The workflow declares `SCCACHE_BACKEND` once, at workflow level, and every
# Linux lane calls this script before any Cargo invocation. Flipping that one
# declaration moves the whole repository between the GitHub Actions cache
# service and a cached directory owned by a `ubicloud/cache` step.
#
# The two backends are never combined. sccache silently prefers whichever
# backend it finds configured first, so a job that exported both would report
# a plausible hit rate while writing to a store nobody owns.
#
# `gha` needs no directory: the runner's Actions cache credentials are
# exported separately, because GitHub exposes them to actions rather than to
# `run` steps. `local` needs a bounded directory, because the archive grows
# with every new compilation unit until `SCCACHE_CACHE_SIZE` trims it.
set -euo pipefail

backend=${SCCACHE_BACKEND:-gha}
env_file=${GITHUB_ENV:-/dev/stdout}
local_cache_dir=${SCCACHE_LOCAL_DIR:-${HOME}/.cache/sccache}
local_cache_size=${SCCACHE_LOCAL_CACHE_SIZE:-2G}

case "${backend}" in
    gha)
        printf 'SCCACHE_GHA_ENABLED=true\n' >>"${env_file}"
        echo "sccache will use the GitHub Actions cache backend."
        ;;
    local)
        {
            printf 'SCCACHE_DIR=%s\n' "${local_cache_dir}"
            printf 'SCCACHE_CACHE_SIZE=%s\n' "${local_cache_size}"
        } >>"${env_file}"
        echo "sccache will use the local directory ${local_cache_dir}" \
            "capped at ${local_cache_size}."
        ;;
    *)
        echo "Unknown SCCACHE_BACKEND '${backend}'; expected 'gha' or 'local'." >&2
        exit 1
        ;;
esac
