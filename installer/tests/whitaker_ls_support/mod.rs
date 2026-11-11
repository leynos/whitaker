//! A strict fake `cargo` for the generated lister's end-to-end test.
//!
//! The shared bootstrap fixture installs a permissive stub that records every
//! invocation. That is the right collaborator for tests about recording, but it
//! cannot prove the lister passes only the supported arguments. This module
//! replaces it with a stub that rejects anything except the version probe and
//! `dylint list`, raising a distinctive status so the wrapper's argument vector
//! is part of the contract under test.

use std::error::Error;

use crate::bootstrap_support::{Installation, write_executable};

/// The strict stub's `cargo` implementation.
///
/// It accepts only the version probe and `dylint list`, recording the probed
/// arguments apart from the list invocation so a test can show the probe ran
/// without a list call. Any other vector fails with a distinctive status, so a
/// wrapper that forwards an unsupported flag is caught at this boundary rather
/// than passing a permissive stub. `dylint list` builds a driver on first use,
/// as Dylint does, and prints an unrelated suite and a suite name that merely
/// starts with `whitaker_suite` so the wrapper's filter is exercised.
const STUB_CARGO_STRICT: &str = r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "$#" -eq 2 && "$1" == dylint && "$2" == --version ]]; then
    printf 'ARG=%s\n' "$@" >>"${STUB_LOG}.probe"
    echo "cargo-dylint ${STUB_DYLINT_VERSION:-6.3.0}"
    exit 0
fi
if [[ "$#" -ne 2 || "$1" != dylint || "$2" != list ]]; then
    echo 'unexpected fake cargo invocation' >&2
    exit 97
fi
{
    printf 'LIB=%s\n' "${DYLINT_LIBRARY_PATH:-}"
    printf 'CLIPPY=%s\n' "${DYLINT_CLIPPY_REPO:-}"
    printf 'DRIVER=%s\n' "${DYLINT_DRIVER_PATH:-}"
    printf 'ARG=%s\n' "$@"
} >>"${STUB_LOG}.env"
driver_dir="${DYLINT_DRIVER_PATH}/${STUB_TOOLCHAIN}"
if [[ ! -s "${driver_dir}/dylint-driver" ]]; then
    mkdir -p "${driver_dir}"
    printf '#!/bin/sh\n' >"${driver_dir}/dylint-driver.partial"
    mv "${driver_dir}/dylint-driver.partial" "${driver_dir}/dylint-driver"
fi
printf 'whitaker_suite    nightly    /lib\nunrelated_suite    nightly    /lib\nwhitaker_suite_extra    nightly    /lib\n'
exit "${STUB_EXIT:-0}"
"#;

/// Replace the installation's permissive stub `cargo` with the strict stub.
///
/// # Errors
///
/// Returns an error if the replacement script cannot be written or made
/// executable.
pub fn use_strict_cargo(installation: &Installation) -> Result<(), Box<dyn Error>> {
    write_executable(&installation.base().join("bin/cargo"), STUB_CARGO_STRICT)
}
