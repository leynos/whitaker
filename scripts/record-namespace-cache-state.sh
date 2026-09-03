#!/usr/bin/env bash
# record-namespace-cache-state.sh — Summarize what the Namespace volume
# actually restored, so every cache miss can be explained from run evidence.
#
# Usage:
#   scripts/record-namespace-cache-state.sh CACHE_TAG PATH [PATH ...]
#
# Reads `NAMESPACE_CACHE_HIT` from the environment: the `cache-hit` output of
# `namespacelabs/nscloud-cache-action`. That output is NOT the volume-hit
# signal. It is false whenever any single requested path is absent, so a
# genuinely warm volume that predates a newly added path still reports
# false. Run 33704058579 recorded `cache-hit: false` while `nsc instance
# report` recorded `cache_volume_hit: true` for the same job, and that run
# reached an 82% sccache hit rate. Label the output for what it measures and
# treat `nsc instance report`'s `cache_volume_hit` column as authoritative.
#
# Per-path sizes are the useful local signal. The cache action mounts every
# requested path, so a cold volume leaves the mount point present but empty;
# presence alone therefore proves nothing and only the restored size
# distinguishes an empty generation from a warm one.
set -euo pipefail

# A mounted but empty directory costs a few kilobytes. Treat anything below
# this as "not restored" so an empty mount cannot masquerade as a warm path.
RESTORED_THRESHOLD_BYTES=$((1024 * 1024))

if [[ "$#" -lt 2 ]]; then
    echo "usage: $0 CACHE_TAG PATH [PATH ...]" >&2
    exit 2
fi

cache_tag=$1
shift

summary=${GITHUB_STEP_SUMMARY:-/dev/stdout}

# `du` fails on an absent path rather than reporting zero, so normalize a
# missing path to zero bytes and let the caller describe it.
path_bytes() {
    if [[ -e "$1" ]]; then
        du -sb "$1" 2>/dev/null | cut -f1
    else
        echo 0
    fi
}

human_bytes() { numfmt --to=iec --suffix=B "$1"; }

restored=0
total=0
# The Markdown emitted below uses backticks for code spans, not command
# substitution, so single quotes are deliberate.
# shellcheck disable=SC2016
{
    printf '### Cache observations\n\n'
    printf -- '- Cache tag: `%s`\n' "${cache_tag}"
    printf -- '- All mounted paths present (`cache-hit`): `%s`\n' \
        "${NAMESPACE_CACHE_HIT:-unset}"
    printf -- '- Authoritative volume hit: the `cache_volume_hit` column of\n'
    printf -- '  `nsc instance report` for this run. The cache action does not\n'
    printf -- '  expose it, and its `cache-hit` output is false whenever any\n'
    printf -- '  single listed path is absent.\n'
    printf -- '- Archive restore/save: not applicable; Namespace exposes the\n'
    printf -- '  volume locally.\n\n'
    printf '| Cached path | Restored |\n'
    printf '| --- | --- |\n'
} >>"${summary}"

for path in "$@"; do
    expanded="${path/#\~/${HOME}}"
    bytes=$(path_bytes "${expanded}")
    total=$((total + 1))
    # shellcheck disable=SC2016  # Backticks are Markdown code spans.
    if ((bytes >= RESTORED_THRESHOLD_BYTES)); then
        restored=$((restored + 1))
        printf '| `%s` | %s |\n' "${path}" "$(human_bytes "${bytes}")" >>"${summary}"
    else
        printf '| `%s` | empty |\n' "${path}" >>"${summary}"
    fi
done

# shellcheck disable=SC2016  # Backticks are Markdown code spans.
printf '\n- Paths restored: `%d` of `%d`\n' "${restored}" "${total}" >>"${summary}"

if ((restored == 0)); then
    echo "::warning::The Namespace volume for ${cache_tag} restored nothing;" \
        "this run rebuilds every cached artefact from cold."
fi
