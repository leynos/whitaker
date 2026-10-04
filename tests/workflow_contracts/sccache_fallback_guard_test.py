"""Every step that reads the sccache statistics stands down on a fallback.

`setup-rust` reports `sccache-status=fallback` when sccache's server would not
start and the job compiles uncached (shared-actions #546). A job with no server
has no statistics, and `sccache --show-stats` prints empty defaults for it, so
a step that records, uploads or health-checks them would publish a table of
zeros that reads as a broken integration. Each such step carries the skip.

`sccache_health_contract_test` holds the three suite lanes' record, upload and
health ordering. This module is the repository-wide half: it finds the steps
by what they run, not by the job they sit in or the name they carry, so the
rolling-release build jobs and `windows-compat` are held by the same rule, and
a differently named step cannot sit outside it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest
from sccache_steps import NOT_FALLBACK, guards_against_fallback, normalized_condition
from ubicloud_workflow_support import all_jobs, job_steps

RECORD_SCRIPT: typ.Final[str] = "scripts/record-sccache-effectiveness.sh"
HEALTH_SCRIPT: typ.Final[str] = "scripts/check_sccache_health.py"
STATISTICS_FILE: typ.Final[str] = "sccache-stats.json"


def _reads_the_statistics(step: dict[str, typ.Any]) -> bool:
    """Return whether a step records, uploads or health-checks the statistics."""
    script = str(step.get("run", ""))
    inputs = step.get("with") if isinstance(step.get("with"), dict) else {}
    # `actions/upload-artifact` takes several newline-separated paths, so the
    # statistics file may be one line of a multi-line input.
    paths = str(inputs.get("path", "")).splitlines()
    return (
        RECORD_SCRIPT in script
        or HEALTH_SCRIPT in script
        or any(path.strip() == STATISTICS_FILE for path in paths)
    )


def _evidence_steps() -> list[tuple[str, dict[str, typ.Any]]]:
    """Return each statistics-reading step with a `workflow:job:step` label.

    Jobs that call a reusable workflow declare no steps and are skipped.
    """
    return [
        (f"{workflow}:{job_name}:{step.get('name')}", step)
        for workflow, job_name, job in all_jobs()
        if "steps" in job
        for step in job_steps(job)
        if _reads_the_statistics(step)
    ]


def test_the_scan_finds_the_steps_it_is_meant_to_hold() -> None:
    """The presence half: a rule over no steps passes by deleting them.

    Includes the two rolling-release recorders and the `windows-compat`
    recorder, the steps the suite-lane contract does not reach.
    """
    labels = {label.rsplit(":", 1)[0] for label, _ in _evidence_steps()}
    assert {
        "ci.yml:windows-compat",
        "rolling-release.yml:build-lints",
        "rolling-release.yml:build-dependency-binaries",
        "ci.yml:coverage-check",
        "ci.yml:linux-full",
        "coverage-main.yml:coverage-upload",
    } <= labels, sorted(labels)


@pytest.mark.parametrize(
    ("label", "step"), _evidence_steps(), ids=[label for label, _ in _evidence_steps()]
)
def test_every_statistics_step_stands_down_on_a_fallback(
    label: str, step: dict[str, typ.Any]
) -> None:
    """Each step's condition is a conjunction holding the fallback skip."""
    condition = normalized_condition(step)
    assert guards_against_fallback(condition), (
        f"{label} reads the sccache statistics but its condition {condition!r} is "
        f"not a conjunction with {NOT_FALLBACK!r}; after a fallback it would "
        "publish empty statistics for an uncached job"
    )


@pytest.mark.parametrize(
    ("condition", "expected"),
    [
        pytest.param(NOT_FALLBACK, True, id="the-guard-alone"),
        pytest.param(f"always()&&{NOT_FALLBACK}", True, id="always-and-guard"),
        pytest.param(
            f"always()&&runner.os=='Linux'&&{NOT_FALLBACK}", True, id="three-conjuncts"
        ),
        pytest.param("always()", False, id="omitted"),
        pytest.param("", False, id="no-condition"),
        pytest.param(
            "always()&&steps.setup-rust.outputs.sccache-status=='fallback'",
            False,
            id="inverted",
        ),
        pytest.param(f"!({NOT_FALLBACK})", False, id="negated"),
        pytest.param(f"always()||{NOT_FALLBACK}", False, id="disjunction-guard-last"),
        pytest.param(f"{NOT_FALLBACK}||always()", False, id="disjunction-guard-first"),
        pytest.param(
            f"always()||runner.os=='Linux'&&{NOT_FALLBACK}",
            False,
            id="disjunction-before-a-conjunction",
        ),
    ],
)
def test_the_guard_predicate_accepts_only_a_conjunction(
    condition: str, *, expected: bool
) -> None:
    """The narrow half: an omitted, inverted, negated or `||` guard is refused."""
    assert guards_against_fallback(condition) is expected


@pytest.mark.parametrize(
    ("step", "expected"),
    [
        pytest.param({"with": {"path": STATISTICS_FILE}}, True, id="scalar-path"),
        pytest.param(
            {"with": {"path": f"other.txt\n  {STATISTICS_FILE}\n"}},
            True,
            id="statistics-file-among-several-paths",
        ),
        pytest.param(
            {"with": {"path": "other.txt\nmore.txt"}}, False, id="other-paths-only"
        ),
        pytest.param({"with": {"path": f"x/{STATISTICS_FILE}"}}, False, id="subpath"),
        pytest.param({"run": f"bash {RECORD_SCRIPT}"}, True, id="record-script"),
        pytest.param({}, False, id="nothing"),
    ],
)
def test_a_statistics_step_is_recognized_by_what_it_runs_or_uploads(
    step: dict[str, typ.Any], *, expected: bool
) -> None:
    """The recognizer reads each line of a multi-line upload path.

    A single-line comparison would miss the statistics file listed beside
    another artefact, leaving that upload outside the rule.
    """
    assert _reads_the_statistics(step) is expected, f"{step!r} must be {expected}"
