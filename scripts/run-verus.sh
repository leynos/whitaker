#!/usr/bin/env bash
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)
VERUS_INSTALL_DIR=$("${SCRIPT_DIR}/install-verus.sh")
VERUS_BIN="${VERUS_INSTALL_DIR}/verus"
PROOF_FILES=(
    "${REPO_ROOT}/verus/decomposition_cosine_threshold.rs"
    "${REPO_ROOT}/verus/decomposition_vector_algebra.rs"
)

if [ $# -gt 0 ] && [ "${1:0:1}" != "-" ]; then
    PROOF_FILES=("$1")
    shift
fi

for proof_file in "${PROOF_FILES[@]}"; do
    if ! "${VERUS_BIN}" "${proof_file}" "$@"; then
        printf 'Verus proof failed: %s\n' "${proof_file}" >&2
        exit 1
    fi
done
