#!/usr/bin/env bash
# run-kani.sh — Run the pinned Kani bounded model checker against Whitaker's
# sidecar proof harnesses.
#
# Usage:
#   scripts/run-kani.sh
#   scripts/run-kani.sh clone-detector [HARNESS_FILTER] [EXTRA_KANI_FLAGS...]
#   scripts/run-kani.sh clone-detector-harness HARNESS
#   scripts/run-kani.sh string-literals [EXTRA_KANI_FLAGS...]
#   scripts/run-kani.sh decomposition [HARNESS_FILTER]
#   scripts/run-kani.sh [HARNESS_FILTER]
#
# Arguments:
#   clone-detector  Run the explicit clone-detector harness list, optionally
#                   restricted to one harness name.
#   clone-detector-harness
#                   Run exactly one clone-detector harness.
#   string-literals Run bounded continuation-scanner harnesses.
#   decomposition   Run the decomposition/common harnesses. Without a filter
#                   this runs only the harnesses that reach a verdict; see
#                   run_decomposition_harnesses for why the rest are excluded.
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
STRING_LITERAL_MANIFEST="${REPO_ROOT}/crates/whitaker_string_literals/Cargo.toml"

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
        verify_missing_cover_returns_none \
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

run_string_literal_harnesses() {
    for harness in verify_scanner_matches_bounded_parity_reference; do
        "${KANI_LIMITED_RUNNER}" "${harness}" "${CARGO_KANI_BIN}" kani \
            --manifest-path "${STRING_LITERAL_MANIFEST}" \
            --default-unwind 9 \
            --harness "${harness}" \
            "$@"
    done
}

run_decomposition_harnesses() {
    cd "${REPO_ROOT}/common"
    if [ $# -gt 0 ]; then
        "${KANI_LIMITED_RUNNER}" "$1" "${CARGO_KANI_BIN}" kani --harness "$1"
        return
    fi
    # Only the zero-iteration harness produces a verdict. The other three enter
    # label propagation's `Vec`/iterator machinery, where CBMC spends the whole
    # budget without discharging the assertion: each was measured at a 300 s
    # timeout using 3.7-4.0 GiB of the 8 GiB cap, and the longest reached
    # 5.7 GiB before its 600 s cap. Raising the limit cannot help, because the
    # growth is unbounded, not merely slow.
    #
    # They remain runnable by name so the blocker stays reproducible, but they
    # are excluded from the group default rather than reported as a pass. A
    # timed-out proof is inconclusive; treating it as green would make this
    # lane claim coverage it does not have. See `docs/brain-trust-lints-design
    # .md` and roadmap item 6.4.6 for the open modelling work.
    local is_provable_harness=verify_propagate_labels_zero_iterations_keeps_initial_labels
    "${KANI_LIMITED_RUNNER}" "${is_provable_harness}" "${CARGO_KANI_BIN}" kani \
        --harness "${is_provable_harness}"

    report_inconclusive_harnesses
}

# Names the propagation harnesses the default run does not attempt.
#
# The default run exits 0 after the one harness that reaches a verdict, so a
# green `make kani` would otherwise read as "every propagation property is
# proved". It is not: three harnesses are excluded, and `make kani` succeeds
# without them by design. Printing them keeps that gap in the output the
# operator actually sees, next to the verdict it qualifies.
report_inconclusive_harnesses() {
    local omitted_harness
    for omitted_harness in \
        verify_propagate_labels_returns_vector_per_input \
        verify_propagate_labels_preserves_label_indices \
        verify_propagate_labels_bounded_return_for_any_max_iterations
    do
        printf 'INCONCLUSIVE: %s was not attempted by this run.\n' "${omitted_harness}"
        printf '  State explosion in label propagation'"'"'s Vec/iterator model; see\n'
        printf '  docs/brain-trust-lints-design.md (6.4.6). This property is\n'
        printf '  unproved, not proved.\n'
    done
    printf 'The default decomposition run proved 1 of 4 harnesses.\n'
    printf 'Run any omitted name as an argument to attempt it directly.\n'
}

if [ $# -eq 0 ]; then
    run_decomposition_harnesses
    run_clone_detector_harnesses
    run_string_literal_harnesses
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
    string-literals)
        shift
        run_string_literal_harnesses "$@"
        ;;
    *)
        run_decomposition_harnesses "$@"
        ;;
esac
