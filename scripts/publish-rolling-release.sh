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
#   3. Move the tag, so a consumer that resolved the tag earlier in the run
#      still finds every asset it was promised.
#   4. Delete superseded assets, last of all.
#
# `gh release upload --clobber` deletes before it uploads, by its own
# documentation, so it is used only where an asset is genuinely being replaced.
# When the dependency manifest is unchanged the dependency archives are not
# rebuilt, are already published, and are left untouched: that is the case that
# broke chutoro, and it now involves no delete at all.
set -euo pipefail

: "${RELEASE_SHA:?RELEASE_SHA must name the commit the tag should point at}"
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
# window to protect because there is nothing published to lose.
if ! published=$(gh release view "${tag}" --json assets --jq '.assets[].name' 2>/dev/null); then
    echo "Rolling release absent; creating it with the full asset set."
    gh release create "${tag}" \
        --title "${title}" \
        --notes "${notes}" \
        --prerelease \
        --latest=false \
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
if [[ "${#replacement_assets[@]}" -gt 0 ]]; then
    echo "Replacing ${#replacement_assets[@]} rebuilt asset(s)."
    gh release upload "${tag}" --clobber "${replacement_assets[@]}"
fi

if [[ "${#manifest_assets[@]}" -gt 0 ]]; then
    echo "Replacing ${#manifest_assets[@]} manifest(s)."
    gh release upload "${tag}" --clobber "${manifest_assets[@]}"
fi

# The release names the tag, and its `target_commitish` is the branch rather
# than a commit, so moving the tag is a plain ref update. GitHub ignores
# `target_commitish` when patching a published release, which is why the tag
# and not the release is what moves.
echo "Moving ${tag} to ${RELEASE_SHA}."
gh api \
    --method PATCH \
    "repos/${repository}/git/refs/tags/${tag}" \
    -f "sha=${RELEASE_SHA}" \
    -F force=true >/dev/null

gh release edit "${tag}" --title "${title}" --notes "${notes}" >/dev/null

# Superseded lint archives only. A dependency archive absent from this run's
# set is not stale, it is unchanged and deliberately left in place; deleting it
# here would recreate the very gap this script exists to close.
for name in ${published_names+"${published_names[@]}"}; do
    case "${name}" in
        whitaker-lints-*) ;;
        *) continue ;;
    esac
    if ! is_wanted "${name}"; then
        echo "Removing superseded asset ${name}."
        gh release delete-asset "${tag}" "${name}" --yes
    fi
done

echo "Rolling release republished at ${RELEASE_SHA} with no empty interval."
