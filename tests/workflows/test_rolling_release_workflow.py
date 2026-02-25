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

import os
import shlex
from pathlib import Path

import pytest
from ruamel.yaml import YAML

from tests.workflows.workflow_test_helpers import (
    WORKFLOW_PATH,
    install_components_script,
    lint_crates_from_resolution_constants,
    lint_crates_from_workflow,
    run_act_build_lints,
    workflow_runtime_is_ready,
    workspace_package_names,
)


def test_lint_crates_are_workspace_packages() -> None:
    """Ensure `LINT_CRATES` only references real workspace packages."""
    lint_crates = lint_crates_from_workflow()
    workspace_packages = workspace_package_names()
    missing = sorted(set(lint_crates) - workspace_packages)
    assert not missing, (
        "rolling-release workflow includes non-workspace lint crates: "
        + ", ".join(missing)
    )


def test_workflow_lint_crates_match_installer_constants() -> None:
    """Ensure workflow crate list matches installer canonical constants."""
    workflow_crates = lint_crates_from_workflow()
    canonical_crates = lint_crates_from_resolution_constants()
    # Ordering is part of this contract: both definitions should evolve in lock-step
    # for deterministic build logs and packaging expectations.
    assert workflow_crates == canonical_crates, (
        "rolling-release workflow LINT_CRATES drifted from installer constants:\n"
        f"workflow={workflow_crates}\n"
        f"installer={canonical_crates}"
    )


def test_install_components_script_missing_step_raises() -> None:
    """Ensure helper fails when workflow lacks install-components step."""
    workflow_text = """name: Rolling Release
jobs:
  build-lints:
    steps:
      - name: Checkout
        uses: actions/checkout@v5
      - name: Install pinned toolchain (renamed)
        run: echo \"noop\"
"""
    with pytest.raises(
        AssertionError,
        match="missing the install-components run step",
    ):
        install_components_script(workflow_text)


def test_install_components_uses_only_matrix_target_rustc_dev() -> None:
    """Ensure install step avoids conflicting dual-target rustc-dev installs."""
    workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
    run_script = install_components_script(workflow_text)
    expected_target = "${{ matrix.target }}"
    install_lines = [
        line.strip()
        for line in run_script.splitlines()
        if line.strip().startswith("rustup component add")
    ]
    parsed_commands = []
    for line in install_lines:
        cleaned_line = line.rstrip()
        if cleaned_line.endswith("\\"):
            cleaned_line = cleaned_line[:-1].rstrip()
        parsed_commands.append(shlex.split(cleaned_line))

    rustc_dev_commands = [
        command for command in parsed_commands if "rustc-dev" in command
    ]
    assert (
        len(rustc_dev_commands) == 1
    ), "install step must contain exactly one rustc-dev rustup install command"

    rustc_dev_command = rustc_dev_commands[0]
    assert "--target" in rustc_dev_command, (
        "rustc-dev install command must include --target"
    )
    target_flag_index = rustc_dev_command.index("--target")
    assert target_flag_index + 1 < len(rustc_dev_command), (
        "rustc-dev install command must include a target value"
    )
    assert (
        rustc_dev_command[target_flag_index + 1] == expected_target
    ), "rustc-dev install command must target the matrix target"
    assert (
        "llvm-tools-preview" in rustc_dev_command
    ), (
        "rustc-dev install command must include llvm-tools-preview in the same "
        "command"
    )
    rust_src_commands = [
        command for command in parsed_commands if "rust-src" in command
    ]
    assert not rust_src_commands, (
        "install step must not install rust-src because it conflicts with "
        "targeted rustc-dev payloads on some runners"
    )


def test_publish_job_runs_even_if_build_lints_fails() -> None:
    """Ensure publish job still runs when some build-lints matrix legs fail."""
    workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
    parsed = YAML(typ="safe").load(workflow_text)
    assert isinstance(parsed, dict), (
        "rolling-release workflow must parse to a mapping"
    )

    jobs = parsed.get("jobs")
    assert isinstance(jobs, dict), "rolling-release workflow must declare jobs"
    publish_job = jobs.get("publish")
    assert isinstance(publish_job, dict), (
        "rolling-release workflow must declare publish job"
    )
    needs = publish_job.get("needs")
    match needs:
        case str():
            needs_list = [needs]
        case list():
            needs_list = needs
        case _:
            assert False, "publish job needs must be a string or list"

    assert "build-lints" in needs_list, "publish job must depend on build-lints"
    assert publish_job.get("if") == "${{ always() }}", (
        "publish job must run even when build-lints has failing matrix legs"
    )

    steps = publish_job.get("steps")
    assert isinstance(steps, list), "publish job must declare steps"
    download_step = next(
        (
            step
            for step in steps
            if isinstance(step, dict) and step.get("name") == "Download all artefacts"
        ),
        None,
    )
    assert isinstance(download_step, dict), (
        "publish job must download build artefacts before publish checks"
    )
    assert download_step.get("continue-on-error") is True, (
        "download step must continue on error so zero-artefact runs can fall "
        "through to has_assets=false"
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
