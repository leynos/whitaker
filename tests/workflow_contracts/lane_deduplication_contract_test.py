"""Execute the Rust test suite exactly once per pull request on Linux.

The coverage job is the single execution. `coverage-check` runs
`make coverage`, which reuses the `make test` recipe and swaps only the
driver to `cargo llvm-cov nextest`, so the instrumented run keeps one
package set, one feature set, and one target set. It then runs
`make test-doc`, because `cargo llvm-cov nextest` executes no doctests and
`--all-targets` excludes them. Those two steps together are one executed
set, not two lanes.

`linux-full` executes no tests at all. It survives as a job because
`main-required-checks` requires that context by name, and it carries the
formatting, spelling, Markdown, Mermaid, lint, Dylint, workflow-contract,
and packaging work. Its former uninstrumented run inside
`make publish-check` is gone.

The two remaining executions are not duplicates: `windows-compat` runs the
suite on a different platform, and `coverage-upload` runs it on `main`
pushes, never on the same event as `coverage-check`.

These tests fail if a second Linux execution appears, if the surviving gate
narrows what it executes, or if the doctest half of that gate goes missing.

Run this contract with:

```sh
make test-workflow-contracts
```
"""

from __future__ import annotations

import re
import typing as typ

from ubicloud_workflow_support import REPOSITORY_ROOT, all_jobs, load_job

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from collections.abc import Iterable

import pytest

MAKEFILE = REPOSITORY_ROOT / "Makefile"

#: The flags every Rust test invocation in CI must carry. Narrowing any of
#: them shrinks the executed set behind the single surviving Linux gate.
REQUIRED_TEST_FLAGS = ("--workspace", "--all-targets", "--all-features")

#: `make test` itself, and not the unrelated `make test-*` helper targets
#: such as `test-glibc-baseline` and `test-workflow-contracts`.
MAKE_TEST = re.compile(r"\bmake test(?![-\w])")

#: The doctest half of the single execution.
MAKE_TEST_DOC = re.compile(r"\bmake test-doc\b")

#: Commands whose presence in a job means that job executes the Rust suite.
#: `make test-doc` is deliberately absent: it is the second step of the
#: coverage lane's one executed set, not an execution of its own.
TEST_COMMANDS = (
    MAKE_TEST,
    re.compile(r"\bmake coverage\b"),
    re.compile(r"\bcargo(?:\s+\+\S+)?\s+test\b(?!\s+--doc)"),
    re.compile(r"\bnextest\s+run\b"),
    re.compile(r"\bllvm-cov\b"),
)

#: Every job permitted to execute the Rust suite, with the reason it is not
#: a second execution of the Linux coverage gate.
JUSTIFIED_TEST_LANES: dict[str, str] = {
    "coverage-check": "the single Linux pull-request execution",
    "coverage-upload": "the main-branch coverage writer, a different trigger",
    "windows-compat": "a different platform",
}

#: Both lanes that generate coverage must execute the identical set, so the
#: pull-request gate and the trunk baseline are comparable.
COVERAGE_LANES = ("coverage-check", "coverage-upload")


def _inline_scripts(job: dict[str, typ.Any]) -> str:
    """Return every inline shell script in a job, joined for substring checks.

    Unlike the shared helper, this tolerates a reusable-workflow call, which
    declares `uses` and no step list at all.
    """
    steps = job.get("steps")
    if not isinstance(steps, list):
        return ""
    return "\n".join(
        str(step.get("run", "")) for step in steps if isinstance(step, dict)
    )


def _jobs_matching(patterns: Iterable[re.Pattern[str]]) -> set[str]:
    """Return the names of jobs whose inline scripts match any pattern."""
    return {
        job_name
        for _, job_name, job in all_jobs()
        if any(pattern.search(_inline_scripts(job)) for pattern in patterns)
    }


def _make_recipe(target: str) -> str:
    """Return one Make target's recipe, blank lines included.

    Splitting on the first blank line would truncate the recipe, because Make
    permits a blank line inside one. A command placed after that line would
    then escape every assertion made about the recipe.
    """
    lines = MAKEFILE.read_text(encoding="utf-8").splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.startswith(f"{target}:")),
        None,
    )
    assert start is not None, f"{target} must be defined in the Makefile"
    body: list[str] = []
    for line in lines[start + 1 :]:
        if line and not line.startswith("\t"):
            break
        body.append(line)
    return "\n".join(body)


def _makefile_variable(name: str) -> str:
    """Return one Makefile variable's raw definition."""
    text = MAKEFILE.read_text(encoding="utf-8")
    match = re.search(rf"^{re.escape(name)} \??= (.*)$", text, re.MULTILINE)
    assert match is not None, f"{name} must be defined in the Makefile"
    return match.group(1)


def test_only_one_linux_job_runs_the_coverage_gate() -> None:
    """A second Linux coverage lane would bill twice for one result."""
    gating = {
        job_name
        for job_name in ("coverage-check", "linux-full")
        if "make coverage" in _inline_scripts(load_job(job_name))
    }
    assert gating == {"coverage-check"}, (
        f"exactly one pull-request job may run the coverage gate, got {gating}"
    )


def test_no_linux_job_adds_a_plain_test_step() -> None:
    """A test-only Linux step adds nothing over the instrumented run."""
    offenders = sorted(
        job_name
        for job_name in ("coverage-check", "linux-full", "coverage-upload")
        if MAKE_TEST.search(_inline_scripts(load_job(job_name)))
    )
    assert not offenders, (
        "the coverage lane already executes this set on Linux; "
        f"remove the redundant test step from {offenders}"
    )


def test_linux_full_executes_no_tests() -> None:
    """The lint lane carries no execution of its own, instrumented or not."""
    scripts = _inline_scripts(load_job("linux-full"))
    executing = sorted(
        pattern.pattern for pattern in TEST_COMMANDS if pattern.search(scripts)
    )
    assert not executing, (
        "linux-full must run lint, format, and packaging work only; "
        f"it still executes {executing}"
    )


def test_linux_full_and_windows_compat_remain_emitted() -> None:
    """`main-required-checks` requires both contexts by job name.

    Removing either job, or renaming its context with an explicit `name` or a
    matrix, would leave the ruleset waiting for a context that never arrives.
    """
    for job_name in ("linux-full", "windows-compat"):
        job = load_job(job_name)
        assert "name" not in job, (
            f"{job_name} must keep its job id as its required check context"
        )
        assert "strategy" not in job, (
            f"{job_name} must not become a matrix job; that renames its context"
        )


@pytest.mark.parametrize("job_name", COVERAGE_LANES)
def test_each_coverage_lane_runs_the_doctests(job_name: str) -> None:
    """Doctests are the half of the executed set nextest cannot reach."""
    assert MAKE_TEST_DOC.search(_inline_scripts(load_job(job_name))), (
        f"{job_name} must run the doctests; no other lane executes them"
    )


def test_the_doctest_lane_covers_the_whole_documented_surface() -> None:
    """The doctest flags must not narrow below the whole workspace.

    Crates that link `rustc_private` are excluded because a doctest is
    compiled as its own crate and cannot load them, not because their
    documentation is exempt.
    """
    flags = _makefile_variable("DOCTEST_CARGO_FLAGS")
    for flag in ("--workspace", "--all-features"):
        assert flag in flags, f"DOCTEST_CARGO_FLAGS must keep {flag}; got {flags!r}"


def test_publish_check_no_longer_executes_the_suite() -> None:
    """Its uninstrumented run was the second Linux execution."""
    recipe = _make_recipe("publish-check")
    assert "nextest run" not in recipe, (
        "publish-check must not re-run the suite the coverage lane executed"
    )


def test_windows_keeps_its_own_platform_lane() -> None:
    """Platform coverage is not duplication, so the Windows lane stays."""
    scripts = _inline_scripts(load_job("windows-compat"))
    assert MAKE_TEST.search(scripts), (
        "the Windows lane must keep running the suite on its own platform"
    )


def test_coverage_reuses_the_plain_test_recipe() -> None:
    """The instrumented run must not drift from the run it replaced."""
    recipe = _make_recipe("coverage")
    assert "$(MAKE) test TEST_RUNNER=" in recipe, (
        "coverage must delegate to the test recipe rather than restate it"
    )
    assert "llvm-cov nextest" in recipe, (
        "coverage must swap only the driver, not the executed set"
    )


@pytest.mark.parametrize("flag", REQUIRED_TEST_FLAGS)
def test_the_surviving_gate_executes_the_whole_workspace(flag: str) -> None:
    """The single Linux gate must keep the full package, target, feature set."""
    cargo_flags = _makefile_variable("CARGO_FLAGS")
    assert flag in cargo_flags, f"CARGO_FLAGS must keep {flag}; got {cargo_flags!r}"
    test_flags = _makefile_variable("TEST_CARGO_FLAGS")
    assert "$(CARGO_FLAGS)" in test_flags, (
        "the test flags must derive from CARGO_FLAGS so every lane agrees"
    )


def test_no_unrecorded_job_executes_the_test_suite() -> None:
    """A new test-executing lane must state why it is not a second execution."""
    unrecorded = sorted(_jobs_matching(TEST_COMMANDS) - set(JUSTIFIED_TEST_LANES))
    assert not unrecorded, (
        "each job that executes tests must be recorded with the reason it is "
        f"not a second execution of the Linux coverage gate: {unrecorded}"
    )
