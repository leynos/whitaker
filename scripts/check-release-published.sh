#!/usr/bin/env bash
# check-release-published.sh — Say whether a version's GitHub release exists.
#
# Usage:
#   GITHUB_REPOSITORY=owner/repo scripts/check-release-published.sh VERSION
#
# Prints `published=true` or `published=false` on stdout, ready to append to
# `$GITHUB_OUTPUT`. Only a confirmed HTTP 404 means "not published". A rate
# limit, an authentication failure, or a server error exits 1 with the lookup's
# own message, so a failed lookup can never make a verification skip green.
set -euo pipefail

if [[ $# -ne 1 || -z "${GITHUB_REPOSITORY:-}" ]]; then
    printf 'usage: GITHUB_REPOSITORY=owner/repo %s VERSION\n' "$0" >&2
    exit 2
fi
version=$1

if lookup="$(gh api "repos/${GITHUB_REPOSITORY}/releases/tags/v${version}" 2>&1)"; then
    printf 'published=true\n'
elif [[ "$lookup" == *"HTTP 404"* ]]; then
    printf 'published=false\n'
else
    printf 'check-release-published: lookup for v%s failed: %s\n' "$version" "$lookup" >&2
    exit 1
fi
