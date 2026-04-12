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
import re
import shlex
from collections.abc import Mapping
from pathlib import Path
from typing import Any, cast

import pytest
from ruamel.yaml import YAML

from tests.workflows.workflow_test_helpers import (
    install_components_script,
    lint_crates_from_resolution_constants,
    lint_crates_from_workflow,
    package_binary_target_names,
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


def test_installer_packaging_bins_match_release_workflow_contract() -> None:
    """Ensure workflow-invoked packaging helpers are real Cargo bin targets."""
    binary_targets = package_binary_target_names("whitaker-installer")

    assert "whitaker-installer" in binary_targets, (
        "installer package must expose primary whitaker-installer binary target "
        "to satisfy release workflow contract"
    )
    assert "whitaker-package-lints" in binary_targets, (
        "rolling-release workflow builds --bin whitaker-package-lints, but the "
        "installer package does not declare that binary target"
    )
    assert "whitaker-package-installer" in binary_targets, (
        "release workflow builds --bin whitaker-package-installer, but the "
        "installer package does not declare that binary target"
    )
    assert "whitaker-package-dependency-binary" in binary_targets, (
        "release and rolling-release workflows build --bin "
        "whitaker-package-dependency-binary, but the installer package does "
        "not declare that binary target"
    )
    assert "package_lints" not in binary_targets, (
        "installer packaging bins must use the workflow-facing target name, "
        "not the source filename-derived fallback"
    )
    assert "package_installer_bin" not in binary_targets, (
        "installer packaging bins must use the workflow-facing target name, "
        "not the source filename-derived fallback"
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


def test_install_components_uses_only_matrix_target_rustc_dev(workflow_text: str) -> None:
    """Ensure install step avoids conflicting dual-target rustc-dev installs."""
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


def _load_workflow_mapping(yaml_text: str) -> dict[str, object]:
    """Load YAML text and return workflow mapping."""
    parsed = YAML(typ="safe").load(yaml_text)
    match parsed:
        case dict() as workflow_mapping:
            return workflow_mapping
        case _:
            pytest.fail("rolling-release workflow must parse to a mapping")


def _get_job_dict(jobs: Mapping[str, Any], job_name: str) -> dict[str, Any]:
    """Return the requested job mapping from the jobs map."""
    match jobs.get(job_name):
        case dict() as job_dict:
            return job_dict
        case _:
            if job_name == "jobs":
                pytest.fail("rolling-release workflow must declare jobs")
            pytest.fail(f"rolling-release workflow must declare {job_name} job")


def _workflow_dispatch_inputs(workflow_mapping: Mapping[str, Any]) -> dict[str, Any]:
    """Return the `workflow_dispatch.inputs` mapping."""
    on_mapping = workflow_mapping.get("on")
    match on_mapping:
        case {"workflow_dispatch": {"inputs": dict() as inputs}}:
            return inputs
        case {"workflow_dispatch": dict()}:
            pytest.fail("workflow_dispatch must declare an inputs mapping")
        case {"workflow_dispatch": _}:
            pytest.fail("workflow_dispatch trigger must be a mapping")
        case _:
            pytest.fail("rolling-release workflow must declare workflow_dispatch")


def _github_expression_value(value: object) -> str:
    """Return a GitHub Actions expression with wrapper delimiters removed."""
    if not isinstance(value, str):
        pytest.fail("workflow expression value must be a string")
    stripped = value.strip()
    if stripped.startswith("${{") and stripped.endswith("}}"):
        return stripped[3:-2].strip()
    return stripped


def _get_needs_list(publish_job: dict[str, Any]) -> list[str]:
    """Return publish job dependency names as a list."""
    needs: str | list[str] | None = publish_job.get("needs")
    match needs:
        case str():
            return [needs]
        case list():
            if all(isinstance(item, str) for item in needs):
                return cast(list[str], needs)
            pytest.fail("publish job needs list must contain only strings")
        case _:
            pytest.fail("publish job needs must be a string or list")


def _find_step_by_name(steps: object, name: str) -> dict[str, object] | None:
    """Find a step dict by its name."""
    match steps:
        case list():
            pass
        case _:
            pytest.fail("publish job must declare steps")

    for step in steps:
        match step:
            case {"name": step_name} if step_name == name:
                return step
            case _:
                continue
    return None


def test_publish_job_runs_even_if_build_lints_fails(workflow_text: str) -> None:
    """Ensure publish job still runs when some build-lints matrix legs fail."""
    workflow_mapping = _load_workflow_mapping(workflow_text)
    jobs = _get_job_dict(workflow_mapping, "jobs")
    publish_job = _get_job_dict(jobs, "publish")
    needs_list = _get_needs_list(publish_job)

    assert "build-lints" in needs_list, "publish job must depend on build-lints"
    assert _github_expression_value(publish_job.get("if")) == "always()", (
        "publish job must run even when build-lints has failing matrix legs"
    )

    download_step = _find_step_by_name(publish_job.get("steps"), "Download all artefacts")
    if download_step is None:
        pytest.fail("publish job must download build artefacts before publish checks")
    assert download_step.get("continue-on-error") is True, (
        "download step must continue on error so zero-artefact runs can fall "
        "through to has_assets=false"
    )


def test_manual_dispatch_exposes_force_dependency_binary_rebuild_input(
    workflow_text: str,
) -> None:
    """Ensure manual dispatch exposes an explicit dependency rebuild switch."""
    workflow_mapping = _load_workflow_mapping(workflow_text)
    inputs = _workflow_dispatch_inputs(workflow_mapping)

    input_mapping = inputs.get("force_dependency_binary_rebuild")
    assert isinstance(input_mapping, dict), (
        "workflow_dispatch must declare force_dependency_binary_rebuild input"
    )
    assert input_mapping.get("type") == "boolean", (
        "force_dependency_binary_rebuild must be a boolean workflow input"
    )
    assert input_mapping.get("required") is False, (
        "force_dependency_binary_rebuild must remain optional for manual "
        "rolling-release republishes"
    )
    assert input_mapping.get("default") is False, (
        "force_dependency_binary_rebuild must default to false so manual runs "
        "do not rebuild dependency binaries implicitly"
    )

    description = input_mapping.get("description")
    assert isinstance(description, str) and description.strip(), (
        "force_dependency_binary_rebuild must document its recovery purpose"
    )
    description_lower = description.lower()
    assert "dependency" in description_lower and "rebuild" in description_lower, (
        "force_dependency_binary_rebuild description must explain that it "
        "forces a dependency binary rebuild"
    )


def test_dependency_manifest_change_step_only_forces_manual_rebuild_when_requested(
    workflow_text: str,
) -> None:
    """Ensure manual dispatch force input gates dependency rebuilds."""
    workflow_mapping = _load_workflow_mapping(workflow_text)
    jobs = _get_job_dict(workflow_mapping, "jobs")
    change_job = _get_job_dict(jobs, "dependency-manifest-changes")
    check_step = _find_step_by_name(
        change_job.get("steps"),
        "Check whether dependency manifest changed",
    )
    assert check_step is not None, (
        "dependency-manifest-changes job must check whether the dependency "
        "manifest changed"
    )
    run_script = check_step.get("run", "")
    assert isinstance(run_script, str), "change-detection step must have a run script"

    assert "github.event.inputs.force_dependency_binary_rebuild" in run_script, (
        "change-detection step must read the manual "
        "force_dependency_binary_rebuild input"
    )
    assert (
        '[[ "${{ github.event_name }}" == "workflow_dispatch" ]]' in run_script
    ), "change-detection step must branch explicitly on workflow_dispatch"
    assert (
        '[[ "${{ github.event.inputs.force_dependency_binary_rebuild }}" == "true" ]]'
        in run_script
    ), (
        "workflow_dispatch path must only set should_build=true when the "
        "manual force input is true"
    )
    assert (
        'if [[ "${{ github.event_name }}" == "workflow_dispatch" ]]; then\n'
        '            echo "should_build=true" >> "$GITHUB_OUTPUT"'
        not in run_script
    ), (
        "workflow_dispatch must not unconditionally rebuild dependency "
        "binaries on every manual run"
    )
    assert "echo \"should_build=false\" >> \"$GITHUB_OUTPUT\"" in run_script, (
        "manual runs without force input must leave should_build=false"
    )
    assert "git diff --quiet" in run_script and "installer/dependency-binaries.toml" in run_script, (
        "push-based dependency manifest diff detection must remain in place"
    )


def test_restore_step_guards_against_missing_dependency_assets(
    workflow_text: str,
) -> None:
    """Ensure restore step checks for archive assets before downloading.

    The rolling release may contain only lint archives (no .tgz/.zip
    dependency binary assets) during bootstrapping or when dependency
    binaries have not yet been published. The restore step must probe the
    release assets, check specifically for archive file extensions, and
    skip gracefully rather than failing on a no-match download.
    """
    workflow_mapping = _load_workflow_mapping(workflow_text)
    jobs = _get_job_dict(workflow_mapping, "jobs")
    publish_job = _get_job_dict(jobs, "publish")
    restore_step = _find_step_by_name(
        publish_job.get("steps"),
        "Restore dependency archives from previous release",
    )
    assert restore_step is not None, (
        "publish job must have a 'Restore dependency archives from previous "
        "release' step"
    )
    run_script = restore_step.get("run", "")
    assert isinstance(run_script, str), "restore step must have a run script"

    assert "gh release view rolling" in run_script, (
        "restore step must probe the rolling release assets before downloading"
    )
    assert "gh release download rolling" in run_script, (
        "restore step must still download assets when they are present"
    )
    assert run_script.index("gh release view rolling") < run_script.index(
        "gh release download rolling"
    ), (
        "restore step must probe the rolling release assets before attempting "
        "to download them"
    )

    # The guard must use a case-based control flow to filter archives by
    # extension, ensuring checksum-only releases do not falsely trigger
    # a download attempt.
    assert re.search(r"case\s+[\"']?\$", run_script), (
        "restore step must use a case statement to filter asset names by "
        "extension pattern"
    )
    assert re.search(
        r"\*\.tgz\s*\|\s*\*\.zip\s*\)",
        run_script,
    ), (
        "restore step case guard must check for .tgz and .zip archive "
        "extensions specifically"
    )

    # The guard must exit 0 when no dependency assets are found rather than
    # letting gh release download fail on a no-match pattern.
    assert "exit 0" in run_script, (
        "restore step must exit cleanly when no dependency assets are found"
    )

    # Only release-not-found errors are benign; auth/network/API failures
    # must propagate so incomplete releases are not silently published.
    assert re.search(
        r"grep\s+.*\s+[\"'](release not found|no releases? found|404)",
        run_script,
        re.IGNORECASE,
    ), (
        "restore step must use grep to specifically match missing-release "
        "errors (release not found, no releases found, 404) rather than "
        "swallowing all gh failures"
    )
    assert "exit 1" in run_script, (
        "restore step must fail on unexpected gh errors (auth, network, API)"
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
