#!/usr/bin/env bash
# run-kani.sh — Run the pinned Kani bounded model checker against Whitaker's
# sidecar proof harnesses.
#
# Usage:
#   scripts/run-kani.sh
#   scripts/run-kani.sh clone-detector [EXTRA_KANI_FLAGS...]
#   scripts/run-kani.sh decomposition [HARNESS_FILTER]
#   scripts/run-kani.sh [HARNESS_FILTER]
#
# Arguments:
#   clone-detector  Run the explicit clone-detector harness list.
#   decomposition   Run the existing decomposition/common harnesses.
#   HARNESS_FILTER  When no group is given, treat the first positional
#                   argument as a decomposition/common harness filter.
#
# Installs Kani via install-kani.sh if not already cached, then configures
# PATH, RUSTUP_TOOLCHAIN, and the appropriate dynamic-library search path
# (DYLD_LIBRARY_PATH on macOS, LD_LIBRARY_PATH on Linux) before invoking
# cargo-kani.
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd -- "${SCRIPT_DIR}/.." && pwd)
KANI_INSTALL_DIR=$("${SCRIPT_DIR}/install-kani.sh")
CARGO_KANI_BIN="${KANI_INSTALL_DIR}/bin/cargo-kani"
CLONE_DETECTOR_MANIFEST="${REPO_ROOT}/crates/whitaker_clones_core/Cargo.toml"

# kani-compiler is dynamically linked against the toolchain's libLLVM.
# On macOS use DYLD_LIBRARY_PATH, on Linux use LD_LIBRARY_PATH.
if [ "$(uname -s)" = "Darwin" ]; then
    export DYLD_LIBRARY_PATH="${KANI_INSTALL_DIR}/toolchain/lib${DYLD_LIBRARY_PATH:+:${DYLD_LIBRARY_PATH}}"
else
    export LD_LIBRARY_PATH="${KANI_INSTALL_DIR}/toolchain/lib${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
fi
# CBMC tools (goto-cc, cbmc, goto-instrument) and the matching nightly
# toolchain (cargo, rustc) must be reachable.
export PATH="${KANI_INSTALL_DIR}/bin:${KANI_INSTALL_DIR}/toolchain/bin:${PATH}"
# Tell rustup to use the Kani-pinned toolchain.
TOOLCHAIN=$(cat "${KANI_INSTALL_DIR}/rust-toolchain-version")
export RUSTUP_TOOLCHAIN="${TOOLCHAIN}"
# goto-cc invokes the C preprocessor (gcc) via execvp.
export CC="${CC:-gcc}"

run_clone_detector_harnesses() {
    for harness in \
        verify_lsh_config_new_smoke \
        verify_lsh_config_new_symbolic \
        verify_lsh_config_new_overflow_product \
        verify_min_hasher_sketch_rejects_empty_input \
        verify_min_hasher_sketch_is_deterministic \
        verify_min_hasher_sketch_ignores_duplicate_hashes \
        verify_lsh_index_rejects_self_pairs \
        verify_lsh_index_canonicalizes_pair_order \
        verify_lsh_index_deduplicates_repeated_band_collisions \
        verify_lsh_index_is_insertion_order_independent \
        verify_smallest_covering_node_selects_minimal_range \
        verify_smallest_covering_root_fallback \
        verify_kind_index_is_bounded \
        verify_count_accumulation_is_order_independent_bounded
    do
        # Keep the parser feature off for every clone-detector harness, including
        # verify_lsh_index_rejects_self_pairs: Kani's pinned Rust cannot satisfy
        # ra_ap_syntax's MSRV.
        "${CARGO_KANI_BIN}" kani \
            --manifest-path "${CLONE_DETECTOR_MANIFEST}" \
            --no-default-features \
            --default-unwind 4 \
            --harness "${harness}" \
            "$@"
    done
}

run_decomposition_harnesses() {
    cd "${REPO_ROOT}/common"
    if [ $# -gt 0 ]; then
        "${CARGO_KANI_BIN}" kani --harness "$1"
    else
        "${CARGO_KANI_BIN}" kani
    fi
}

run_expected_counterexample() {
    harness=$1
    expected_witness=$2
    expected_failure=$3
    shift 3
    output_file=$(mktemp)
    if "${CARGO_KANI_BIN}" kani \
        --manifest-path "${REPO_ROOT}/Cargo.toml" \
        --package whitaker \
        --lib \
        --harness "${harness}" \
        "$@" >"${output_file}" 2>&1; then
        cat "${output_file}"
        rm -f "${output_file}"
        printf 'Kani reachability harness unexpectedly verified: %s\n' "${harness}" >&2
        return 1
    fi
    cat "${output_file}"
    if ! grep -Fq 'VERIFICATION:- FAILED' "${output_file}" \
        || ! grep -Fq "Failed Checks: \"${expected_failure}\"" "${output_file}" \
        || ! awk -v expected="${expected_witness}" '
            /^Check [0-9]+:/ {
                if (is_witness && status == "SUCCESS" && index(description, expected) > 0) {
                    found = 1
                }
                is_witness = ($0 ~ /assertion\.1$/)
                status = ""
                description = ""
            }
            /- Status:/ && is_witness { status = $3 }
            /- Description:/ && is_witness { description = $0 }
            END {
                if (is_witness && status == "SUCCESS" && index(description, expected) > 0) {
                    found = 1
                }
                exit !found
            }
        ' "${output_file}"; then
        rm -f "${output_file}"
        printf 'Kani did not report the expected witness and named failure: %s\n' \
            "${harness}" >&2
        return 1
    fi
    rm -f "${output_file}"
}

run_whitaker_cli_harness() {
    harness=$1
    printf 'Running Whitaker CLI Kani harness: %s\n' "${harness}"
    "${CARGO_KANI_BIN}" kani \
        --manifest-path "${REPO_ROOT}/Cargo.toml" \
        --package whitaker \
        --lib \
        --harness "${harness}"
}

run_whitaker_cli_harnesses() {
    run_whitaker_cli_harness verify_classify_is_total_and_disjoint
    run_expected_counterexample \
        verify_subcommand_is_reachable \
        'assertion failed: classify(&[TokenTag::Install]) == Class::Subcommand(Subcommand::Install)' \
        'subcommand class should have a counterexample'
    run_expected_counterexample \
        verify_forward_is_reachable \
        'assertion failed: classify(&[TokenTag::DylintFlag]) == Class::Forward' \
        'forward class should have a counterexample'
    run_expected_counterexample \
        verify_display_is_reachable \
        'assertion failed: classify(&[TokenTag::Help]) == Class::Display' \
        'display class should have a counterexample'
}

if [ $# -eq 0 ]; then
    run_decomposition_harnesses
    run_clone_detector_harnesses
    run_whitaker_cli_harnesses
    exit 0
fi

case "$1" in
    common|decomposition)
        shift
        run_decomposition_harnesses "$@"
        ;;
    clone-detector)
        shift
        run_clone_detector_harnesses "$@"
        ;;
    whitaker-cli)
        shift
        run_whitaker_cli_harnesses "$@"
        ;;
    *)
        run_decomposition_harnesses "$@"
        ;;
esac
