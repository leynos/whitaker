"""Keep one Linux gate for the Rust test suite, and keep it complete.

A coverage job and a test-only job on the same platform bill twice for one
executed set. Whitaker's pull-request gate therefore runs the suite once on
Linux, inside `coverage-check`, and the instrumented run stays in lockstep
with the plain run it replaced: `make coverage` reuses the `make test`
recipe and swaps only the driver, so both share one package set, one
feature set, and one target set.

The other lanes that execute tests are not duplicates of it:

- `windows-compat` runs the suite on a different platform;
- `linux-full` runs it once more inside `make publish-check`, with
  production-like static linking rather than the `prefer-dynamic` flags the
  coverage lane needs for its `cdylib` lint crates; and
- `coverage-upload` runs on `main` pushes, never on the same event as
  `coverage-check`.

These tests fail if a Linux test-only lane reappears, if the surviving gate
narrows what it executes, or if a new job starts running tests without a
recorded reason.

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

#: Commands whose presence in a job means that job executes the Rust suite.
TEST_COMMANDS = (
    MAKE_TEST,
    re.compile(r"\bmake coverage\b"),
    re.compile(r"\bmake publish-check\b"),
    re.compile(r"\bcargo(?:\s+\+\S+)?\s+test\b"),
    re.compile(r"\bnextest\s+run\b"),
    re.compile(r"\bllvm-cov\b"),
)

#: Every job permitted to execute the Rust suite, with the reason it is not
#: a duplicate of the Linux coverage gate.
JUSTIFIED_TEST_LANES: dict[str, str] = {
    "coverage-check": "the Linux pull-request gate",
    "coverage-upload": "the main-branch coverage writer, a different trigger",
    "windows-compat": "a different platform",
    "linux-full": "production-like static linking inside publish-check",
}


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


def test_windows_keeps_its_own_platform_lane() -> None:
    """Platform coverage is not duplication, so the Windows lane stays."""
    scripts = _inline_scripts(load_job("windows-compat"))
    assert MAKE_TEST.search(scripts), (
        "the Windows lane must keep running the suite on its own platform"
    )


def test_coverage_reuses_the_plain_test_recipe() -> None:
    """The instrumented run must not drift from the run it replaced."""
    text = MAKEFILE.read_text(encoding="utf-8")
    recipe = text.split("\ncoverage:", 1)[1].split("\n\n", 1)[0]
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
    """A new test-executing lane must state why it is not a duplicate."""
    unrecorded = sorted(_jobs_matching(TEST_COMMANDS) - set(JUSTIFIED_TEST_LANES))
    assert not unrecorded, (
        "each job that executes tests must be recorded with the reason it is "
        f"not a duplicate of the Linux coverage gate: {unrecorded}"
    )
