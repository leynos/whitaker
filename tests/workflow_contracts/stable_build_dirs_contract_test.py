"""Validate that the linux-full scratch builds compile at fixed paths.

sccache keys a Rust compilation on its absolute paths, so a build tree named
afresh on every run makes every compilation in it a miss on every run. A
per-step probe of `linux-full` (dispatch run 36126452531) attributed all 340 of
its recurring Rust misses to the two recipes checked here: 151 to the installer
MSRV check and 189 to the publish check, each of which built under a
`mktemp -d` directory. The rule reads the recipes as Make would run them, so it
judges the directory the shell is actually given rather than how the Makefile
spells it.
"""

from __future__ import annotations

import re
import subprocess
import typing as typ

import pytest
from ubicloud_workflow_support import REPOSITORY_ROOT

#: The recipes `linux-full` runs that compile in a scratch tree, mapped to the
#: directory each must use. Both sit under the workspace's own `target`.
SCRATCH_BUILDS: typ.Final[dict[str, str]] = {
    "installer-msrv-check": "target/installer-msrv",
    "publish-check": "target/publish-check",
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

    >>> scratch_directory('set -eu; TMP_DIR="/w/target/x"; mkdir -p "$TMP_DIR";')
    '/w/target/x'
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

    >>> is_stable_path("/w/target/publish-check")
    True
    >>> is_stable_path("$(mktemp -d)")
    False
    >>> is_stable_path("/w/target/publish-check-$$")
    False
    """
    return "$" not in value and "`" not in value


@pytest.mark.parametrize(("target", "relative"), SCRATCH_BUILDS.items())
def test_scratch_builds_use_a_fixed_directory_under_target(
    target: str, relative: str
) -> None:
    """A scratch build tree must be the same absolute path on every run."""
    directory = scratch_directory(_expanded_recipe(target))
    assert is_stable_path(directory), (
        f"{target} builds in {directory!r}, which the shell names afresh on "
        "each run, so sccache misses every compilation in it"
    )
    assert directory == str(REPOSITORY_ROOT / relative), (
        f"{target} must build in {relative} under the workspace, not {directory!r}"
    )
