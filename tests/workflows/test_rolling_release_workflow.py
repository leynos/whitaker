"""Black-box smoke tests for the rolling release GitHub Actions workflow."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github/workflows/rolling-release.yml"
EVENT_PATH = (
    REPO_ROOT
    / "tests/workflows/fixtures/workflow_dispatch.rolling-release.event.json"
)
MATRIX = {"os": "ubuntu-latest", "target": "x86_64-unknown-linux-gnu"}


def lint_crates_from_workflow() -> list[str]:
    marker = "  LINT_CRATES: >-"
    lines = WORKFLOW_PATH.read_text(encoding="utf-8").splitlines()
    try:
        start = lines.index(marker) + 1
    except ValueError as error:
        raise AssertionError(f"{WORKFLOW_PATH} is missing {marker!r}") from error

    lint_crates: list[str] = []
    for line in lines[start:]:
        if not line.startswith("    "):
            break
        crate_name = line.strip()
        if crate_name:
            lint_crates.append(crate_name)

    assert lint_crates, "rolling-release workflow must define at least one lint crate"
    return lint_crates


def workspace_package_names() -> set[str]:
    completed = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, (
        "cargo metadata failed while resolving workspace packages:\n"
        f"{completed.stderr}"
    )
    metadata = json.loads(completed.stdout)
    return {package["name"] for package in metadata["packages"]}


def run_act_build_lints(*, artifact_dir: Path) -> tuple[int, str]:
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
    )
    logs = f"{completed.stdout}\n{completed.stderr}"
    return completed.returncode, logs


def workflow_runtime_is_ready() -> bool:
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
    )
    return completed.returncode == 0


def test_lint_crates_are_workspace_packages() -> None:
    lint_crates = lint_crates_from_workflow()
    workspace_packages = workspace_package_names()
    missing = sorted(set(lint_crates) - workspace_packages)
    assert not missing, (
        "rolling-release workflow includes non-workspace lint crates: "
        + ", ".join(missing)
    )
    assert "suite" not in lint_crates
    assert "whitaker_suite" in lint_crates


@pytest.mark.skipif(
    os.getenv("ACT_WORKFLOW_TESTS") != "1",
    reason="set ACT_WORKFLOW_TESTS=1 to run act workflow smoke tests",
)
def test_build_lints_job_succeeds_under_act(tmp_path: Path) -> None:
    if not workflow_runtime_is_ready():
        pytest.skip("act runtime is unavailable or cannot reach the container socket")

    artifact_dir = tmp_path / "act-artifacts"
    code, logs = run_act_build_lints(artifact_dir=artifact_dir)
    assert code == 0, f"act build-lints job failed:\n{logs}"
    assert "cannot specify features for packages outside of workspace" not in logs
    assert any(artifact_dir.rglob("*")), "act did not export any artefacts"
