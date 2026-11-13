#!/usr/bin/env bash
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd -- "${SCRIPT_DIR}/.." && pwd)
# The tool installs Verus (`make install-verus`, or the install-prover-tools
# action in CI) and runs it; this script only chooses the proof files. The
# default command is the Makefile's, for a run outside make.
PROVER_TOOLS=${PROVER_TOOLS:-"uv tool run --python 3.14 --from git+https://github.com/leynos/rust-prover-tools@$(awk '/^ref:/ { print $2; exit }' "${REPO_ROOT}/tools/rust-prover-tools/REF") prover-tools"}

proof_files_for_group() {
    case "$1" in
        decomposition)
            printf '%s\n' \
                "${REPO_ROOT}/verus/decomposition_cosine_threshold.rs" \
                "${REPO_ROOT}/verus/decomposition_vector_algebra.rs"
            ;;
        clone-detector)
            printf '%s\n' \
                "${REPO_ROOT}/verus/clone_detector_lsh_config.rs" \
                "${REPO_ROOT}/verus/clone_detector_candidate_pair.rs" \
                "${REPO_ROOT}/verus/clone_detector_ast_features.rs"
            ;;
        cli)
            printf '%s\n' "${REPO_ROOT}/verus/whitaker_cli_dispatch.rs"
            ;;
        string-literals)
            printf '%s\n' "${REPO_ROOT}/verus/string_continuation_rewrite.rs"
            ;;
        all)
            printf '%s\n' \
                "${REPO_ROOT}/verus/decomposition_cosine_threshold.rs" \
                "${REPO_ROOT}/verus/decomposition_vector_algebra.rs" \
                "${REPO_ROOT}/verus/clone_detector_lsh_config.rs" \
                "${REPO_ROOT}/verus/clone_detector_candidate_pair.rs" \
                "${REPO_ROOT}/verus/clone_detector_ast_features.rs" \
                "${REPO_ROOT}/verus/whitaker_cli_dispatch.rs" \
                "${REPO_ROOT}/verus/string_continuation_rewrite.rs"
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
        all|decomposition|clone-detector|cli|string-literals)
            set_proof_files_for_group "$1"
            shift
            ;;
        *)
            PROOF_FILES=("$1")
            shift
            ;;
    esac
fi

# Every remaining argument is a Verus flag, forwarded to each proof. Nothing is
# installed or downloaded here: a missing Verus fails with the install hint.
extra_args=()
for flag in "$@"; do
    extra_args+=("--extra-arg=${flag}")
done
proof_args=()
for proof_file in "${PROOF_FILES[@]}"; do
    proof_args+=(--proof-file "${proof_file}")
done
# shellcheck disable=SC2086 # PROVER_TOOLS is a command line, split on purpose.
if ! ${PROVER_TOOLS} verus run --repo-root "${REPO_ROOT}" --no-install-missing \
    "${proof_args[@]}" ${extra_args[@]+"${extra_args[@]}"}; then
    printf 'Verus proofs failed (install with make install-verus)\n' >&2
    exit 1
fi
