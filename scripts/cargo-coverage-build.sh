#!/usr/bin/env bash
# cargo-coverage-build.sh — a `cargo` stand-in that compiles the instrumented
# tests `make coverage` builds, and stops before running any of them.
#
# Usage (through the Makefile, which sets CARGO to this script):
#   make coverage-build
#
# `cargo llvm-cov nextest --no-run` means "report without running", so it
# cannot be used to compile only. This script instead takes the environment
# `cargo llvm-cov show-env` prints, under the RUSTFLAGS the calling recipe has
# already assigned, and runs `cargo nextest run --no-run` in the target
# directory the coverage run builds in. The compiler cache therefore holds the
# entries a later `make coverage` run reads. Any other cargo invocation passes
# through untouched.
set -euo pipefail

# An encoded flag set outranks RUSTFLAGS in cargo, so an inherited one would
# defeat the instrumentation show-env asks for.
unset CARGO_ENCODED_RUSTFLAGS

if [[ "${1:-}" == test ]]; then
  # The recipe also builds `dylint_testing`'s library tests; compile them only.
  exec cargo "$@" --no-run
fi
if [[ "${1:-}" != nextest ]]; then
  exec cargo "$@"
fi
shift

# show-env is read, never executed blind: only `export NAME=value` lines count.
env_lines="$(cargo llvm-cov show-env --export-prefix)"
while IFS= read -r line; do
  [[ "${line}" == export\ * ]] && eval "${line}"
done <<<"${env_lines}"
# The marker only tells cargo-llvm-cov it is being asked for its environment.
unset CARGO_LLVM_COV_SHOW_ENV

target_dir="${CARGO_LLVM_COV_TARGET_DIR:?CARGO_LLVM_COV_TARGET_DIR must name the coverage target directory}"
exec cargo nextest "$@" --target-dir "${target_dir}"
