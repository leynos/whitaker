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
        archive_name="${package}-${target}-v${version}.${extension}"
        archive="$dist/$archive_name"
        checksum="${archive}.sha256"
        require_asset "$archive"
        require_asset "$checksum"
        mapfile -t checksum_lines < "$checksum"
        sidecar_line_count=${#checksum_lines[@]}
        if [[ "$sidecar_line_count" -ne 1 ]]; then
            echo "::error::Checksum or filename does not match $archive." >&2
            exit 1
        fi

        sidecar_digest=${checksum_lines[0]:0:64}
        sidecar_separator=${checksum_lines[0]:64:2}
        sidecar_path=${checksum_lines[0]:66}
        expected_sidecar_path="dist/$archive_name"
        actual_digest=$(sha256sum "$archive")
        actual_digest=${actual_digest%% *}
        sidecar_digest=${sidecar_digest,,}
        # The sidecar chooses only GNU's text or binary marker. Its path must
        # still name this release archive rather than another downloaded file.
        if [[ ! "$sidecar_digest" =~ ^[[:xdigit:]]{64}$ ||
              ( "$sidecar_separator" != '  ' && "$sidecar_separator" != ' *' ) ||
              "$sidecar_path" != "$expected_sidecar_path" ||
              "$sidecar_digest" != "$actual_digest" ]]; then
            echo "::error::Checksum or filename does not match $archive." >&2
            exit 1
        fi
    done <<< "$manifest_rows"
done
