#!/usr/bin/env bash
# record-cache-observations.sh — Make every cache restore explainable.
#
# Usage:
#   CARGO_REGISTRY_KEY=... CARGO_REGISTRY_HIT=... \
#     scripts/record-cache-observations.sh
#
# Each restore step exports its rendered primary key and its `cache-hit`
# output under a fixed prefix. This script renders them into the job summary
# so a reviewer can explain every miss from the run evidence alone, without
# re-reading the workflow to work out which key a job actually asked for.
#
# A step that a job does not use, such as the compiler-cache directory while
# the GitHub Actions backend is selected, exports an empty key and is reported
# as inactive rather than omitted: an absent line would be indistinguishable
# from a step that failed to run.
#
# Restore and save byte counts and durations are not available as step
# outputs. Read them from the cache action's own log lines in the job log.
set -euo pipefail

# Ordered so the summary reads from the coarsest cache to the finest.
CACHE_STEPS=(
    'Cargo registry and Git index:CARGO_REGISTRY'
    'Rust toolchain and installed tools:RUST_TOOLS'
    'Dylint host tools:DYLINT_TOOLS'
    'Clippy source mirror:CLIPPY_MIRROR'
    'Compiler cache directory:SCCACHE_DIR'
)

summary=${GITHUB_STEP_SUMMARY:-/dev/stdout}

emit_observation() {
    local label=$1 prefix=$2
    local key_name="${prefix}_KEY" hit_name="${prefix}_HIT"
    local key=${!key_name:-} hit=${!hit_name:-}

    if [[ -z "${key}" ]]; then
        printf -- '- %s: inactive in this job\n' "${label}"
        return
    fi
    # shellcheck disable=SC2016  # The backticks are Markdown, not a subshell.
    printf -- '- %s: key `%s` hit `%s`\n' "${label}" "${key}" "${hit:-false}"
}

{
    printf '### Cache observations\n\n'
    # shellcheck disable=SC2016  # The backticks are Markdown, not a subshell.
    printf -- '- Compiler cache backend: `%s`\n' "${SCCACHE_BACKEND:-unset}"
    for entry in "${CACHE_STEPS[@]}"; do
        emit_observation "${entry%%:*}" "${entry##*:}"
    done
} >>"${summary}"
