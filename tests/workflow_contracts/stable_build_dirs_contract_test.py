"""Validate that the linux-full scratch builds compile at fixed, private paths.

sccache keys a Rust compilation on its absolute paths, so a build tree named
afresh on every run makes every compilation in it a miss on every run. A
per-step probe of `linux-full` (dispatch run 36126452531) attributed all 340 of
its recurring Rust misses to the two recipes checked here: 151 to the installer
MSRV check and 189 to the publish check, each of which built under a
`mktemp -d` directory.

A fixed path is not enough on its own. The trees must stay outside the
workspace, because Cargo walks up from a package it installs, and a packaged
crate extracted under the checkout finds the root `Cargo.toml` and refuses to
build; the first attempt at this fix failed that way. They must sit under the
user's own cache directory rather than a shared temporary directory, where
another user could pre-create or swap a tree the recipes clear. And each name
must carry a fixed-length digest of the checkout's path, so two checkouts never
share a tree and a deep checkout never produces an over-long name.

The rules expand the recipes with `make -n` under a supplied `HOME` and
`XDG_CACHE_HOME`, so they judge the directory the shell is actually given, and
do so without depending on the environment the suite happens to run in.
"""

from __future__ import annotations

import hashlib
import os
import re
import subprocess
import typing as typ
from pathlib import PurePosixPath

import pytest
from ubicloud_workflow_support import REPOSITORY_ROOT

#: The recipes `linux-full` runs that compile in a scratch tree, mapped to the
#: name each tree starts with.
SCRATCH_BUILDS: typ.Final[dict[str, str]] = {
    "installer-msrv-check": "installer-msrv",
    "publish-check": "publish-check",
}

#: The scratch tree's assignment in the expanded recipe.
SCRATCH_ASSIGNMENT: typ.Final = re.compile(
    r"TMP_DIR=(?P<value>\"[^\"]*\"|\$\([^)]*\)|[^;\s]+);"
)

#: A home directory for the expansion, so the expected root does not depend on
#: the account running the suite.
FAKE_HOME: typ.Final[str] = "/home/contract-user"


def _expanded_recipe(target: str, cache_home: str | None) -> str:
    """Return a Make target's recipe as Make would hand it to the shell."""
    env = {key: value for key, value in os.environ.items() if key != "XDG_CACHE_HOME"}
    env["HOME"] = FAKE_HOME
    if cache_home is not None:
        env["XDG_CACHE_HOME"] = cache_home
    result = subprocess.run(
        ["make", "-n", "--no-print-directory", target, "PUBLISH_PACKAGES=x"],
        cwd=REPOSITORY_ROOT,
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    assert result.returncode == 0, f"make -n {target} failed: {result.stderr}"
    return result.stdout


def scratch_directory(recipe: str) -> str:
    """Return the value a recipe assigns to its scratch tree, unquoted.

    Parameters
    ----------
    recipe : str
        A recipe as `make -n` prints it.

    Returns
    -------
    str
        The value assigned to `TMP_DIR`, without surrounding quotes.

    Raises
    ------
    AssertionError
        When the recipe assigns `TMP_DIR` other than exactly once.

    >>> scratch_directory('set -eu; TMP_DIR="/c/x"; mkdir -p "$TMP_DIR";')
    '/c/x'
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

    Parameters
    ----------
    value : str
        A value as the shell would receive it.

    Returns
    -------
    bool
        ``True`` when the value holds no shell expansion.

    >>> is_stable_path("/c/whitaker/scratch/publish-check-0123456789abcdef")
    True
    >>> is_stable_path("$(mktemp -d)")
    False
    >>> is_stable_path("/c/publish-check-$$")
    False
    """
    return "$" not in value and "`" not in value


def is_outside(directory: str, workspace: str) -> bool:
    """Return whether an absolute directory lies outside a workspace.

    Parameters
    ----------
    directory : str
        The directory to judge.
    workspace : str
        The workspace root.

    Returns
    -------
    bool
        ``True`` when ``directory`` is absolute and neither the workspace nor
        anything beneath it.

    >>> is_outside("/c/whitaker/scratch/installer-msrv-0", "/w")
    True
    >>> is_outside("/w/target/installer-msrv", "/w")
    False
    >>> is_outside("/w-other/x", "/w")
    True
    """
    path, root = PurePosixPath(directory), PurePosixPath(workspace)
    return path.is_absolute() and path != root and root not in path.parents


def checkout_id(workspace: str) -> str:
    """Return the fixed-length identifier a checkout's scratch trees carry.

    Parameters
    ----------
    workspace : str
        The checkout's absolute path.

    Returns
    -------
    str
        The first 16 hex digits of the SHA-256 of the path.

    >>> checkout_id("/a-b/c") != checkout_id("/a/b-c")
    True
    >>> len(checkout_id("/" + "deep/" * 200))
    16
    """
    return hashlib.sha256(workspace.encode()).hexdigest()[:16]


@pytest.mark.parametrize(
    ("cache_home", "root"),
    [
        pytest.param(None, f"{FAKE_HOME}/.cache", id="home-cache"),
        pytest.param("/xdg/cache", "/xdg/cache", id="xdg-cache-home"),
    ],
)
@pytest.mark.parametrize(("target", "prefix"), SCRATCH_BUILDS.items())
def test_scratch_builds_use_a_fixed_private_directory(
    target: str, prefix: str, cache_home: str | None, root: str
) -> None:
    """A scratch tree is one private path per checkout, outside the checkout."""
    directory = scratch_directory(_expanded_recipe(target, cache_home))
    workspace = str(REPOSITORY_ROOT)
    assert is_stable_path(directory), (
        f"{target} builds in {directory!r}, which the shell names afresh on "
        "each run, so sccache misses every compilation in it"
    )
    assert is_outside(directory, workspace), (
        f"{target} builds in {directory!r}, inside the workspace, where Cargo "
        "finds the root manifest and refuses the extracted package"
    )
    expected = f"{root}/whitaker/scratch/{prefix}-{checkout_id(workspace)}"
    assert directory == expected, (
        f"{target} must build in the user's cache directory under a name "
        f"carrying the checkout's digest, {expected!r}, not {directory!r}"
    )
