"""Validate CI workflow contracts for coverage, Linux, and Windows lanes.

This module treats `.github/workflows/ci.yml` as a behavioural contract rather
than a loose implementation detail. The tests ensure Linux remains the full
validation lane, Windows remains focused on compatibility and installer smoke
coverage, and shared workflow environment variables keep caching and warnings
policy consistent across all jobs.

The tests expect `WORKFLOW_PATH` to point at a readable GitHub Actions workflow
YAML file and return normal pytest pass/fail output. Run them directly with:

```sh
PYTHONPATH=. uv run pytest tests/workflows/test_ci_workflow.py
```

In CI, these checks are covered by the `linux-full` job's workflow-test gate.
"""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from typing import Any

import pytest
from ruamel.yaml import YAML

WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"


@pytest.fixture(scope="module")
def workflow() -> dict[str, Any]:
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


def _assert_steps_in_order(
    step_names: list[str],
    required_steps: list[str],
    message: str,
) -> None:
    """Assert that the required workflow steps appear in order."""
    search_from = 0
    for required_step in required_steps:
        try:
            step_index = step_names.index(required_step, search_from)
        except ValueError:
            pytest.fail(message)
        search_from = step_index + 1


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


def test_ci_splits_linux_and_windows_jobs_by_purpose(
    workflow: Mapping[str, Any],
) -> None:
    """Ensure CI uses dedicated Linux and Windows jobs instead of a shared matrix."""
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


def test_ci_enables_shared_sccache_env_and_debug_target_cache_scope(
    workflow: Mapping[str, Any],
) -> None:
    """Ensure the workflow enables sccache and narrows cache scope to debug builds."""
    env = _get_mapping_item(workflow, "env", parent_name="CI workflow")

    assert env.get("BUILD_PROFILE") == "debug", (
        "CI must set BUILD_PROFILE=debug so shared target caching does not "
        "expand to the entire target tree"
    )
    assert env.get("SCCACHE_GHA_ENABLED") == "true", (
        "CI must enable the GitHub Actions-backed sccache integration"
    )
    assert env.get("RUSTC_WRAPPER") == "sccache", "CI must route rustc through sccache"
    assert str(env.get("CARGO_INCREMENTAL")) == "0", (
        "CI must disable incremental Cargo builds when relying on sccache"
    )
    assert env.get("RUSTFLAGS") == "-D warnings", (
        "CI must treat all Rust compiler warnings as errors via RUSTFLAGS"
    )
    assert env.get("RUSTDOCFLAGS") == "-D warnings", (
        "CI must treat all rustdoc warnings as errors via RUSTDOCFLAGS"
    )


def _assert_coverage_workflow_permissions(workflow: Mapping[str, Any]) -> None:
    """Assert the permissions required by the coverage-check contract."""
    permissions = _get_mapping_item(
        workflow,
        "permissions",
        parent_name="CI workflow",
    )
    assert permissions == {"contents": "read"}, "CI permissions must be read-only"


def _coverage_check_job(workflow: Mapping[str, Any]) -> dict[str, Any]:
    """Return the coverage job after asserting its execution contract."""
    jobs = _get_mapping_item(workflow, "jobs", parent_name="CI workflow")
    coverage_job = _get_mapping_item(
        jobs,
        "coverage-check",
        parent_name="CI workflow jobs",
    )
    assert coverage_job.get("permissions") in (None, {"contents": "read"}), (
        "coverage-check must not expand the workflow permissions"
    )
    assert coverage_job.get("if") == "github.event_name == 'pull_request'", (
        "coverage-check must run only for pull requests"
    )
    assert coverage_job.get("runs-on") == "ubicloud-standard-4-ubuntu-2404", (
        "coverage-check must use the dedicated Ubicloud Linux runner"
    )
    assert coverage_job.get("defaults", {}).get("run", {}).get("shell") == "bash", (
        "coverage-check must use Bash for Makefile targets"
    )
    assert _step_names(coverage_job) == [
        "Checkout",
        "Setup Rust",
        "Install cargo-nextest",
        "Install cargo-llvm-cov",
        "Generate coverage",
        "Check coverage against CodeScene gates",
    ], "coverage-check must contain only the approved ordered steps"
    return coverage_job


def _assert_coverage_checkout_and_setup(coverage_job: Mapping[str, Any]) -> None:
    """Assert checkout history and Rust setup for the coverage job."""
    checkout_step = _find_step(coverage_job, "Checkout")
    assert checkout_step.get("uses") == (
        "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0"
    ), "coverage-check must use the repository-approved checkout action"
    assert checkout_step.get("with", {}).get("fetch-depth") == 0, (
        "CodeScene requires full Git history for changed-line coverage"
    )
    assert checkout_step.get("with", {}).get("persist-credentials") is False, (
        "coverage-check must not retain checkout credentials"
    )

    setup_step = _find_step(coverage_job, "Setup Rust")
    assert setup_step.get("uses") == (
        "leynos/shared-actions/.github/actions/setup-rust@"
        "18bed1ca49a6de3d8882bd72635a32ae3f023d57"
    ), "coverage-check must reuse the current main-branch Rust setup pin"


def _assert_coverage_tool_installation(coverage_job: Mapping[str, Any]) -> None:
    """Assert tool pins and coverage generation for the coverage job."""
    nextest_step = _find_step(coverage_job, "Install cargo-nextest")
    llvm_cov_step = _find_step(coverage_job, "Install cargo-llvm-cov")
    installer_action = "taiki-e/install-action@db22c42b5af88356329b9a8056bb2c2f026d5a10"
    assert nextest_step.get("uses") == installer_action, (
        "cargo-nextest must use the repository-approved installer action pin"
    )
    assert nextest_step.get("with", {}).get("tool") == "nextest@0.9.114", (
        "coverage-check must install the approved cargo-nextest version"
    )
    assert llvm_cov_step.get("uses") == installer_action, (
        "cargo-llvm-cov must use the repository-approved installer action pin"
    )
    assert llvm_cov_step.get("with", {}).get("tool") == "cargo-llvm-cov@0.6.24", (
        "coverage-check must install the estate-audited cargo-llvm-cov version"
    )

    assert _find_step(coverage_job, "Generate coverage").get("run") == (
        "make coverage"
    ), "coverage-check must preserve Whitaker's crate exclusions and RUSTFLAGS"


def _assert_codescene_check(coverage_job: Mapping[str, Any]) -> None:
    """Assert the CodeScene changed-line coverage contract."""
    check_step = _find_step(coverage_job, "Check coverage against CodeScene gates")
    assert check_step.get("env", {}).get("CS_ACCESS_TOKEN") == (
        "${{ secrets.CS_ACCESS_TOKEN }}"
    ), "the CodeScene token must remain scoped to the check step"
    assert check_step.get("if") == (
        "github.event_name == 'pull_request' && env.CS_ACCESS_TOKEN != ''"
    ), "the CodeScene step must guard its pull-request secret"
    assert check_step.get("uses") == (
        "leynos/shared-actions/.github/actions/upload-codescene-coverage@"
        "18bed1ca49a6de3d8882bd72635a32ae3f023d57"
    ), "coverage-check must use the proven CodeScene action pin"
    assert check_step.get("with") == {
        "format": "lcov",
        "mode": "check",
        "project-url": "https://api.codescene.io/v2/projects/71836",
        "access-token": "${{ env.CS_ACCESS_TOKEN }}",
        "installer-checksum": "${{ vars.CODESCENE_CLI_SHA256 }}",
    }, "coverage-check must pass the canonical project and check-mode inputs"


def test_coverage_check_reuses_bespoke_whitaker_coverage_path(
    workflow: Mapping[str, Any],
) -> None:
    """Ensure the PR coverage gate preserves Whitaker's coverage constraints."""
    _assert_coverage_workflow_permissions(workflow)
    coverage_job = _coverage_check_job(workflow)
    _assert_coverage_checkout_and_setup(coverage_job)
    _assert_coverage_tool_installation(coverage_job)
    _assert_codescene_check(coverage_job)


def _assert_pinned_checkout(job: Mapping[str, Any], job_name: str) -> None:
    """Assert a validation job pins the approved checkout action without creds."""
    checkout_step = _find_step(job, "Checkout")
    assert checkout_step.get("uses") == (
        "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0"
    ), f"{job_name} must use the repository-approved pinned checkout action"
    assert checkout_step.get("with", {}).get("persist-credentials") is False, (
        f"{job_name} must not retain checkout credentials"
    )


def test_validation_jobs_pin_the_checkout_action(
    workflow: Mapping[str, Any],
) -> None:
    """linux-full and windows-compat both pin the approved checkout action."""
    jobs = _get_mapping_item(workflow, "jobs", parent_name="CI workflow")
    for job_name in ("linux-full", "windows-compat"):
        job = _get_mapping_item(jobs, job_name, parent_name="CI workflow jobs")
        _assert_pinned_checkout(job, job_name)


def test_linux_full_keeps_the_full_linux_validation_stack(
    workflow: Mapping[str, Any],
) -> None:
    """Ensure Linux remains the single lane for Linux-shaped validation work."""
    jobs = _get_mapping_item(workflow, "jobs", parent_name="CI workflow")
    linux_job = _get_mapping_item(jobs, "linux-full", parent_name="CI workflow jobs")

    _assert_steps_in_order(
        _step_names(linux_job),
        [
            "Check formatting",
            "Install bun",
            "Install Mermaid CLI",
            "Setup uv",
            "Setup Rust for Merman",
            "Install Merman CLI",
            "Install Nixie",
            "Enforce en-GB-oxendict spelling",
            "Nixie",
            "Markdown lint",
            "Lint",
            "Publish dry run",
        ],
        (
            "linux-full must remain the only CI lane that runs format, Mermaid, "
            "Nixie, Markdown lint, Clippy/doc linting, and publish-check"
        ),
    )
    assert _find_step(linux_job, "Publish dry run").get("run") == (
        'make publish-check PUBLISH_PACKAGES="whitaker-common whitaker-installer"'
    ), "linux-full must publish-check the release crates on Linux only"

    assert _find_step(linux_job, "Install Merman CLI").get("run") == (
        'cargo +1.95.0 install merman-cli --version "=0.7.0" --locked'
    ), "linux-full must install Merman 0.7.0 with its pinned Rust 1.95 toolchain"
    merman_setup = _find_step(linux_job, "Setup Rust for Merman")
    assert merman_setup.get("uses") == (
        "dtolnay/rust-toolchain@e97e2d8cc328f1b50210efc529dca0028893a2d9"
    ), "linux-full must pin the Merman Rust setup action"
    assert merman_setup.get("with", {}).get("toolchain") == "1.95.0", (
        "linux-full must pin Merman's Rust toolchain"
    )
    assert (
        _find_step(linux_job, "Enforce en-GB-oxendict spelling").get("run")
        == "make spelling"
    ), "linux-full must run the spelling gate"
    assert _find_step(linux_job, "Setup uv").get("with", {}).get("version") == "0.11.19", (
        "linux-full must use the tested uv version"
    )
    markdown_globs = (
        _find_step(linux_job, "Markdown lint")
        .get("with", {})
        .get(
            "globs",
        )
    )
    assert markdown_globs == ("**/*.md\n!**/.uv-cache/**\n!**/.uv-tools/**\n"), (
        "Markdown lint must exclude rollout-owned uv cache and tool directories"
    )


def test_windows_compat_stays_limited_to_windows_compatibility_checks(
    workflow: Mapping[str, Any],
) -> None:
    """Ensure Windows validates installer behaviour without Linux-only work."""
    jobs = _get_mapping_item(workflow, "jobs", parent_name="CI workflow")
    windows_job = _get_mapping_item(
        jobs,
        "windows-compat",
        parent_name="CI workflow jobs",
    )
    defaults = windows_job.get("defaults", {})
    if defaults.get("run", {}).get("shell") != "bash":
        pytest.fail("windows-compat must keep using Bash for POSIX make targets")

    step_names = _step_names(windows_job)
    _assert_steps_in_order(
        step_names,
        [
            "Setup Rust",
            "Install cargo-nextest",
            "Test",
            "Installer smoke test",
            "Installer release dry run",
            "Show sccache stats",
        ],
        (
            "windows-compat must stay focused on Rust tests, installer smoke "
            "coverage, installer release validation, and cache diagnostics"
        ),
    )

    linux_only_steps = {
        "Check formatting",
        "Install bun",
        "Install Mermaid CLI",
        "Setup uv",
        "Setup Rust for Merman",
        "Install Merman CLI",
        "Install Nixie",
        "Enforce en-GB-oxendict spelling",
        "Nixie",
        "Markdown lint",
        "Lint",
        "Publish dry run",
    }
    assert linux_only_steps.isdisjoint(step_names), (
        "windows-compat must not duplicate Linux-only validation work"
    )

    assert (
        _find_step(windows_job, "Test").get("run") == "make test NEXTEST_PROFILE=ci"
    ), "windows-compat must run the full CI test profile on Windows"
    assert _find_step(windows_job, "Installer smoke test").get("run") == (
        "make install-smoke"
    ), "windows-compat must install and execute the packaged installer"
    assert _find_step(windows_job, "Installer release dry run").get("run") == (
        "make release-installer-dry-run"
    ), "windows-compat must validate the host-platform installer release path"
