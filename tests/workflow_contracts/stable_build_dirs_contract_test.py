"""Validate that the linux-full scratch builds compile at fixed paths.

sccache keys a Rust compilation on its absolute paths, so a build tree named
afresh on every run makes every compilation in it a miss on every run. A
per-step probe of `linux-full` (dispatch run 36126452531) attributed all 340 of
its recurring Rust misses to the two recipes checked here: 151 to the installer
MSRV check and 189 to the publish check, each of which built under a
`mktemp -d` directory.

A fixed path is not enough on its own. The trees must also stay outside the
workspace: Cargo walks up from a package it installs, and a packaged crate
extracted under the checkout finds the root `Cargo.toml` and refuses to build,
which is how the first attempt at this fix failed. The rules read the recipes as
Make would run them, so they judge the directory the shell is actually given
rather than how the Makefile spells it.
"""

from __future__ import annotations

import re
import subprocess
import typing as typ
from pathlib import PurePosixPath

import pytest
from ubicloud_workflow_support import REPOSITORY_ROOT

#: The recipes `linux-full` runs that compile in a scratch tree, mapped to the
#: name each tree starts with.
SCRATCH_BUILDS: typ.Final[dict[str, str]] = {
    "installer-msrv-check": "whitaker-installer-msrv",
    "publish-check": "whitaker-publish-check",
}

#: The scratch tree's assignment in the expanded recipe.
SCRATCH_ASSIGNMENT: typ.Final = re.compile(
    r"TMP_DIR=(?P<value>\"[^\"]*\"|\$\([^)]*\)|[^;\s]+);"
)


def _expanded_recipe(target: str) -> str:
    """Return a Make target's recipe with Make's own variables expanded.

    `make -n` prints the commands without running them, so the shell text it
    shows is what the recipe would hand the shell: `$(CURDIR)` is resolved and
    `$$` has become `$`.
    """
    result = subprocess.run(
        ["make", "-n", "--no-print-directory", target, "PUBLISH_PACKAGES=x"],
        cwd=REPOSITORY_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, f"make -n {target} failed: {result.stderr}"
    return result.stdout


def scratch_directory(recipe: str) -> str:
    """Return the value a recipe assigns to its scratch tree, unquoted.

    >>> scratch_directory('set -eu; TMP_DIR="/tmp/x"; mkdir -p "$TMP_DIR";')
    '/tmp/x'
    >>> scratch_directory('TMP_DIR=$(mktemp -d); trap ...')
    '$(mktemp -d)'
    """
    matches = SCRATCH_ASSIGNMENT.findall(recipe)
    assert len(matches) == 1, f"expected one TMP_DIR assignment, got {matches!r}"
    return matches[0].strip('"')


def is_stable_path(value: str) -> bool:
    """Return whether a shell value names the same directory on every run.

    Any `$` or backtick left after Make's expansion is shell expansion, such as
    `$(mktemp -d)` or a `$$` process identifier, and may differ run to run.

    >>> is_stable_path("/tmp/whitaker-publish-check-w")
    True
    >>> is_stable_path("$(mktemp -d)")
    False
    >>> is_stable_path("/tmp/whitaker-publish-check-$$")
    False
    """
    return "$" not in value and "`" not in value


def is_outside(directory: str, workspace: str) -> bool:
    """Return whether an absolute directory lies outside a workspace.

    >>> is_outside("/tmp/whitaker-installer-msrv-w", "/w")
    True
    >>> is_outside("/w/target/installer-msrv", "/w")
    False
    >>> is_outside("/w-other/x", "/w")
    True
    """
    path, root = PurePosixPath(directory), PurePosixPath(workspace)
    return path.is_absolute() and path != root and root not in path.parents


@pytest.mark.parametrize(("target", "prefix"), SCRATCH_BUILDS.items())
def test_scratch_builds_use_a_fixed_directory_outside_the_workspace(
    target: str, prefix: str
) -> None:
    """A scratch tree is one absolute path, outside the checkout, per checkout.

    Stable so sccache hits, outside so Cargo does not adopt the extracted
    package into the root workspace, and named after the checkout so two
    checkouts on one host do not clear each other's tree.
    """
    directory = scratch_directory(_expanded_recipe(target))
    workspace = str(REPOSITORY_ROOT)
    assert is_stable_path(directory), (
        f"{target} builds in {directory!r}, which the shell names afresh on "
        "each run, so sccache misses every compilation in it"
    )
    assert is_outside(directory, workspace), (
        f"{target} builds in {directory!r}, inside the workspace, where Cargo "
        "finds the root manifest and refuses the extracted package"
    )
    name = PurePosixPath(directory).name
    assert name == prefix + workspace.replace("/", "-"), (
        f"{target} must name its tree {prefix} plus this checkout's path, "
        f"not {name!r}"
    )
