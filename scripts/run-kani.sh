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

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)
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
        verify_lsh_config_new_overflow_product
    do
        "${CARGO_KANI_BIN}" kani \
            --manifest-path "${CLONE_DETECTOR_MANIFEST}" \
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
    *)
        run_decomposition_harnesses "$@"
        ;;
esac
