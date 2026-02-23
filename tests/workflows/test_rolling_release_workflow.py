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
import re
import shutil
import subprocess
from pathlib import Path

import pytest
from ruamel.yaml import YAML

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github/workflows/rolling-release.yml"
RESOLUTION_PATH = REPO_ROOT / "installer/src/resolution.rs"
EVENT_PATH = (
    REPO_ROOT
    / "tests/workflows/fixtures/workflow_dispatch.rolling-release.event.json"
)
MATRIX = {"os": "ubuntu-latest", "target": "x86_64-unknown-linux-gnu"}
CARGO_METADATA_TIMEOUT_SECONDS = 30
ACT_LIST_TIMEOUT_SECONDS = 60
ACT_RUN_TIMEOUT_SECONDS = 900


def _find_lint_crates_value(parsed: dict[str, object]) -> str | list[str] | None:
    """Return the raw `LINT_CRATES` value from workflow or build-lints env."""
    workflow_env = None
    jobs = None
    match parsed:
        case {"env": workflow_env_candidate, "jobs": jobs_candidate}:
            workflow_env = workflow_env_candidate
            jobs = jobs_candidate
        case {"env": workflow_env_candidate}:
            workflow_env = workflow_env_candidate
        case {"jobs": jobs_candidate}:
            jobs = jobs_candidate
        case _:
            return None

    build_lints = None
    match jobs:
        case {"build-lints": build_lints_candidate}:
            build_lints = build_lints_candidate
        case _:
            build_lints = None

    build_lints_env = None
    match build_lints:
        case {"env": build_lints_env_candidate}:
            build_lints_env = build_lints_env_candidate
        case _:
            build_lints_env = None

    for env in (workflow_env, build_lints_env):
        match env:
            case {"LINT_CRATES": lint_value} if lint_value is not None:
                return lint_value
            case _:
                continue

    return None


def _normalize_lint_crates_value(lint_value: str | list[str] | None) -> list[str]:
    """Normalize a raw `LINT_CRATES` value into a list of crate names."""
    match lint_value:
        case list() as crates:
            return [str(crate).strip() for crate in crates if str(crate).strip()]
        case str() as crates:
            return [crate for crate in crates.split() if crate]
        case None:
            return []
        case _:
            return [crate for crate in str(lint_value).split() if crate]


def _extract_lint_crates_from_text(workflow_text: str) -> list[str]:
    """Return the list of crate names from workflow text."""
    yaml = YAML()
    parsed = yaml.load(workflow_text)
    match parsed:
        case dict():
            parsed_dict = parsed
        case _:
            parsed_dict = {}
    lint_value = _find_lint_crates_value(parsed_dict)

    if lint_value is None:
        raise AssertionError(f"{WORKFLOW_PATH} is missing a LINT_CRATES declaration")

    return _normalize_lint_crates_value(lint_value)


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


def lint_crates_from_resolution_constants() -> list[str]:
    """Return canonical lint crate names from installer constants.

    Parameters
    ----------
    None

    Returns
    -------
    list[str]
        `LINT_CRATES` plus `SUITE_CRATE` from `installer/src/resolution.rs`.
    """
    source = RESOLUTION_PATH.read_text(encoding="utf-8")
    lint_match = re.search(
        r"pub const LINT_CRATES: &\[&str\] = &\[(.*?)\];",
        source,
        flags=re.DOTALL,
    )
    assert lint_match, f"unable to parse LINT_CRATES from {RESOLUTION_PATH}"
    lint_crates = re.findall(r'"([^"]+)"', lint_match.group(1))
    assert lint_crates, f"{RESOLUTION_PATH} LINT_CRATES is unexpectedly empty"

    suite_match = re.search(r'pub const SUITE_CRATE: &str = "([^"]+)";', source)
    assert suite_match, f"unable to parse SUITE_CRATE from {RESOLUTION_PATH}"
    return lint_crates + [suite_match.group(1)]


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


def run_act_build_lints(*, artefact_dir: Path) -> tuple[int, str]:
    """Run the workflow build-lints job through `act`.

    Parameters
    ----------
    artefact_dir
        Directory where uploaded artefacts should be written by `act`.

    Returns
    -------
    tuple[int, str]
        Process return code and merged stdout/stderr logs.
    """
    artefact_dir.mkdir(parents=True, exist_ok=True)
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
        str(artefact_dir),
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


def test_workflow_lint_crates_match_installer_constants() -> None:
    """Ensure workflow crate list matches installer canonical constants.

    Parameters
    ----------
    None

    Returns
    -------
    None
    """
    workflow_crates = lint_crates_from_workflow()
    canonical_crates = lint_crates_from_resolution_constants()
    assert workflow_crates == canonical_crates, (
        "rolling-release workflow LINT_CRATES drifted from installer constants:\n"
        f"workflow={workflow_crates}\n"
        f"installer={canonical_crates}"
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
        Pytest-provided temporary path used for artefact output.

    Returns
    -------
    None
    """
    if not workflow_runtime_is_ready():
        pytest.fail(
            "ACT_WORKFLOW_TESTS=1 was set but act runtime is unavailable or "
            "cannot reach the container socket"
        )

    artefact_dir = tmp_path / "act-artefacts"
    code, logs = run_act_build_lints(artefact_dir=artefact_dir)
    assert code == 0, f"act build-lints job failed:\n{logs}"
    assert "cannot specify features for packages outside of workspace" not in logs, (
        "Unexpected error message found in logs: "
        "cannot specify features for packages outside of workspace\n"
        f"{logs}"
    )
    assert any(path.is_file() for path in artefact_dir.rglob("*")), (
        "act did not export any artefact files"
    )
