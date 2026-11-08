#!/usr/bin/env bash
# publish-rolling-release.sh — Republish the rolling release without ever
# leaving its tag short of a complete asset set.
#
# Usage:
#   RELEASE_SHA=<full> RELEASE_SHORT_SHA=<short> scripts/publish-rolling-release.sh
#
# The previous implementation ran `gh release delete rolling --cleanup-tag`
# and then `gh release create rolling <assets>`. Between those two commands the
# release, its assets and the `rolling` tag itself did not exist, so every
# consumer resolving that tag failed. The gap was six to seven seconds in
# practice, and a consumer landed in it: chutoro's `Install Whitaker` step
# began at 02:47:02 on 2026-09-05, the same second the delete began, could not
# fetch `cargo-dylint-x86_64-unknown-linux-gnu-v6.0.1.tgz`, and fell back to a
# source build of the Dylint tools.
#
# This script never deletes the release. New assets go up before old ones come
# down, so the published set is a superset of one generation or the other at
# every instant, rather than briefly empty.
#
# Ordering is deliberate and the reason is consistency, not speed:
#
#   1. Upload assets whose names are absent. The lint archives carry the commit
#      in their names, so this is where they land, with no window at all.
#   2. Replace the per-target manifests last among the uploads. A manifest
#      names the archive it describes, so replacing it after that archive
#      exists means a consumer reads either the old manifest and the old
#      archive, or the new manifest and the new archive. Never a manifest
#      pointing at something absent.
#   3. Delete superseded assets, last of all.
#
# The `rolling` tag is never moved. Installs resolve assets through the
# release by tag name, not by the commit the tag points at, and each manifest
# records the commit it was built from. Moving the tag also needed a token that
# may update a ref onto commits that touch `.github/workflows`, which
# `GITHUB_TOKEN` is not: the move failed with HTTP 403 and, running before the
# clean-up, left superseded archives behind. The tag is created once, with the
# release, and then left alone.
#
# `gh release upload --clobber` deletes before it uploads, by its own
# documentation, so it is used only where an asset is genuinely being replaced.
# When the dependency manifest is unchanged the dependency archives are not
# rebuilt, are already published, and are left untouched: that is the case that
# broke chutoro, and it now involves no delete at all.
set -euo pipefail

: "${RELEASE_SHA:?RELEASE_SHA must name the commit being published}"
: "${RELEASE_SHORT_SHA:?RELEASE_SHORT_SHA must be set for the release title}"
repository=${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}
tag=${ROLLING_TAG:-rolling}
asset_list=${RELEASE_ASSET_LIST:-dist/release-assets.txt}

[[ -f "${asset_list}" ]] || {
    echo "::error::${asset_list} is missing; nothing to publish." >&2
    exit 1
}

mapfile -t assets <"${asset_list}"
[[ "${#assets[@]}" -gt 0 ]] || {
    echo "::error::${asset_list} is empty; refusing to publish an empty set." >&2
    exit 1
}

title="Rolling Release (${RELEASE_SHORT_SHA})"
notes="Prebuilt lint libraries from commit ${RELEASE_SHA}."

# A release that does not exist yet cannot be updated in place, and there is no
# window to protect because there is nothing published to lose. `--target`
# names the commit a new tag is created at; an existing tag is left where it is.
if ! published=$(gh release view "${tag}" --json assets --jq '.assets[].name' 2>/dev/null); then
    echo "Rolling release absent; creating it with the full asset set."
    gh release create "${tag}" \
        --title "${title}" \
        --notes "${notes}" \
        --prerelease \
        --latest=false \
        --target "${RELEASE_SHA}" \
        "${assets[@]}"
    exit 0
fi

published_names=()
if [[ -n "${published}" ]]; then
    while IFS= read -r name; do
        [[ -n "${name}" ]] && published_names+=("${name}")
    done <<<"${published}"
fi

is_published() {
    local needle=$1 name
    for name in ${published_names+"${published_names[@]}"}; do
        [[ "${name}" == "${needle}" ]] && return 0
    done
    return 1
}

is_wanted() {
    local needle=$1 path
    for path in "${assets[@]}"; do
        [[ "$(basename -- "${path}")" == "${needle}" ]] && return 0
    done
    return 1
}

# Manifests are replaced rather than added, because their names are fixed. They
# are held back so that every archive a new manifest names is already
# published when the manifest appears.
new_assets=()
replacement_assets=()
manifest_assets=()
for path in "${assets[@]}"; do
    name=$(basename -- "${path}")
    case "${name}" in
        manifest-*.json) manifest_assets+=("${path}") ;;
        whitaker-clippy-snapshot-*)
            # Named by commit and content-identical whenever the name matches,
            # but not byte-identical across builds. Replacing a published one
            # would change the digest under an installer mid-download, so the
            # first upload stands.
            if is_published "${name}"; then
                echo "Keeping the published ${name}."
                continue
            fi
            # A bundle and its sidecar are one pair. A sidecar computed from a
            # rebuilt bundle must never be attached to a different published
            # bundle; only a sidecar derived from the published bytes (staged
            # without a bundle) may join it.
            case "${name}" in
                *.bundle.sha256)
                    if is_published "${name%.sha256}" && is_wanted "${name%.sha256}"; then
                        echo "::error::${name} belongs to a rebuilt bundle, but" \
                            "${name%.sha256} is already published; refusing to" \
                            "pair them." >&2
                        exit 1
                    fi
                    ;;
            esac
            new_assets+=("${path}")
            ;;
        *)
            if is_published "${name}"; then
                replacement_assets+=("${path}")
            else
                new_assets+=("${path}")
            fi
            ;;
    esac
done

if [[ "${#new_assets[@]}" -gt 0 ]]; then
    echo "Uploading ${#new_assets[@]} new asset(s)."
    gh release upload "${tag}" "${new_assets[@]}"
fi

# Reached only when this run rebuilt an asset that already exists under the
# same name, which today means the dependency archives after a manifest
# change. `--clobber` deletes first, so it is confined to that case.
#
# An archive and its `.sha256` must never disagree. Clobbering both together
# would publish the new archive while the old checksum was still up, and a
# consumer verifying the digest in that instant gets a mismatch, which is a
# hard failure rather than a retryable absence. So the old checksum is removed
# first: the states a consumer can observe are old archive with no checksum,
# new archive with no checksum, then both new. A missing checksum is something
# a caller can retry; a wrong one is not.
if [[ "${#replacement_assets[@]}" -gt 0 ]]; then
    echo "Replacing ${#replacement_assets[@]} rebuilt asset(s)."
    replacement_archives=()
    replacement_checksums=()
    for path in "${replacement_assets[@]}"; do
        case "$(basename -- "${path}")" in
            *.sha256) replacement_checksums+=("${path}") ;;
            *) replacement_archives+=("${path}") ;;
        esac
    done
    for path in ${replacement_checksums+"${replacement_checksums[@]}"}; do
        name=$(basename -- "${path}")
        echo "Withdrawing the superseded checksum ${name}."
        gh release delete-asset "${tag}" "${name}" --yes
    done
    if [[ "${#replacement_archives[@]}" -gt 0 ]]; then
        gh release upload "${tag}" --clobber "${replacement_archives[@]}"
    fi
    if [[ "${#replacement_checksums[@]}" -gt 0 ]]; then
        gh release upload "${tag}" "${replacement_checksums[@]}"
    fi
fi

if [[ "${#manifest_assets[@]}" -gt 0 ]]; then
    echo "Replacing ${#manifest_assets[@]} manifest(s)."
    gh release upload "${tag}" --clobber "${manifest_assets[@]}"
fi

gh release edit "${tag}" --title "${title}" --notes "${notes}" >/dev/null

# Archives that a manifest now on the release still names. A leg that failed to
# build publishes no new archive or manifest for its target, so that target's
# old manifest stays and keeps naming its old archive; the clean-up below must
# not remove it. Manifests are read from the release itself, after this run's
# uploads, so the rule covers exactly what a consumer can read.
referenced_archives() {
    local manifest_dir manifest status=0
    manifest_dir=$(mktemp -d)
    gh release download "${tag}" --pattern 'manifest-*.json' --dir "${manifest_dir}" \
        --clobber >/dev/null || status=$?
    if [[ "${status}" -eq 0 ]]; then
        shopt -s nullglob
        for manifest in "${manifest_dir}"/manifest-*.json; do
            jq -er '"whitaker-lints-\(.git_sha // error("no git_sha"))-\(.toolchain // error("no toolchain"))-\(.target // error("no target")).tar.zst"' \
                "${manifest}" || { status=$?; break; }
        done
        shopt -u nullglob
    fi
    rm -rf -- "${manifest_dir}"
    return "${status}"
}
# Cleanup fails closed: if any manifest cannot be downloaded or parsed, nothing
# is deleted, because an incomplete reference list could remove an archive a
# retained manifest still names. `set -e` stops the script at this assignment.
referenced_output=$(referenced_archives)
referenced=()
if [[ -n "${referenced_output}" ]]; then
    mapfile -t referenced <<<"${referenced_output}"
fi

is_referenced() {
    local needle=$1 name
    for name in ${referenced+"${referenced[@]}"}; do
        [[ "${name}" == "${needle}" ]] && return 0
    done
    return 1
}

# Superseded lint archives only. A dependency archive absent from this run's
# set is not stale, it is unchanged and deliberately left in place; deleting it
# here would recreate the very gap this script exists to close. Clippy
# snapshots are never removed either: every installer release downloads the
# generation it pins from this tag, including installers built before a
# re-pin. A lint archive that any manifest still on the release names is kept
# even when this run did not rebuild it.
for name in ${published_names+"${published_names[@]}"}; do
    case "${name}" in
        whitaker-lints-*) ;;
        *) continue ;;
    esac
    if is_wanted "${name}"; then
        continue
    fi
    if is_referenced "${name}"; then
        echo "Keeping ${name}: a manifest on the release still names it."
        continue
    fi
    echo "Removing superseded asset ${name}."
    gh release delete-asset "${tag}" "${name}" --yes
done

echo "Rolling release republished at ${RELEASE_SHA} with no empty interval."
