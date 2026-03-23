#!/usr/bin/env bash
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
VERUS_CACHE_DIR=${WHITAKER_VERUS_CACHE_DIR:-"${XDG_CACHE_HOME:-${HOME}/.cache}/whitaker/verus"}
VERUS_RELEASE_VERSION=${VERUS_RELEASE_VERSION:-0.2026.03.17.a96bad0}
VERUS_RELEASE_TAG=${VERUS_RELEASE_TAG:-release/0.2026.03.17.a96bad0}

tmp_dir=""
toolchain_log=""

cleanup() {
    [ -n "${tmp_dir}" ] && rm -rf "${tmp_dir}"
    [ -n "${toolchain_log}" ] && rm -f "${toolchain_log}"
}

trap cleanup EXIT INT TERM HUP

platform_asset_suffix() {
    case "$(uname -s):$(uname -m)" in
        Linux:x86_64)
            printf '%s\n' 'x86-linux'
            ;;
        Darwin:arm64)
            printf '%s\n' 'arm64-macos'
            ;;
        Darwin:x86_64)
            printf '%s\n' 'x86-macos'
            ;;
        MINGW*:x86_64 | MSYS*:x86_64 | CYGWIN*:x86_64)
            printf '%s\n' 'x86-win'
            ;;
        *)
            printf 'unsupported host for Verus release %s: %s %s\n' \
                "${VERUS_RELEASE_VERSION}" "$(uname -s)" "$(uname -m)" >&2
            exit 1
            ;;
    esac
}

asset_suffix=$(platform_asset_suffix)
asset_name="verus-${VERUS_RELEASE_VERSION}-${asset_suffix}.zip"
install_root="${VERUS_CACHE_DIR}/${VERUS_RELEASE_VERSION}"
install_dir="${install_root}/verus-${asset_suffix}"
verus_bin="${install_dir}/verus"

if [ ! -x "${verus_bin}" ]; then
    tmp_dir=$(mktemp -d)
    mkdir -p "${install_root}"
    curl -fsSL \
        -o "${tmp_dir}/${asset_name}" \
        "https://github.com/verus-lang/verus/releases/download/${VERUS_RELEASE_TAG}/${asset_name}"
    python3 - "${tmp_dir}/${asset_name}" "${install_root}" <<'PY'
import sys
import zipfile

archive_path, destination = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(archive_path) as archive:
    archive.extractall(destination)
PY
    chmod +x \
        "${verus_bin}" \
        "${install_dir}/cargo-verus" \
        "${install_dir}/rust_verify" \
        "${install_dir}/z3"
fi

toolchain_log=$(mktemp)
if ! "${verus_bin}" --version >"${toolchain_log}" 2>&1; then
    required_toolchain=$(python3 - "${toolchain_log}" <<'PY'
import pathlib
import re
import sys

contents = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
match = re.search(r"rustup install ([^\s]+)", contents)
if match is not None:
    print(match.group(1))
PY
)
    if [ -z "${required_toolchain}" ]; then
        cat "${toolchain_log}" >&2
        exit 1
    fi
    rustup toolchain install "${required_toolchain}"
    "${verus_bin}" --version >/dev/null
fi

printf '%s\n' "${install_dir}"
