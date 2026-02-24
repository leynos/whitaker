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
from pathlib import Path

import pytest

from tests.workflows.workflow_test_helpers import (
    WORKFLOW_PATH,
    _install_components_script,
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
        _install_components_script(workflow_text)


def test_install_components_uses_only_matrix_target_rustc_dev() -> None:
    """Ensure install step avoids conflicting dual-target rustc-dev installs."""
    workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
    run_script = _install_components_script(workflow_text)

    expected_matrix_install = (
        '--target "${{ matrix.target }}" rustc-dev llvm-tools-preview'
    )
    assert (
        expected_matrix_install in run_script
    ), "install step must install rustc-dev for matrix target"
    assert (
        '--target "${HOST_TARGET}" rustc-dev llvm-tools-preview' not in run_script
    ), "install step should not install rustc-dev for HOST_TARGET"
    assert (
        'if [ "${HOST_TARGET}" != "${{ matrix.target }}" ];' not in run_script
    ), "install step should not conditionally install a second rustc-dev target"

    rustc_dev_install_lines = [
        line
        for line in run_script.splitlines()
        if "rustup component add" in line and "rustc-dev" in line
    ]
    assert (
        len(rustc_dev_install_lines) == 1
    ), "install step must contain exactly one rustc-dev rustup install line"

    matrix_target_install_lines = [
        line for line in rustc_dev_install_lines if expected_matrix_install in line
    ]
    assert (
        len(matrix_target_install_lines) == 1
    ), "the single rustc-dev install line must target the matrix target"

    install_lines = [
        line for line in run_script.splitlines() if "rustup component add" in line
    ]
    llvm_tools_occurrences = sum(
        "llvm-tools-preview" in line for line in install_lines
    )
    assert (
        llvm_tools_occurrences == 1
    ), (
        "'llvm-tools-preview' must appear exactly once in install script, "
        f"found {llvm_tools_occurrences}"
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
