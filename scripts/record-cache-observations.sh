#!/usr/bin/env bash
# record-cache-observations.sh — Make every cache restore explainable.
#
# Usage:
#   CARGO_REGISTRY_KEY=... CARGO_REGISTRY_MATCHED=... CARGO_REGISTRY_HIT=... \
#     scripts/record-cache-observations.sh
#
# Each restore step exports its rendered primary key, the key the action
# actually matched, and its `cache-hit` output under a fixed prefix. This
# script renders them into the job summary so a reviewer can explain every
# restore from the run evidence alone, without re-reading the workflow to
# work out which key a job asked for.
#
# `cache-hit` alone cannot classify a restore. The action reports `true`
# only for an exact primary-key match, so a successful `restore-keys`
# restore and a complete miss both surface as a falsy value. Every warm
# compiler-cache restore takes the prefix path, because that key ends with
# the current `github.run_id` and can never match exactly. The matched key
# is therefore the primary evidence: empty means a miss, equal to the
# primary key means an exact hit, and anything else names the generation
# the prefix restore actually loaded. The raw `cache-hit` value is printed
# verbatim alongside it, with an absent value shown as `unset` rather than
# coerced to `false`.
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

# Classify one restore from its primary and matched keys. Kept separate from
# rendering so the three outcomes can be read, and tested, on their own.
classify_restore() {
    local key=$1 matched=$2
    if [[ -z "${matched}" ]]; then
        printf 'miss\n'
    elif [[ "${matched}" == "${key}" ]]; then
        printf 'exact hit\n'
    else
        # shellcheck disable=SC2016  # The backticks are Markdown, not a subshell.
        printf 'prefix restore from `%s`\n' "${matched}"
    fi
}

emit_observation() {
    local label=$1 prefix=$2
    local key_name="${prefix}_KEY"
    local matched_name="${prefix}_MATCHED"
    local hit_name="${prefix}_HIT"
    local key=${!key_name:-}
    local matched=${!matched_name:-}
    local hit=${!hit_name:-}

    if [[ -z "${key}" ]]; then
        printf -- '- %s: inactive in this job\n' "${label}"
        return
    fi
    # shellcheck disable=SC2016  # The backticks are Markdown, not a subshell.
    printf -- '- %s: key `%s` %s (cache-hit `%s`)\n' \
        "${label}" "${key}" "$(classify_restore "${key}" "${matched}")" \
        "${hit:-unset}"
}

{
    printf '### Cache observations\n\n'
    # shellcheck disable=SC2016  # The backticks are Markdown, not a subshell.
    printf -- '- Compiler cache backend: `%s`\n' "${SCCACHE_BACKEND:-unset}"
    for entry in "${CACHE_STEPS[@]}"; do
        emit_observation "${entry%%:*}" "${entry##*:}"
    done
} >>"${summary}"
