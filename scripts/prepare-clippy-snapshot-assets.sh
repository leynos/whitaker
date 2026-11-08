#!/usr/bin/env bash
# prepare-clippy-snapshot-assets.sh — Decide and stage the Clippy snapshot
# assets a release needs, treating the bundle and its sidecar as one pair.
#
# Usage:
#   scripts/prepare-clippy-snapshot-assets.sh RELEASE DIST_DIR
#
# A bundle and its `.sha256` sidecar are only trustworthy together. Bundles
# are content-identical but not byte-identical across builds, so a sidecar
# computed from a rebuilt bundle must never sit beside a different published
# bundle: consumers would see a permanent checksum mismatch. Given the
# release's current assets, this script:
#
#   - does nothing when both are published (`state=published`);
#   - when only the bundle is published, an interrupted earlier publication,
#     downloads those published bytes, derives the sidecar from them, verifies
#     the bundle fully against the pinned manifest, and stages only the
#     sidecar (`state=repaired`);
#   - when only the sidecar is published, refuses: no bundle could match it
#     except the lost one, so an operator must delete the orphan first;
#   - when neither is published, builds the pair (`state=built`).
#
# The state is written to `GITHUB_OUTPUT` when it is set. Staged files land in
# DIST_DIR for the workflow to upload.
set -euo pipefail

if [[ "$#" -ne 2 ]]; then
    echo "usage: $0 RELEASE DIST_DIR" >&2
    exit 2
fi
release=$1
dist=$2
mkdir -p "${dist}"

commit=$(sed -n 's/^commit = .\([0-9a-f]*\).*/\1/p' installer/clippy-snapshot.toml)
[[ -n "${commit}" ]] || {
    echo "::error::could not read the snapshot commit from installer/clippy-snapshot.toml" >&2
    exit 1
}
asset="whitaker-clippy-snapshot-${commit}.bundle"

emit_state() {
    echo "Clippy snapshot ${asset} on ${release}: $1"
    if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
        printf 'state=%s\n' "$1" >>"${GITHUB_OUTPUT}"
    fi
}

names=$(gh release view "${release}" --json assets --jq '.assets[].name' 2>/dev/null || true)
has() { grep -qxF "$1" <<<"${names}"; }

if has "${asset}" && has "${asset}.sha256"; then
    emit_state published
elif has "${asset}.sha256"; then
    echo "::error::${release} publishes ${asset}.sha256 without ${asset}; no rebuilt" \
        "bundle can match it. Delete the orphan with" \
        "'gh release delete-asset ${release} ${asset}.sha256 --yes' and rerun." >&2
    exit 1
elif has "${asset}"; then
    work=$(mktemp -d)
    trap 'rm -rf -- "${work}"' EXIT
    gh release download "${release}" --pattern "${asset}" --dir "${work}"
    digest=$(sha256sum "${work}/${asset}" | cut -d ' ' -f 1)
    printf '%s  %s\n' "${digest}" "${asset}" >"${work}/${asset}.sha256"
    # Full verification of the published bytes before vouching for them.
    make clippy-snapshot CLIPPY_SNAPSHOT_BUNDLE="${work}/${asset}" \
        CLIPPY_SNAPSHOT_ROOT="${work}/root" DYLINT_CLIPPY_REPO="${work}/root/${commit}" \
        DYLINT_DRIVER_ROOT="${work}/drivers"
    cp -- "${work}/${asset}.sha256" "${dist}/${asset}.sha256"
    emit_state repaired
else
    make clippy-snapshot-bundle CLIPPY_SNAPSHOT_DIST="${dist}"
    emit_state built
fi
