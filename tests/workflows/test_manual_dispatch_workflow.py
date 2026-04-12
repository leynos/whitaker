"""Validate rolling-release manual-dispatch workflow contracts."""

from __future__ import annotations

import re

import pytest

from tests.workflows.rolling_release_workflow_test_support import (
    _find_step_by_name,
    _get_job_dict,
    _load_workflow_mapping,
    _workflow_dispatch_branch_body,
    _workflow_dispatch_inputs,
)


def test_manual_dispatch_exposes_force_dependency_binary_rebuild_input(
    workflow_text: str,
) -> None:
    """Ensure manual dispatch exposes an explicit dependency rebuild switch."""
    workflow_mapping = _load_workflow_mapping(workflow_text)
    inputs = _workflow_dispatch_inputs(workflow_mapping)

    match inputs.get("force_dependency_binary_rebuild"):
        case dict() as input_mapping:
            pass
        case _:
            pytest.fail(
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

    match input_mapping.get("description"):
        case str() as description if description.strip():
            pass
        case _:
            pytest.fail(
                "force_dependency_binary_rebuild must document its recovery purpose"
            )

    description_lower = description.lower()
    assert "dependency" in description_lower, (
        "force_dependency_binary_rebuild description must explain that it "
        "forces a dependency binary rebuild"
    )
    assert "rebuild" in description_lower, (
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
    assert re.search(
        r'\[\[\s+"\$\{\{\s*github\.event_name\s*\}\}"\s+==\s+"workflow_dispatch"\s+\]\]',
        run_script,
    ), "change-detection step must branch explicitly on workflow_dispatch"
    assert (
        '[[ "${{ github.event.inputs.force_dependency_binary_rebuild }}" == "true" ]]'
        in run_script
    ), (
        "workflow_dispatch path must only set should_build=true when the "
        "manual force input is true"
    )

    dispatch_branch = _workflow_dispatch_branch_body(run_script)
    assert re.match(
        r'\s*echo\s+"should_build=true"\s+>>\s+"\$GITHUB_OUTPUT"\s*$',
        dispatch_branch,
    ) is None, (
        "workflow_dispatch must not unconditionally rebuild dependency "
        "binaries on every manual run"
    )
    assert "echo \"should_build=false\" >> \"$GITHUB_OUTPUT\"" in dispatch_branch, (
        "manual runs without force input must leave should_build=false"
    )
    assert "git diff --quiet" in run_script, (
        "push-based dependency manifest diff detection must remain in place"
    )
    assert "installer/dependency-binaries.toml" in run_script, (
        "push-based dependency manifest diff detection must remain in place"
    )
