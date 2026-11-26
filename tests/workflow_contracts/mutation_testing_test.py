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

The caller also passes ``--workspace``. This workspace has a root package
beside its members and sets no ``default-members``, so without the flag
cargo-mutants mutates only the root package and a changed file in a member
enumerates nothing.

The caller must reference the correct reusable workflow at a commit SHA;
Dependabot owns the SHA value, so these tests assert the shape of the pin
(the reusable workflow path, and a full 40-hex commit SHA) rather than the
specific SHA currently in force.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import fnmatch
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

#: The exact caller configuration: workspace source prefixes, the
#: scaffolding excludes (rustc proxy shims, the clippy_utils stub,
#: dylint UI fixtures, shared test infrastructure, and the vendored
#: upstream crate), and the ``--workspace`` argument that makes every
#: member mutable. The caller selects no features, so cargo-mutants tests
#: each package with its default features. Without ``--workspace``
#: cargo-mutants mutates only the root package, so a changed file in a
#: workspace member enumerates no mutants and the lane records a vacuous
#: pass.
EXPECTED_WITH = {
    "paths": "src/,cli/,common/,crates/,installer/,suite/",
    "exclude-globs": (
        "crates/rustc_*/**,crates/clippy_utils/**,crates/*/ui/**,"
        "crates/*/examples/**,crates/whitaker_lint_core/src/testing/**,"
        "common/src/test_support/**,vendor/dylint_testing/**"
    ),
    "extra-args": "--workspace",
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
    """The caller passes exactly the paths, excludes, and workspace argument."""
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

def _with_block() -> dict[str, object]:
    """Return the caller's ``with:`` mapping."""
    with_block = _mutation_job(_load()).get("with")
    assert isinstance(with_block, dict), "jobs.mutation.with is missing"
    return with_block


def _csv_setting(name: str) -> tuple[str, ...]:
    """Return one comma-separated caller setting as trimmed entries."""
    raw = _with_block().get(name)
    assert isinstance(raw, str), f"jobs.mutation.with.{name} must be a string"
    return tuple(part.strip() for part in raw.split(",") if part.strip())


def _workspace_member_dirs(root: Path) -> tuple[str, ...]:
    """Expand the root manifest's ``[workspace] members`` globs to directories.

    Only the explicitly listed members are returned. Cargo also promotes a
    member's path dependencies to members implicitly, so this is a lower
    bound on the workspace rather than an exact inventory.
    """
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    workspace = manifest.get("workspace")
    assert isinstance(workspace, dict), "the root Cargo.toml declares no [workspace]"
    members = workspace.get("members")
    assert isinstance(members, list), "the workspace declares no members"
    expanded: set[str] = set()
    for pattern in members:
        assert isinstance(pattern, str), f"workspace member {pattern!r} is not a string"
        for match in root.glob(pattern):
            if (match / "Cargo.toml").is_file():
                expanded.add(match.relative_to(root).as_posix())
    assert expanded, "no workspace member directories matched"
    return tuple(sorted(expanded))


def _under_prefix(relative_dir: str, prefix: str) -> bool:
    """Return True when ``relative_dir`` is the prefix directory or beneath it."""
    base = prefix.rstrip("/")
    return relative_dir == base or relative_dir.startswith(f"{base}/")


def _excluded_sources(member: str, root: Path, globs: tuple[str, ...]) -> bool:
    """Return True when every Rust source under the member is excluded.

    An excluded member contributes nothing to a mutation run, so it does not
    need a ``paths`` prefix; an included one must have that prefix or change
    detection can never select it.
    """
    sources = [
        path.relative_to(root).as_posix()
        for path in (root / member / "src").rglob("*.rs")
    ]
    if not sources:
        return False
    return all(
        any(fnmatch.fnmatchcase(source, glob) for glob in globs) for source in sources
    )


def _members_invisible_to_detection(
    members: tuple[str, ...], prefixes: tuple[str, ...], globs: tuple[str, ...]
) -> list[str]:
    """Return members that are neither detected nor deliberately excluded."""
    return [
        member
        for member in members
        if not any(_under_prefix(member, prefix) for prefix in prefixes)
        and not _excluded_sources(member, REPOSITORY_ROOT, globs)
    ]


def test_every_workspace_member_is_detected_or_excluded() -> None:
    """No member is invisible to change detection.

    ``paths`` decides which changed files schedule a scoped run, and
    ``--workspace`` decides which packages that run can mutate. A member in
    neither the prefixes nor the excludes is silently unreachable: its changes
    schedule nothing, or reach the run and enumerate nothing. Deriving the
    member list from the root manifest is what makes a newly added member fail
    here rather than in a scheduled run months later.
    """
    members = _workspace_member_dirs(REPOSITORY_ROOT)
    invisible = _members_invisible_to_detection(
        members, _csv_setting("paths"), _csv_setting("exclude-globs")
    )
    assert not invisible, (
        "every workspace member must appear in jobs.mutation.with.paths, or "
        "have its sources covered by exclude-globs; these are in neither: "
        f"{invisible}"
    )


def test_the_detection_guard_reports_a_missing_member() -> None:
    """The guard is not vacuous: an uncovered member is reported.

    Without this, a ``paths`` value that covered everything would let the
    coverage test above pass while checking nothing, which is the same class
    of vacuous green the caller itself was fixed for.
    """
    members = _workspace_member_dirs(REPOSITORY_ROOT)
    all_prefixes = tuple(f"{member}/" for member in members)
    assert _members_invisible_to_detection(members, all_prefixes, ()) == [], (
        "a prefix per member must cover every member"
    )
    trimmed = tuple(prefix for prefix in all_prefixes if not prefix.startswith("cli/"))
    globs = _csv_setting("exclude-globs")
    assert "cli" in _members_invisible_to_detection(members, trimmed, globs), (
        "dropping the cli/ prefix must report cli as invisible"
    )


def test_root_package_sources_are_detected() -> None:
    """A root package's ``src/`` is a member prefix in its own right.

    The root package sits at the repository root, so it has no member
    directory for the coverage test to check. Its sources are reachable only
    because ``src/`` is listed, and a root-package-only change would otherwise
    schedule nothing.
    """
    prefixes = _csv_setting("paths")
    if any((REPOSITORY_ROOT / "src").rglob("*.rs")):
        assert "src/" in prefixes, (
            "the root package ships src/ Rust sources, so jobs.mutation.with.paths "
            f"must list 'src/', got {prefixes}"
        )
    for prefix in prefixes:
        target = REPOSITORY_ROOT / prefix
        assert target.exists(), (
            f"jobs.mutation.with.paths entry {prefix!r} matches nothing in the "
            "repository, so it detects no changes"
        )
