#!/usr/bin/env bash
# verify-installed-bootstrap.sh — Prove an installed Whitaker lints without
# public Clippy.
#
# Usage:
#   scripts/verify-installed-bootstrap.sh WORK_DIR
#
# Run after `whitaker-installer` has installed the suite, the Dylint tools,
# and the Clippy snapshot. The script then denies `github.com`, confirms the
# denial with a `git ls-remote` canary, and checks the installed bootstrap
# end to end against a fixture crate that deliberately calls `std::fs`:
#
#   - a cold run with an empty driver directory builds the driver and fails
#     with a `no_std_fs_operations` diagnostic;
#   - a warm run reuses the driver byte for byte and fails the same way;
#   - two simultaneous cold runs both report the diagnostic;
#   - on Linux, a relocated data root still works;
#   - a deleted snapshot generation fails clearly before any lint runs.
#
# A crash, a dependency-download failure, or a missing driver never counts as
# lint enforcement: every lint failure must name the diagnostic.
set -euo pipefail

if [[ "$#" -ne 1 ]]; then
    echo "usage: $0 WORK_DIR" >&2
    exit 2
fi
work=$1
mkdir -p "${work}"
work=$(cd "${work}" && pwd -P)
diagnostic=no_std_fs_operations

fail() {
    echo "verify-installed-bootstrap: $*" >&2
    exit 1
}

# The dry run names the generation and driver directories, so this script
# never hard-codes a platform's data-directory convention.
dry_run_value() {
    whitaker-installer --dry-run --skip-deps 2>&1 |
        sed -n "s/^$1: //p" | head -n 1
}

deny_github() {
    if git ls-remote https://github.com/rust-lang/rust-clippy HEAD >/dev/null 2>&1; then
        echo "0.0.0.0 github.com" | sudo tee -a /etc/hosts >/dev/null
    fi
    if git ls-remote https://github.com/rust-lang/rust-clippy HEAD >/dev/null 2>&1; then
        fail "the github.com canary still succeeds; the denial did not take effect"
    fi
    echo "github.com is unreachable; public Clippy cannot be read."
}

make_fixture() {
    local fixture="${work}/fixture"
    rm -rf -- "${fixture}"
    mkdir -p "${fixture}/src"
    cat >"${fixture}/Cargo.toml" <<'TOML'
[package]
name = "whitaker_bootstrap_fixture"
version = "0.1.0"
edition = "2021"

[workspace]
TOML
    cat >"${fixture}/src/lib.rs" <<'RUST'
//! Deliberately bypasses the capability-based filesystem policy.

/// Read a file through ambient `std::fs`, which Whitaker forbids.
pub fn read_config() -> String {
    std::fs::read_to_string("config.toml").unwrap_or_default()
}
RUST
    echo "${fixture}"
}

# Run `whitaker --all` in the fixture, expecting a lint failure naming the
# diagnostic. Output goes to `$2` for inspection.
expect_diagnostic() {
    local fixture=$1 log=$2 status=0
    (cd "${fixture}" && RUSTFLAGS="-D warnings" whitaker --all) >"${log}" 2>&1 || status=$?
    [[ "${status}" -ne 0 ]] || fail "whitaker succeeded on a std::fs violation; see ${log}"
    grep -q "${diagnostic}" "${log}" ||
        fail "whitaker failed (${status}) without a ${diagnostic} diagnostic; see ${log}"
}

driver_digest() {
    local driver_dir=$1 driver
    driver=$(find "${driver_dir}" -type f \( -name dylint-driver -o -name dylint-driver.exe \) | head -n 1)
    [[ -n "${driver}" ]] || fail "no driver was built under ${driver_dir}"
    if command -v sha256sum >/dev/null; then
        sha256sum "${driver}" | cut -d ' ' -f 1
    else
        shasum -a 256 "${driver}" | cut -d ' ' -f 1
    fi
}

snapshot=$(dry_run_value "Clippy snapshot")
driver_dir=$(dry_run_value "Dylint driver directory")
[[ -n "${snapshot}" && -d "${snapshot}" ]] || fail "the installed snapshot '${snapshot}' is missing"
[[ -n "${driver_dir}" ]] || fail "the dry run did not name a driver directory"

deny_github
fixture=$(make_fixture)

echo "Cold run with an empty driver directory."
rm -rf -- "${driver_dir}"
expect_diagnostic "${fixture}" "${work}/cold.log"
cold=$(driver_digest "${driver_dir}")

echo "Warm run reusing the driver."
expect_diagnostic "${fixture}" "${work}/warm.log"
[[ "$(driver_digest "${driver_dir}")" == "${cold}" ]] || fail "the warm run rebuilt the driver"

echo "Two simultaneous cold runs."
rm -rf -- "${driver_dir}"
expect_diagnostic "${fixture}" "${work}/concurrent-a.log" &
first=$!
expect_diagnostic "${fixture}" "${work}/concurrent-b.log" &
second=$!
wait "${first}" || fail "the first concurrent run did not report the diagnostic"
wait "${second}" || fail "the second concurrent run did not report the diagnostic"

if [[ "$(uname -s)" == Linux ]]; then
    echo "Relocated data root."
    data_root=${XDG_DATA_HOME:-${HOME}/.local/share}
    moved="${work}/moved-share"
    mv -- "${data_root}" "${moved}"
    status=0
    (XDG_DATA_HOME="${moved}" expect_diagnostic "${fixture}" "${work}/relocated.log") || status=$?
    mv -- "${moved}" "${data_root}"
    [[ "${status}" -eq 0 ]] || fail "the relocated installation did not lint"
fi

echo "Deleted snapshot generation."
mv -- "${snapshot}" "${snapshot}.removed"
status=0
(cd "${fixture}" && whitaker --all) >"${work}/deleted.log" 2>&1 || status=$?
mv -- "${snapshot}.removed" "${snapshot}"
[[ "${status}" -ne 0 ]] || fail "whitaker ran without its Clippy snapshot"
grep -q "never falls back to cloning public Clippy" "${work}/deleted.log" ||
    fail "a missing snapshot did not fail clearly; see ${work}/deleted.log"
if grep -q "${diagnostic}" "${work}/deleted.log"; then
    fail "lints ran although the snapshot was missing"
fi

echo "The installed bootstrap lints without public Clippy."
