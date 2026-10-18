#!/usr/bin/env bash
# run-kani.sh — Run the pinned Kani bounded model checker against Whitaker's
# sidecar proof harnesses.
#
# Usage:
#   scripts/run-kani.sh
#   scripts/run-kani.sh clone-detector [HARNESS_FILTER] [EXTRA_KANI_FLAGS...]
#   scripts/run-kani.sh clone-detector-harness HARNESS
#   scripts/run-kani.sh decomposition [HARNESS_FILTER]
#   scripts/run-kani.sh [HARNESS_FILTER]
#
# Arguments:
#   clone-detector  Run the explicit clone-detector harness list, optionally
#                   restricted to one harness name.
#   clone-detector-harness
#                   Run exactly one clone-detector harness.
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
KANI_LIMITED_RUNNER="${SCRIPT_DIR}/run-kani-limited.sh"

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
    local harness_filter=""
    local found_filter=false

    if [ "$#" -gt 0 ] && [[ "$1" != -* ]]; then
        harness_filter=$1
        shift
    fi

    for harness in \
        verify_lsh_config_new_smoke \
        verify_lsh_config_new_symbolic \
        verify_lsh_config_new_overflow_product \
        verify_min_hasher_sketch_rejects_empty_input \
        verify_min_hasher_lane_minimum_for_boundary_hashes \
        verify_min_hasher_unique_hashes_sorts_and_deduplicates \
        verify_lsh_index_rejects_self_pairs \
        verify_lsh_index_canonicalizes_pair_order \
        verify_lsh_index_deduplicates_repeated_band_collisions \
        verify_lsh_index_is_insertion_order_independent \
        verify_smallest_covering_node_selects_minimal_range \
        verify_smallest_covering_root_fallback \
        verify_kind_id_round_trips_full_u16_domain \
        verify_kind_counts_stop_at_max_depth \
        verify_kind_id_equality_and_order_cover_small_alphabet
    do
        if [ -n "${harness_filter}" ] && [ "${harness}" != "${harness_filter}" ]; then
            continue
        fi
        found_filter=true
        # Keep the parser feature off for every clone-detector harness, including
        # verify_lsh_index_rejects_self_pairs: Kani's pinned Rust cannot satisfy
        # ra_ap_syntax's MSRV.
        "${KANI_LIMITED_RUNNER}" "${harness}" "${CARGO_KANI_BIN}" kani \
            --manifest-path "${CLONE_DETECTOR_MANIFEST}" \
            --no-default-features \
            --default-unwind 4 \
            --harness "${harness}" \
            "$@"
    done

    if [ -n "${harness_filter}" ] && [ "${found_filter}" != true ]; then
        printf 'unknown clone-detector Kani harness: %s\n' "${harness_filter}" >&2
        return 2
    fi
}

run_decomposition_harnesses() {
    cd "${REPO_ROOT}/common"
    if [ $# -gt 0 ]; then
        "${KANI_LIMITED_RUNNER}" "$1" "${CARGO_KANI_BIN}" kani --harness "$1"
    else
        "${KANI_LIMITED_RUNNER}" decomposition-all "${CARGO_KANI_BIN}" kani
    fi
}

if [ $# -eq 0 ]; then
    run_decomposition_harnesses
    run_clone_detector_harnesses
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
    clone-detector-harness)
        shift
        if [ "$#" -ne 1 ]; then
            printf 'usage: scripts/run-kani.sh clone-detector-harness HARNESS\n' >&2
            exit 2
        fi
        run_clone_detector_harnesses "$1"
        ;;
    *)
        run_decomposition_harnesses "$@"
        ;;
esac
