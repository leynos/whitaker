"""Contract tests for the mutation-testing caller workflow.

The executable logic lives in the ``leynos/shared-actions`` reusable
workflow, which carries its own unit and integration tests; whitaker's
caller is declarative configuration. These tests parse the caller with
PyYAML and pin the contract it must uphold, so drift (repointing the pin
at a branch, widening permissions, or losing the workspace paths,
scaffolding excludes, or feature selection) fails CI on the pull request
rather than surfacing in a scheduled or manual run.

The caller mutates under the packages' default features. Passing
``--all-features`` enabled ``dylint-driver`` across the workspace, Cargo
unified it into the lint crates' integration-test binaries, and those
binaries cannot link the compiler's ``rustc_private`` libraries: every
shard of run 33961579896 failed its baseline with 19 duplicate-dependency
errors before testing a mutant. The guard below rejects that flag, and any
other spelling that reaches it, because the failure is invisible until a
test binary links.

The caller must reference the correct reusable workflow at a commit SHA;
Dependabot owns the SHA value, so these tests assert the shape of the pin
(the reusable workflow path, and a full 40-hex commit SHA) rather than the
specific SHA currently in force.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import shlex
import tomllib
from pathlib import Path

import pytest
from mutation_features import driver_selections, feature_tokens
from ubicloud_workflow_support import REPOSITORY_ROOT, parse_workflow

WORKFLOW_PATH = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / "mutation-testing.yml"
)

USES_RE = re.compile(
    r"^leynos/shared-actions/\.github/workflows/mutation-cargo\.yml@[0-9a-f]{40}$"
)

#: The exact caller configuration: workspace source prefixes and the
#: scaffolding excludes (rustc proxy shims, the clippy_utils stub,
#: dylint UI fixtures, and shared test infrastructure). No `extra-args`
#: key, so cargo-mutants selects each package's default features.
EXPECTED_WITH = {
    "paths": "src/,common/,crates/,installer/,suite/",
    "exclude-globs": (
        "crates/rustc_*/**,crates/clippy_utils/**,crates/*/ui/**,"
        "crates/*/examples/**,crates/whitaker_lint_core/src/testing/**,"
        "common/src/test_support/**"
    ),
}

#: The root manifest whose default feature set the caller must respect.
ROOT_MANIFEST = REPOSITORY_ROOT / "Cargo.toml"


def _load() -> dict[str, object]:
    """Parse the workflow file."""
    return parse_workflow(WORKFLOW_PATH.read_text(encoding="utf-8"))


def _triggers(workflow: dict[str, object]) -> dict[str, object]:
    """Return the ``on:`` mapping (PyYAML parses the bare key as True)."""
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "the workflow must declare an on: mapping"
    return triggers


def _mutation_job(workflow: dict[str, object]) -> dict[str, object]:
    """Return the single calling job."""
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the workflow must declare a jobs mapping"
    assert jobs, "the workflow must declare at least one job"
    assert list(jobs) == ["mutation"], (
        f"expected a single job named 'mutation', found {sorted(jobs)}"
    )
    return jobs["mutation"]


def test_uses_reference_is_pinned_to_a_commit_sha() -> None:
    """The job must call the correct reusable workflow at a commit SHA.

    Dependabot owns the SHA value, so this asserts the shape of the pin
    (reusable workflow path plus a 40-hex commit SHA) rather than the
    specific SHA currently in force.
    """
    uses = _mutation_job(_load()).get("uses")
    assert uses is not None, "jobs.mutation.uses is missing"
    assert USES_RE.match(uses), (
        "jobs.mutation.uses must reference mutation-cargo.yml pinned to a "
        f"full 40-character lowercase hex commit SHA, not a branch or tag: "
        f"{uses!r}"
    )


def test_job_permissions_are_exactly_least_privilege() -> None:
    """The job grants contents: read and id-token: write, nothing broader."""
    permissions = _mutation_job(_load()).get("permissions")
    assert permissions == {"contents": "read", "id-token": "write"}, (
        "jobs.mutation.permissions must be exactly "
        f"{{'contents': 'read', 'id-token': 'write'}}, got {permissions!r}"
    )


def test_workflow_default_permissions_are_empty() -> None:
    """The workflow-level default token scope is empty."""
    workflow = _load()
    assert workflow.get("permissions") == {}, (
        f"top-level permissions must be an empty mapping, got "
        f"{workflow.get('permissions')!r}"
    )


def test_concurrency_serializes_per_ref_without_cancelling() -> None:
    """Runs queue per ref instead of cancelling one another."""
    concurrency = _load().get("concurrency")
    assert isinstance(concurrency, dict), "the workflow must declare concurrency"
    assert concurrency.get("group") == "mutation-testing-${{ github.ref }}", (
        f"concurrency.group must key on the triggering ref, got "
        f"{concurrency.get('group')!r}"
    )
    assert concurrency.get("cancel-in-progress") is False, (
        f"concurrency.cancel-in-progress must be false, got "
        f"{concurrency.get('cancel-in-progress')!r}"
    )


def test_triggers_keep_schedule_and_plain_dispatch() -> None:
    """The daily schedule stays; dispatch has no legacy branch input."""
    triggers = _triggers(_load())
    schedule = triggers.get("schedule")
    assert schedule == [{"cron": "50 4 * * *"}], (
        f"on.schedule must be the daily 04:50 UTC cron, got {schedule!r}"
    )
    assert "workflow_dispatch" in triggers, "on.workflow_dispatch is missing"
    dispatch = triggers.get("workflow_dispatch") or {}
    inputs = dispatch.get("inputs") or {}
    assert "branch" not in inputs, (
        "on.workflow_dispatch must not declare a branch input; the Actions "
        "run-workflow control selects the ref"
    )


def test_with_block_carries_the_caller_configuration() -> None:
    """The caller passes exactly the paths and excludes, and no feature args."""
    with_block = _mutation_job(_load()).get("with")
    assert isinstance(with_block, dict), "jobs.mutation.with is missing"
    assert with_block == EXPECTED_WITH, (
        "jobs.mutation.with must be exactly the documented configuration "
        f"{EXPECTED_WITH!r}, got {with_block!r}"
    )


def test_caller_never_selects_the_dylint_driver() -> None:
    """The caller mutates the packages' default features.

    `--all-features` enabled the driver on every lint crate, and their
    integration-test binaries cannot link `rustc_private`; the shared
    workflow's baseline then fails before any mutant is tested. The guard is
    here because nothing in CI links those binaries, so the flag's effect is
    invisible until a scheduled run.
    """
    with_block = _mutation_job(_load()).get("with")
    assert isinstance(with_block, dict), "jobs.mutation.with is missing"
    extra_args = str(with_block.get("extra-args", ""))
    violations = driver_selections(shlex.split(extra_args))
    assert not violations, (
        "jobs.mutation.with.extra-args must not select the dylint-driver "
        f"feature; {', '.join(violations)}. See `make mutation-baseline` for "
        "the baseline this selection must keep linking."
    )


@pytest.mark.parametrize(
    ("extra_args", "is_rejected"),
    [
        pytest.param("", False, id="no-extra-arguments"),
        pytest.param("--all-features", True, id="all-features"),
        pytest.param("--features dylint-driver", True, id="features-separated"),
        pytest.param("--features=dylint-driver", True, id="features-attached"),
        pytest.param(
            "--features other,dylint-driver", True, id="features-in-a-list"
        ),
        pytest.param("-F dylint-driver", True, id="short-feature-flag"),
        pytest.param("-Fdylint-driver", True, id="short-feature-flag-attached"),
        pytest.param("-C '--all-features'", True, id="cargo-argument"),
        pytest.param("-C--all-features", True, id="cargo-argument-attached"),
        pytest.param(
            "--cargo-arg=--features=dylint-driver", True, id="cargo-argument-long"
        ),
        pytest.param(
            "--cargo-test-arg=--all-features", True, id="cargo-test-argument"
        ),
        pytest.param("--features an-unrelated-feature", False, id="other-feature"),
    ],
)
def test_feature_selection_guard_rejects_driver_enabling_arguments(
    extra_args: str, *, is_rejected: bool
) -> None:
    """Every spelling that reaches the driver feature is rejected."""
    rejected = bool(driver_selections(shlex.split(extra_args)))
    assert rejected is is_rejected, (
        f"{extra_args!r} must read as "
        f"{'selecting' if is_rejected else 'not selecting'} dylint-driver"
    )


@pytest.mark.parametrize(
    ("extra_args", "expected"),
    [
        pytest.param("", [], id="no-selection"),
        pytest.param(
            "--no-default-features",
            ["--no-default-features"],
            id="default-features-off",
        ),
        pytest.param("--features other", ["--features other"], id="named-feature"),
    ],
)
def test_feature_tokens_distinguish_the_default_selection(
    extra_args: str, expected: list[str]
) -> None:
    """Turning defaults off is a selection, not the empty default set.

    The baseline guard compares feature tokens for parity, so reading
    ``--no-default-features`` as no selection would let a caller and recipe
    disagree about the default set without failing.
    """
    assert feature_tokens(shlex.split(extra_args)) == expected, (
        f"{extra_args!r} must read as {expected!r}"
    )


def test_root_default_features_exclude_the_dylint_driver() -> None:
    """The root package's default features are what the caller runs.

    The root `whitaker` package currently declares no features at all. The
    assertion is about `default`, not the table's presence, so a later default
    that pulls the driver in fails here rather than in a nightly run.
    """
    manifest = tomllib.loads(ROOT_MANIFEST.read_text(encoding="utf-8"))
    features = manifest.get("features", {})
    assert isinstance(features, dict), "[features] must be a table"
    default = features.get("default", [])
    assert isinstance(default, list), "features.default must be a list"
    assert "dylint-driver" not in default, (
        "the root package's default features must not enable dylint-driver; "
        f"its test binaries cannot link rustc_private. Got {default!r}"
    )
