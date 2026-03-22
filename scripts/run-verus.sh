#!/usr/bin/env bash
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)
VERUS_INSTALL_DIR=$("${SCRIPT_DIR}/install-verus.sh")
VERUS_BIN="${VERUS_INSTALL_DIR}/verus"
PROOF_FILE=${1:-"${REPO_ROOT}/verus/decomposition_cosine_threshold.rs"}

if [ $# -gt 0 ]; then
    shift
fi

"${VERUS_BIN}" "${PROOF_FILE}" "$@"
