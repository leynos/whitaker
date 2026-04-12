#!/usr/bin/env bash
set -euo pipefail

KANI_CACHE_DIR=${WHITAKER_KANI_CACHE_DIR:-"${XDG_CACHE_HOME:-${HOME}/.cache}/whitaker/kani"}
KANI_VERSION=${KANI_VERSION:-0.67.0}
KANI_RELEASE_TAG=${KANI_RELEASE_TAG:-kani-${KANI_VERSION}}

tmp_dir=""

cleanup() {
    [ -n "${tmp_dir}" ] && rm -rf "${tmp_dir}"
}

trap cleanup EXIT INT TERM HUP

platform_asset_suffix() {
    case "$(uname -s):$(uname -m)" in
        Linux:x86_64)
            printf '%s\n' 'x86_64-unknown-linux-gnu'
            ;;
        Linux:aarch64)
            printf '%s\n' 'aarch64-unknown-linux-gnu'
            ;;
        Darwin:arm64)
            printf '%s\n' 'aarch64-apple-darwin'
            ;;
        Darwin:x86_64)
            printf '%s\n' 'x86_64-apple-darwin'
            ;;
        *)
            printf 'unsupported host for Kani release %s: %s %s\n' \
                "${KANI_VERSION}" "$(uname -s)" "$(uname -m)" >&2
            exit 1
            ;;
    esac
}

asset_suffix=$(platform_asset_suffix)
asset_name="kani-${KANI_VERSION}-${asset_suffix}.tar.gz"
install_root="${KANI_CACHE_DIR}/${KANI_VERSION}"
install_dir="${install_root}/kani-${KANI_VERSION}"
kani_driver_bin="${install_dir}/bin/kani-driver"
cargo_kani_bin="${install_dir}/bin/cargo-kani"

if [ ! -x "${kani_driver_bin}" ]; then
    tmp_dir=$(mktemp -d)
    mkdir -p "${install_root}"
    curl -fsSL \
        --retry 3 \
        --connect-timeout 20 \
        --max-time 300 \
        -o "${tmp_dir}/${asset_name}" \
        "https://github.com/model-checking/kani/releases/download/${KANI_RELEASE_TAG}/${asset_name}"
    tar -xzf "${tmp_dir}/${asset_name}" -C "${install_root}"
    if [ ! -x "${kani_driver_bin}" ]; then
        printf 'expected executable Kani driver at %s after extracting %s\n' \
            "${kani_driver_bin}" "${asset_name}" >&2
        exit 1
    fi
fi

# Kani 0.67+ ships kani-driver but expects to be invoked as cargo-kani for
# cargo workspace mode.  Create the symlink when the tarball omits it.
if [ ! -e "${cargo_kani_bin}" ]; then
    ln -sf "${kani_driver_bin}" "${cargo_kani_bin}"
fi

# Kani expects a `toolchain` directory next to its binaries containing the
# matching nightly rustc/cargo.  Read the pinned version from the bundle and
# install it via rustup, then symlink it into place.
if [ ! -d "${install_dir}/toolchain" ]; then
    toolchain_tag=$(cat "${install_dir}/rust-toolchain-version")
    printf 'Installing Kani toolchain %s via rustup ...\n' "${toolchain_tag}" >&2
    rustup toolchain install "${toolchain_tag}" \
        --component rustc-dev,rust-src,rustfmt >&2
    # Extract the path from the last field, handling both default and non-default toolchains.
    # When a toolchain is the default, rustup inserts "(default)" before the path.
    toolchain_dir=$(rustup toolchain list -v \
        | grep "^${toolchain_tag} " \
        | awk '{print $NF}')
    if [ -z "${toolchain_dir}" ] || [ ! -d "${toolchain_dir}" ]; then
        printf 'failed to locate installed Kani toolchain directory for %s\n' "${toolchain_tag}" >&2
        exit 1
    fi
    ln -sf "${toolchain_dir}" "${install_dir}/toolchain"
fi

printf '%s\n' "${install_dir}"
