#!/usr/bin/env bash
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)
SIDECAR="${REPO_ROOT}/verus/clone_detector_candidate_pair.rs"
EXPECTED_PATH="../crates/whitaker_clones_core/src/index/fragment_id.rs"
EXPECTED_SYMBOL="FragmentId"
RUNTIME_FILE="${REPO_ROOT}/verus/${EXPECTED_PATH}"

if ! grep -F "#[path = \"${EXPECTED_PATH}\"]" "${SIDECAR}" >/dev/null; then
    printf 'missing expected Verus bridge path in %s: #[path = "%s"]\n' \
        "${SIDECAR}" "${EXPECTED_PATH}" >&2
    exit 1
fi

if ! grep -F "use fragment_id_runtime::${EXPECTED_SYMBOL};" "${SIDECAR}" >/dev/null; then
    printf 'missing expected Verus bridge symbol import in %s: %s\n' \
        "${SIDECAR}" "${EXPECTED_SYMBOL}" >&2
    exit 1
fi

if [ ! -f "${RUNTIME_FILE}" ]; then
    printf 'Verus bridge runtime file does not exist: %s\n' "${RUNTIME_FILE}" >&2
    exit 1
fi

if ! grep -Eq "(^|[[:space:]])pub[[:space:]]+struct[[:space:]]+${EXPECTED_SYMBOL}($|[[:space:]<(])" \
    "${RUNTIME_FILE}"; then
    printf 'Verus bridge runtime file does not declare expected type: %s\n' \
        "${EXPECTED_SYMBOL}" >&2
    exit 1
fi
