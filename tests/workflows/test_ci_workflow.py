"""Validate CI workflow contracts for Linux and Windows lanes."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from typing import Any

import pytest
from ruamel.yaml import YAML

WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"


def _load_workflow_mapping() -> dict[str, Any]:
    """Load the CI workflow YAML and return it as a mapping."""
    parsed = YAML(typ="safe").load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    match parsed:
        case dict() as workflow_mapping:
            return workflow_mapping
        case _:
            pytest.fail("CI workflow must parse to a mapping")


def _get_mapping_item(
    mapping: Mapping[str, Any],
    key: str,
    *,
    parent_name: str,
) -> dict[str, Any]:
    """Return a child mapping and fail with a targeted contract message."""
    match mapping.get(key):
        case dict() as child_mapping:
            return child_mapping
        case _:
            pytest.fail(f"{parent_name} must declare {key}")


def _step_names(job: Mapping[str, Any]) -> list[str]:
    """Return ordered workflow step names for the provided job mapping."""
    match job.get("steps"):
        case list() as steps:
            pass
        case _:
            pytest.fail("CI job must declare steps")

    names: list[str] = []
    for step in steps:
        match step:
            case {"name": str() as step_name}:
                names.append(step_name)
            case {"uses": str()}:
                pytest.fail("CI workflow steps must be named for contract tests")
            case _:
                pytest.fail("CI workflow step must be a mapping with a name")
    return names


def _find_step(job: Mapping[str, Any], name: str) -> dict[str, Any]:
    """Return a named workflow step from the job mapping."""
    match job.get("steps"):
        case list() as steps:
            pass
        case _:
            pytest.fail("CI job must declare steps")

    for step in steps:
        match step:
            case {"name": str() as step_name} if step_name == name:
                return step
            case _:
                continue
    pytest.fail(f"CI job must include {name!r} step")


def test_ci_splits_linux_and_windows_jobs_by_purpose() -> None:
    """Ensure CI uses dedicated Linux and Windows jobs instead of a shared matrix."""
    workflow = _load_workflow_mapping()
    jobs = _get_mapping_item(workflow, "jobs", parent_name="CI workflow")

    assert "build-test" not in jobs, (
        "CI workflow must no longer use the shared build-test matrix job"
    )

    linux_job = _get_mapping_item(jobs, "linux-full", parent_name="CI workflow jobs")
    windows_job = _get_mapping_item(
        jobs,
        "windows-compat",
        parent_name="CI workflow jobs",
    )

    assert linux_job.get("runs-on") == "ubicloud-standard-4-ubuntu-2404", (
        "linux-full must run on the dedicated Ubicloud Linux runner"
    )
    assert windows_job.get("runs-on") == "windows-latest", (
        "windows-compat must keep using the hosted Windows runner"
    )
    assert "strategy" not in linux_job, (
        "linux-full should be a purpose-built job, not a matrix include row"
    )
    assert "strategy" not in windows_job, (
        "windows-compat should be a purpose-built job, not a matrix include row"
    )


def test_ci_enables_shared_sccache_env_and_debug_target_cache_scope() -> None:
    """Ensure the workflow enables sccache and narrows cache scope to debug builds."""
    workflow = _load_workflow_mapping()
    env = _get_mapping_item(workflow, "env", parent_name="CI workflow")

    assert env.get("BUILD_PROFILE") == "debug", (
        "CI must set BUILD_PROFILE=debug so shared target caching does not "
        "expand to the entire target tree"
    )
    assert env.get("SCCACHE_GHA_ENABLED") == "true", (
        "CI must enable the GitHub Actions-backed sccache integration"
    )
    assert env.get("RUSTC_WRAPPER") == "sccache", (
        "CI must route rustc through sccache"
    )
    assert str(env.get("CARGO_INCREMENTAL")) == "0", (
        "CI must disable incremental Cargo builds when relying on sccache"
    )


def test_linux_full_keeps_the_full_linux_validation_stack() -> None:
    """Ensure Linux remains the single lane for Linux-shaped validation work."""
    workflow = _load_workflow_mapping()
    jobs = _get_mapping_item(workflow, "jobs", parent_name="CI workflow")
    linux_job = _get_mapping_item(jobs, "linux-full", parent_name="CI workflow jobs")

    assert _step_names(linux_job) == [
        "Checkout",
        "Setup Rust",
        "Install cargo-nextest",
        "Check formatting",
        "Install bun",
        "Install Mermaid CLI",
        "Setup uv",
        "Install Nixie",
        "Nixie",
        "Markdown lint",
        "Lint",
        "Publish dry run",
    ], (
        "linux-full must remain the only CI lane that runs format, Mermaid, "
        "Nixie, Markdown lint, Clippy/doc linting, and publish-check"
    )

    assert _find_step(linux_job, "Publish dry run").get("run") == "make publish-check", (
        "linux-full must keep publish-check on Linux only"
    )


def test_windows_compat_stays_limited_to_windows_compatibility_checks() -> None:
    """Ensure Windows validates build and installer behaviour without Linux-only work."""
    workflow = _load_workflow_mapping()
    jobs = _get_mapping_item(workflow, "jobs", parent_name="CI workflow")
    windows_job = _get_mapping_item(
        jobs,
        "windows-compat",
        parent_name="CI workflow jobs",
    )

    assert _step_names(windows_job) == [
        "Checkout",
        "Setup Rust",
        "Install cargo-nextest",
        "Test",
        "Installer release dry run",
        "Show sccache stats",
    ], (
        "windows-compat must stay focused on Rust tests, installer release "
        "validation, and cache diagnostics"
    )

    assert _find_step(windows_job, "Test").get("run") == "make test NEXTEST_PROFILE=ci", (
        "windows-compat must run the full CI test profile on Windows"
    )
    assert _find_step(windows_job, "Installer release dry run").get("run") == (
        "make release-installer-dry-run"
    ), "windows-compat must validate the host-platform installer release path"
