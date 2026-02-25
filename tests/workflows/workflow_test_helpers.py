"""Shared helper utilities for rolling-release workflow tests.

This module centralizes helpers used by workflow contract and smoke tests,
including workflow parsing utilities, workspace metadata lookups, and `act`
runtime execution helpers.

Main utilities provided:
- Crate-list extraction and normalization helpers for workflow/config checks.
- Subprocess-backed helpers for Cargo metadata and `act` invocation.
- Reusable workflow fixture paths/constants consumed by test modules.

Typical usage:
Tests import these helpers to assert workflow contracts and optionally run
`build-lints` through `act` with deterministic matrix inputs.

Examples
--------
>>> from tests.workflows.workflow_test_helpers import workflow_runtime_is_ready
>>> isinstance(workflow_runtime_is_ready(), bool)
True
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
from pathlib import Path

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

    build_lints_env = None
    match build_lints:
        case {"env": build_lints_env_candidate}:
            build_lints_env = build_lints_env_candidate

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

    assert lint_value is not None, (
        f"{WORKFLOW_PATH} is missing a LINT_CRATES declaration"
    )

    return _normalize_lint_crates_value(lint_value)


def install_components_script(workflow_text: str) -> str:
    """Return the run script for the install-components workflow step.

    Parameters
    ----------
    workflow_text : str
        Raw YAML text for the rolling-release workflow.

    Returns
    -------
    str
        The shell script configured under the
        `Install pinned toolchain components` step.

    Raises
    ------
    AssertionError
        Raised when the install-components run step is absent.
    """
    yaml = YAML()
    parsed = yaml.load(workflow_text)
    run_script: str | None = None
    match parsed:
        case {"jobs": {"build-lints": {"steps": list() as steps}}}:
            for step in steps:
                match step:
                    case {
                        "name": "Install pinned toolchain components",
                        "run": str() as candidate_script,
                    }:
                        run_script = candidate_script
                        break
        case _:
            pass
    assert run_script is not None, (
        "rolling-release workflow is missing the install-components run step"
    )
    return run_script


def lint_crates_from_workflow() -> list[str]:
    """Return lint crates declared in the rolling release workflow.

    Returns
    -------
    list[str]
        Workflow lint crate names in declaration order.
    """
    workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
    lint_crates = _extract_lint_crates_from_text(workflow_text)
    assert lint_crates, "rolling-release workflow must define at least one lint crate"
    return lint_crates


def lint_crates_from_resolution_constants() -> list[str]:
    """Return canonical lint crate names from installer constants.

    Returns
    -------
    list[str]
        Canonical lint crate names from `LINT_CRATES` plus `SUITE_CRATE`.
    """
    source = RESOLUTION_PATH.read_text(encoding="utf-8")
    lint_match = re.search(
        r"pub(?:\(crate\))?\s+const\s+LINT_CRATES\s*:\s*&\[\s*&str\s*\]\s*=\s*&\[(.*?)\];",
        source,
        flags=re.DOTALL,
    )
    assert lint_match, f"unable to parse LINT_CRATES from {RESOLUTION_PATH}"
    lint_crates = re.findall(r'"([^"]+)"', lint_match.group(1))
    assert lint_crates, f"{RESOLUTION_PATH} LINT_CRATES is unexpectedly empty"

    suite_match = re.search(
        r'pub(?:\(crate\))?\s+const\s+SUITE_CRATE\s*:\s*&str\s*=\s*"([^"]+)";',
        source,
    )
    assert suite_match, f"unable to parse SUITE_CRATE from {RESOLUTION_PATH}"
    return [*lint_crates, suite_match.group(1)]


def workspace_package_names() -> set[str]:
    """Resolve package names from Cargo workspace metadata.

    Returns
    -------
    set[str]
        Package names reported by `cargo metadata --no-deps`.
    """
    completed = subprocess.run(  # noqa: S603,S607  # FIXME: uses trusted test-only PATH-resolved tool
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
    """Run the workflow `build-lints` job through `act`.

    Parameters
    ----------
    artefact_dir : Path
        Directory where `act` exports build artefacts.

    Returns
    -------
    tuple[int, str]
        A pair of `(return_code, logs)` containing the process exit status and
        combined standard output/error text.

    Raises
    ------
    AssertionError
        Propagated when `lint_crates_from_workflow()` cannot extract
        `LINT_CRATES`.
    subprocess.TimeoutExpired
        Raised if `act` does not complete within
        `ACT_RUN_TIMEOUT_SECONDS`.
    OSError
        Raised when the subprocess cannot be started (for example if `act` is
        unavailable).
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
    completed = subprocess.run(  # noqa: S603,S607  # FIXME: uses trusted test-only PATH-resolved tool
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
    """Return whether `act --list` can run for the workflow.

    Returns
    -------
    bool
        `True` when `act` is available and can list the workflow, otherwise
        `False`.
    """
    if shutil.which("act") is None:
        return False

    completed = subprocess.run(  # noqa: S603,S607  # FIXME: uses trusted test-only PATH-resolved tool
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
