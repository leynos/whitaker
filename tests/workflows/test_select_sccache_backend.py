"""Behavioural tests for the compiler-cache backend selector.

`scripts/select-sccache-backend.sh` is the single point that routes sccache
at either the GitHub Actions cache service or a directory owned by a cache
step. Every Linux lane runs it before any Cargo invocation, so a wrong or
partial export silently changes where the whole repository's compiler
output is stored.

Covered behaviour:

- `gha` exports only the Actions-service switch and no directory;
- `local` exports the directory and the size cap, and never the switch;
- the directory and cap honour their overrides and fall back to the
  documented defaults;
- an unset or empty backend keeps the historical `gha` default; and
- an unrecognized backend is a hard error that exports nothing.

Examples
--------
Run all tests:
    PYTHONPATH=. python3 -m pytest tests/workflows/test_select_sccache_backend.py
"""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "select-sccache-backend.sh"
DEFAULT_CACHE_SIZE = "4G"


def _run(
    tmp_path: Path,
    **overrides: str,
) -> tuple[subprocess.CompletedProcess[str], dict[str, str]]:
    """Run the selector and return its result and the exported environment."""
    env_file = tmp_path / "github-env"
    env_file.touch()
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(tmp_path / "home"),
        "GITHUB_ENV": str(env_file),
        **overrides,
    }
    result = subprocess.run(
        ["bash", str(SCRIPT)],
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    exported = dict(
        line.split("=", 1)
        for line in env_file.read_text(encoding="utf-8").splitlines()
        if "=" in line
    )
    return result, exported


def test_gha_backend_exports_only_the_actions_switch(tmp_path: Path) -> None:
    """The Actions backend needs no directory and must not export one."""
    result, exported = _run(tmp_path, SCCACHE_BACKEND="gha")

    assert result.returncode == 0, result.stderr
    assert exported == {"SCCACHE_GHA_ENABLED": "true"}, (
        f"the Actions backend must export exactly one variable, got {exported}"
    )


@pytest.mark.parametrize(
    "overrides",
    [
        pytest.param({}, id="unset"),
        pytest.param({"SCCACHE_BACKEND": ""}, id="empty"),
    ],
)
def test_absent_backend_keeps_the_actions_default(
    tmp_path: Path,
    overrides: dict[str, str],
) -> None:
    """An unset or empty selector is the documented `gha` default.

    An empty value is treated as absent rather than rejected, so a lane that
    drops the workflow-level declaration behaves exactly as it did before the
    selector existed instead of failing at the first Cargo step.
    """
    result, exported = _run(tmp_path, **overrides)

    assert result.returncode == 0, result.stderr
    assert exported == {"SCCACHE_GHA_ENABLED": "true"}, (
        f"an absent backend must behave as `gha`, got {exported}"
    )


def test_local_backend_exports_a_bounded_directory(tmp_path: Path) -> None:
    """The local backend needs a directory and a cap, and no Actions switch."""
    result, exported = _run(tmp_path, SCCACHE_BACKEND="local")

    assert result.returncode == 0, result.stderr
    assert exported == {
        "SCCACHE_DIR": str(tmp_path / "home" / ".cache" / "sccache"),
        "SCCACHE_CACHE_SIZE": DEFAULT_CACHE_SIZE,
    }, f"the local backend must export a bounded directory, got {exported}"
    assert "SCCACHE_GHA_ENABLED" not in exported, (
        "the two backends must never be combined"
    )


def test_local_backend_honours_its_overrides(tmp_path: Path) -> None:
    """The directory and cap are configurable for a differently shaped runner."""
    result, exported = _run(
        tmp_path,
        SCCACHE_BACKEND="local",
        SCCACHE_LOCAL_DIR=str(tmp_path / "elsewhere"),
        SCCACHE_LOCAL_CACHE_SIZE="9G",
    )

    assert result.returncode == 0, result.stderr
    assert exported == {
        "SCCACHE_DIR": str(tmp_path / "elsewhere"),
        "SCCACHE_CACHE_SIZE": "9G",
    }, f"both overrides must reach the exported environment, got {exported}"


@pytest.mark.parametrize(
    "backend",
    [
        pytest.param("GHA", id="wrong-case"),
        pytest.param("redis", id="unsupported-store"),
        pytest.param("local ", id="trailing-space"),
    ],
)
def test_unknown_backend_aborts_without_exporting(
    tmp_path: Path,
    backend: str,
) -> None:
    """An unrecognized backend must fail rather than pick a store silently."""
    result, exported = _run(tmp_path, SCCACHE_BACKEND=backend)

    assert result.returncode == 1, result.stdout
    assert "Unknown SCCACHE_BACKEND" in result.stderr, result.stderr
    assert exported == {}, f"a rejected backend must export nothing, got {exported}"
