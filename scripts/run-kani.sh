#!/usr/bin/env bash
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)
KANI_INSTALL_DIR=$("${SCRIPT_DIR}/install-kani.sh")
CARGO_KANI_BIN="${KANI_INSTALL_DIR}/bin/cargo-kani"

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
export RUSTUP_TOOLCHAIN=$(cat "${KANI_INSTALL_DIR}/rust-toolchain-version")
# goto-cc invokes the C preprocessor (gcc) via execvp.
export CC="${CC:-gcc}"

HARNESS_FILTER="${1:-}"

cd "${REPO_ROOT}/common"

if [ -n "${HARNESS_FILTER}" ]; then
    "${CARGO_KANI_BIN}" kani --harness "${HARNESS_FILTER}"
else
    # Run all harnesses (no --harness filter)
    "${CARGO_KANI_BIN}" kani
fi
