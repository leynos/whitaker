#!/usr/bin/env bash
# Reject an incomplete forced rolling rebuild before the release is changed.
# Ordinary pushes retain the rolling release's partial-publication policy.
set -euo pipefail

if [[ "${GITHUB_EVENT_NAME:-}" != workflow_dispatch ||
      "${FORCE_DEPENDENCY_BINARY_REBUILD:-}" != true ]]; then
    exit 0
fi

if [[ "${LINT_BUILD_RESULT:-}" != success ||
      "${DEPENDENCY_BUILD_RESULT:-}" != success ]]; then
    echo "::error::A forced rolling rebuild requires both complete build matrices." >&2
    exit 1
fi

dist=${ROLLING_DIST_DIR:-dist}
short_sha=$(git rev-parse --short HEAD)
toolchain=$(awk -F '"' '/^channel/ {print $2}' rust-toolchain.toml)
if [[ -z "$toolchain" ]]; then
    echo "::error::Cannot determine the pinned lint toolchain." >&2
    exit 1
fi

require_asset() {
    if [[ ! -s "$1" ]]; then
        echo "::error::Forced rolling rebuild is missing $1." >&2
        return 1
    fi
}

manifest_rows=$(installer/scripts/dependency_binaries_manifest.py)
if [[ -z "$manifest_rows" ]]; then
    echo "::error::No dependency binaries were declared." >&2
    exit 1
fi

targets=(
    x86_64-unknown-linux-gnu
    aarch64-unknown-linux-gnu
    x86_64-apple-darwin
    aarch64-apple-darwin
    x86_64-pc-windows-msvc
)
for target in "${targets[@]}"; do
    require_asset "$dist/whitaker-lints-${short_sha}-${toolchain}-${target}.tar.zst"
    require_asset "$dist/manifest-${target}.json"

    extension=tgz
    if [[ "$target" == *windows* ]]; then
        extension=zip
    fi
    while IFS=$'\t' read -r package _binary version; do
        archive="$dist/${package}-${target}-v${version}.${extension}"
        checksum="${archive}.sha256"
        require_asset "$archive"
        require_asset "$checksum"
        # Compare the entire line. `sha256sum --check` follows the filename
        # inside a sidecar, which could name a different file than this archive.
        if [[ "$(cat "$checksum")" != "$(sha256sum "$archive")" ]]; then
            echo "::error::Checksum or filename does not match $archive." >&2
            exit 1
        fi
    done <<< "$manifest_rows"
done
