"""Validate the rolling release workflow with contract and smoke checks.

This module provides two complementary checks for
`.github/workflows/rolling-release.yml`:

1. A fast contract test that ensures every crate listed in `LINT_CRATES` is a
   real workspace package.
2. An opt-in black-box smoke test that runs the `build-lints` job with `act`.

The smoke test is intentionally gated behind `ACT_WORKFLOW_TESTS=1` because it
depends on a container runtime and can take several minutes.

Examples
--------
Run the contract check:
`python3 -m pytest tests/workflows/test_rolling_release_workflow.py -k lint`

Run the `act` smoke test:
`ACT_WORKFLOW_TESTS=1 python3 -m pytest tests/workflows/test_rolling_release_workflow.py`
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest
from ruamel.yaml import YAML

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github/workflows/rolling-release.yml"
EVENT_PATH = (
    REPO_ROOT
    / "tests/workflows/fixtures/workflow_dispatch.rolling-release.event.json"
)
MATRIX = {"os": "ubuntu-latest", "target": "x86_64-unknown-linux-gnu"}
CARGO_METADATA_TIMEOUT_SECONDS = 30
ACT_LIST_TIMEOUT_SECONDS = 60
ACT_RUN_TIMEOUT_SECONDS = 900


def _extract_lint_crates_from_text(workflow_text: str) -> list[str]:
    """Extract `LINT_CRATES` from workflow YAML using `ruamel.yaml`.

    Parameters
    ----------
    workflow_text
        Raw contents of `.github/workflows/rolling-release.yml`.

    Returns
    -------
    list[str]
        Parsed crate names in declaration order.
    """
    yaml = YAML()
    parsed = yaml.load(workflow_text)

    workflow_env = parsed.get("env") if isinstance(parsed, dict) else None
    jobs = parsed.get("jobs") if isinstance(parsed, dict) else None
    build_lints = jobs.get("build-lints") if isinstance(jobs, dict) else None
    build_lints_env = (
        build_lints.get("env") if isinstance(build_lints, dict) else None
    )
    lint_value = None

    for env in (workflow_env, build_lints_env):
        if not isinstance(env, dict):
            continue
        lint_value = env.get("LINT_CRATES")
        if lint_value is not None:
            break

    if lint_value is None:
        raise AssertionError(f"{WORKFLOW_PATH} is missing a LINT_CRATES declaration")

    if isinstance(lint_value, list):
        return [str(crate).strip() for crate in lint_value if str(crate).strip()]

    if isinstance(lint_value, str):
        return [crate for crate in lint_value.split() if crate]

    return [crate for crate in str(lint_value).split() if crate]


def lint_crates_from_workflow() -> list[str]:
    """Return lint crates declared in the rolling release workflow.

    Parameters
    ----------
    None

    Returns
    -------
    list[str]
        Crate names used by the workflow build loop.
    """
    workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
    lint_crates = _extract_lint_crates_from_text(workflow_text)
    assert lint_crates, "rolling-release workflow must define at least one lint crate"
    return lint_crates


def workspace_package_names() -> set[str]:
    """Resolve package names from Cargo workspace metadata.

    Parameters
    ----------
    None

    Returns
    -------
    set[str]
        Workspace package names from `cargo metadata`.
    """
    completed = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=CARGO_METADATA_TIMEOUT_SECONDS,
    )
    assert completed.returncode == 0, (
        "cargo metadata failed while resolving workspace packages:\n"
        f"{completed.stderr}"
    )
    metadata = json.loads(completed.stdout)
    return {package["name"] for package in metadata["packages"]}


def run_act_build_lints(*, artifact_dir: Path) -> tuple[int, str]:
    """Run the workflow build-lints job through `act`.

    Parameters
    ----------
    artifact_dir
        Directory where uploaded artifacts should be written by `act`.

    Returns
    -------
    tuple[int, str]
        Process return code and merged stdout/stderr logs.
    """
    artifact_dir.mkdir(parents=True, exist_ok=True)
    lint_crates = " ".join(lint_crates_from_workflow())
    command = [
        "act",
        "workflow_dispatch",
        "-W",
        str(WORKFLOW_PATH.relative_to(REPO_ROOT)),
        "-j",
        "build-lints",
        "-e",
        str(EVENT_PATH.relative_to(REPO_ROOT)),
        "-P",
        "ubuntu-latest=catthehacker/ubuntu:act-latest",
        "--artifact-server-path",
        str(artifact_dir),
        "--json",
        "--bind",
        "--matrix",
        f"os:{MATRIX['os']}",
        "--matrix",
        f"target:{MATRIX['target']}",
        "--env",
        f"LINT_CRATES={lint_crates}",
    ]
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=ACT_RUN_TIMEOUT_SECONDS,
    )
    logs = f"{completed.stdout}\n{completed.stderr}"
    return completed.returncode, logs


def workflow_runtime_is_ready() -> bool:
    """Check whether `act` can reach a working container runtime.

    Parameters
    ----------
    None

    Returns
    -------
    bool
        `True` when `act --list` succeeds for the workflow; otherwise `False`.
    """
    if shutil.which("act") is None:
        return False

    completed = subprocess.run(
        [
            "act",
            "workflow_dispatch",
            "-W",
            str(WORKFLOW_PATH.relative_to(REPO_ROOT)),
            "--list",
        ],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=ACT_LIST_TIMEOUT_SECONDS,
    )
    return completed.returncode == 0


def test_lint_crates_are_workspace_packages() -> None:
    """Ensure `LINT_CRATES` only references real workspace packages.

    Parameters
    ----------
    None

    Returns
    -------
    None
    """
    lint_crates = lint_crates_from_workflow()
    workspace_packages = workspace_package_names()
    missing = sorted(set(lint_crates) - workspace_packages)
    assert not missing, (
        "rolling-release workflow includes non-workspace lint crates: "
        + ", ".join(missing)
    )


@pytest.mark.skipif(
    os.getenv("ACT_WORKFLOW_TESTS") != "1",
    reason="set ACT_WORKFLOW_TESTS=1 to run act workflow smoke tests",
)
def test_build_lints_job_succeeds_under_act(tmp_path: Path) -> None:
    """Verify the `build-lints` job succeeds under `act`.

    Parameters
    ----------
    tmp_path
        Pytest-provided temporary path used for artifact output.

    Returns
    -------
    None
    """
    if not workflow_runtime_is_ready():
        pytest.fail(
            "ACT_WORKFLOW_TESTS=1 was set but act runtime is unavailable or "
            "cannot reach the container socket"
        )

    artifact_dir = tmp_path / "act-artifacts"
    code, logs = run_act_build_lints(artifact_dir=artifact_dir)
    assert code == 0, f"act build-lints job failed:\n{logs}"
    assert "cannot specify features for packages outside of workspace" not in logs
    assert any(path.is_file() for path in artifact_dir.rglob("*")), (
        "act did not export any artifact files"
    )
