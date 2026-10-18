#!/usr/bin/env bash
# Run one Kani invocation inside a measured, resource-limited cgroup scope.
#
# Usage:
#   scripts/run-kani-limited.sh HARNESS COMMAND [ARGUMENT...]
#
# The per-invocation cgroup is capped at 8 GiB with swap disabled, and GNU
# timeout stops the complete command process group after ten minutes. Limits
# may be lowered for a run through environment variables but never raised.
set -eu

DEFAULT_MEMORY_MAX_BYTES=8589934592
DEFAULT_TIMEOUT_SECONDS=600
MEMORY_MAX_BYTES=${WHITAKER_KANI_MEMORY_MAX_BYTES:-${DEFAULT_MEMORY_MAX_BYTES}}
TIMEOUT_SECONDS=${WHITAKER_KANI_TIMEOUT_SECONDS:-${DEFAULT_TIMEOUT_SECONDS}}

validate_limit() {
    local name=$1
    local value=$2
    local maximum=$3

    case "${value}" in
        ''|*[!0-9]*)
            printf '%s must be a positive integer, got %s\n' "${name}" "${value}" >&2
            exit 2
            ;;
    esac
    if [ "${value}" -eq 0 ] || [ "${value}" -gt "${maximum}" ]; then
        printf '%s must be between 1 and %s, got %s\n' \
            "${name}" "${maximum}" "${value}" >&2
        exit 2
    fi
}

if [ "$(uname -s)" != Linux ]; then
    printf 'Kani resource limits require Linux systemd cgroup scopes\n' >&2
    exit 2
fi
if [ "$#" -lt 2 ]; then
    printf 'usage: scripts/run-kani-limited.sh HARNESS COMMAND [ARGUMENT...]\n' >&2
    exit 2
fi

validate_limit WHITAKER_KANI_MEMORY_MAX_BYTES \
    "${MEMORY_MAX_BYTES}" "${DEFAULT_MEMORY_MAX_BYTES}"
validate_limit WHITAKER_KANI_TIMEOUT_SECONDS \
    "${TIMEOUT_SECONDS}" "${DEFAULT_TIMEOUT_SECONDS}"

for command_name in systemd-run systemctl timeout; do
    if ! command -v "${command_name}" >/dev/null 2>&1; then
        printf 'Kani requires %s to enforce process-tree resource limits\n' \
            "${command_name}" >&2
        exit 2
    fi
done
if [ ! -r /sys/fs/cgroup/cgroup.controllers ] \
    || ! grep -qw memory /sys/fs/cgroup/cgroup.controllers; then
    printf 'Kani requires the cgroup v2 memory controller for resource limits\n' >&2
    exit 2
fi

harness=$1
shift
unit_name="whitaker-kani-${$}-$(date +%s%N)"
started_at=$(date +%s)
peak_file=$(mktemp "${TMPDIR:-/tmp}/whitaker-kani-peak.XXXXXX")
trap 'rm -f "${peak_file}"' EXIT
# The child Bash must expand this program after it enters the scoped cgroup.
# Expanding it here would read the parent's cgroup and miss Kani's peak.
# shellcheck disable=SC2016
scope_command='
timeout_seconds=$1
peak_file=$2
shift 2
timeout --signal=TERM --kill-after=15s "${timeout_seconds}" "$@"
exit_status=$?
cgroup_path=$(awk -F: '\''$1 == "0" { print $3 }'\'' /proc/self/cgroup)
if [ -z "${cgroup_path}" ] || [ ! -r "/sys/fs/cgroup${cgroup_path}/memory.peak" ]; then
    printf "Kani cgroup peak memory is unavailable for %s\\n" "$unit_name" >&2
    exit 125
fi
cat "/sys/fs/cgroup${cgroup_path}/memory.peak" > "${peak_file}" || exit 125
exit "${exit_status}"
'
if systemd-run --user --scope --quiet --unit="${unit_name}" \
    --property="MemoryMax=${MEMORY_MAX_BYTES}" \
    --property=MemorySwapMax=0 -- \
    bash -c "${scope_command}" kani-scope \
    "${TIMEOUT_SECONDS}s" "${peak_file}" "$@"
then
    exit_status=0
else
    exit_status=$?
fi
elapsed_seconds=$(($(date +%s) - started_at))

scope_details=$(systemctl --user show "${unit_name}.scope" \
    --no-pager -p Result -p MemoryPeak 2>/dev/null || true)
scope_result=$(printf '%s\n' "${scope_details}" \
    | sed -n 's/^Result=//p')
scope_peak_bytes=$(printf '%s\n' "${scope_details}" \
    | sed -n 's/^MemoryPeak=//p')
peak_bytes=$(cat "${peak_file}")
if [ -z "${peak_bytes}" ] && [ -n "${scope_peak_bytes}" ]; then
    peak_bytes=${scope_peak_bytes}
fi
case "${peak_bytes}" in
    ''|*[!0-9]*)
        peak_bytes=unavailable
        ;;
esac

peak_mib=unavailable
if [ "${peak_bytes}" != unavailable ]; then
    peak_mib=$((peak_bytes / 1048576))
fi
printf 'Kani resources: harness=%s exit_status=%s duration_seconds=%s peak_memory_bytes=%s peak_memory_mib=%s memory_cap_bytes=%s timeout_seconds=%s scope_result=%s\n' \
    "${harness}" "${exit_status}" "${elapsed_seconds}" \
    "${peak_bytes}" "${peak_mib}" \
    "${MEMORY_MAX_BYTES}" "${TIMEOUT_SECONDS}" "${scope_result:-unknown}"
if [ "${scope_result}" = oom-kill ]; then
    printf 'Kani exceeded the process-tree memory cap for %s; proof is inconclusive\n' \
        "${harness}" >&2
    exit 1
fi
if [ "${peak_bytes}" = unavailable ]; then
    printf 'Kani could not report peak memory for %s; proof is inconclusive\n' \
        "${harness}" >&2
    exit 1
fi
if [ "${exit_status}" -eq 124 ]; then
    printf 'Kani timed out for %s; proof is inconclusive\n' "${harness}" >&2
    exit "${exit_status}"
fi
if [ "${exit_status}" -ne 0 ]; then
    printf 'Kani failed for %s (possibly a resource limit); no proof pass recorded\n' \
        "${harness}" >&2
    exit "${exit_status}"
fi
