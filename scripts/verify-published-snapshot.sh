#!/usr/bin/env bash
# verify-published-snapshot.sh — Prove the published Clippy snapshot installs,
# and that a tampered copy cannot.
#
# Usage:
#   scripts/verify-published-snapshot.sh WORK_DIR
#
# Run from a checkout after the release is published. The script downloads
# the snapshot the checkout's manifest pins through
# `whitaker-clippy-snapshot install`, which fetches the bundle and its
# `.sha256` sidecar from the rolling release, checks one against the other,
# and fully verifies the resulting generation. It then flips one byte of a
# downloaded copy and requires installation to refuse it, with `github.com`
# already denied so a refusal cannot be mistaken for a network failure.
set -euo pipefail

if [[ "$#" -ne 1 ]]; then
    echo "usage: $0 WORK_DIR" >&2
    exit 2
fi
work=$1
mkdir -p "${work}"
work=$(cd "${work}" && pwd -P)

fail() {
    echo "verify-published-snapshot: $*" >&2
    exit 1
}

tool() {
    cargo run --locked --quiet -p whitaker-installer --bin whitaker-clippy-snapshot -- "$@"
}

commit=$(awk -F '"' '/^commit = / {print $2}' installer/clippy-snapshot.toml)
asset="whitaker-clippy-snapshot-${commit}.bundle"
base_url="https://github.com/${GITHUB_REPOSITORY:-leynos/whitaker}/releases/download/rolling"

echo "Installing the published ${asset}."
generation=$(tool install --root "${work}/published")
[[ "${generation}" == "${work}/published/${commit}" ]] ||
    fail "the published snapshot installed at ${generation}"
tool verify --repo "${generation}"

echo "Fetching a copy to tamper with."
curl --fail --location --silent --show-error --output "${work}/${asset}" "${base_url}/${asset}"
curl --fail --location --silent --show-error --output "${work}/${asset}.sha256" \
    "${base_url}/${asset}.sha256"
(cd "${work}" && sha256sum --check --strict "${asset}.sha256") ||
    fail "the published sidecar does not match the published bundle"

echo "0.0.0.0 github.com" | sudo tee -a /etc/hosts >/dev/null
if git ls-remote https://github.com/rust-lang/rust-clippy HEAD >/dev/null 2>&1; then
    fail "the github.com canary still succeeds; the denial did not take effect"
fi

# Invert one byte rather than overwrite it with a constant, which could leave
# the file unchanged when the bundle already holds that value there.
offset=4096
original=$(od -An -tu1 -j "${offset}" -N1 "${work}/${asset}" | tr -d ' ')
[[ -n "${original}" ]] || fail "the published bundle is shorter than ${offset} bytes"
printf '%b' "\\$(printf '%03o' $((255 - original)))" |
    dd of="${work}/${asset}" bs=1 seek="${offset}" conv=notrunc status=none
cmp -s "${work}/${asset}" <(curl --fail --location --silent "${base_url}/${asset}") &&
    fail "the tampered copy still matches the published bundle"
status=0
tool install --root "${work}/tampered" --bundle "${work}/${asset}" >"${work}/tampered.log" 2>&1 ||
    status=$?
[[ "${status}" -ne 0 ]] || fail "a tampered bundle was installed"
grep -q "failed SHA-256 verification" "${work}/tampered.log" ||
    fail "the tampered bundle was not rejected by its digest; see ${work}/tampered.log"
[[ ! -e "${work}/tampered/${commit}" ]] || fail "a rejected bundle left a generation behind"

echo "The published snapshot verifies, and a tampered copy is refused offline."
