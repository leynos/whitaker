#!/usr/bin/env bash
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)
VERUS_INSTALL_DIR=$("${SCRIPT_DIR}/install-verus.sh")
VERUS_BIN="${VERUS_INSTALL_DIR}/verus"

proof_files_for_group() {
    case "$1" in
        decomposition)
            printf '%s\n' \
                "${REPO_ROOT}/verus/decomposition_cosine_threshold.rs" \
                "${REPO_ROOT}/verus/decomposition_vector_algebra.rs"
            ;;
        clone-detector)
            printf '%s\n' \
                "${REPO_ROOT}/verus/clone_detector_lsh_config.rs"
            ;;
        all)
            printf '%s\n' \
                "${REPO_ROOT}/verus/decomposition_cosine_threshold.rs" \
                "${REPO_ROOT}/verus/decomposition_vector_algebra.rs" \
                "${REPO_ROOT}/verus/clone_detector_lsh_config.rs"
            ;;
        *)
            printf 'unknown Verus proof group: %s\n' "$1" >&2
            exit 1
            ;;
    esac
}

set_proof_files_for_group() {
    local group proof_file proof_files_output
    group=$1
    proof_files_output=$(proof_files_for_group "${group}")
    PROOF_FILES=()
    while IFS= read -r proof_file; do
        [ -n "${proof_file}" ] || continue
        PROOF_FILES+=("${proof_file}")
    done <<EOF
${proof_files_output}
EOF
}

set_proof_files_for_group all

if [ $# -gt 0 ] && [ "${1:0:1}" != "-" ]; then
    case "$1" in
        all|decomposition|clone-detector)
            set_proof_files_for_group "$1"
            shift
            ;;
        *)
            PROOF_FILES=("$1")
            shift
            ;;
    esac
fi

for proof_file in "${PROOF_FILES[@]}"; do
    if ! "${VERUS_BIN}" "${proof_file}" "$@"; then
        printf 'Verus proof failed: %s\n' "${proof_file}" >&2
        exit 1
    fi
done
