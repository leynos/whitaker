"""Lane discovery driven with workflows this repository does not have.

The tree carries a small, uniform set of suite-running jobs, so the
contract's own assertions cannot tell a correct discovery from one that
happens to agree with the files. These pass parsed documents straight
into the discovery, which is why the file reading sits at a boundary
the query functions do not reach through.
"""

from __future__ import annotations

import pathlib
import typing as typ

import pytest
from suite_lanes import (
    WorkflowLoadError,
    _declared_jobs,
    _job_ceiling,
    _lanes_in_job,
    _workflow_documents,
)


def _document(**jobs: dict[str, typ.Any]) -> dict[str, dict[str, typ.Any]]:
    """Return one workflow document named ``ci.yml`` holding those jobs."""
    return {"ci.yml": {"jobs": dict(jobs)}}


def test_a_job_without_a_ceiling_is_still_a_lane() -> None:
    """A missing `timeout-minutes` must read as a lane with no budget.

    Dropping the job instead would turn the state this contract exists
    to detect into silence: the Windows lane's absent ceiling went
    unnoticed exactly that way.
    """
    documents = _document(unbounded={"steps": [{"name": "Test", "run": "make test"}]})
    (job,) = _declared_jobs(documents)
    (lane,) = _lanes_in_job(job)
    assert _job_ceiling(job) is None
    assert lane.job_timeout is None, "a job declaring no ceiling has no budget"
    assert lane.command == "make test"


def test_a_step_running_the_suite_twice_is_two_lanes() -> None:
    """Two invocations under two profiles are two lanes with two budgets."""
    documents = _document(
        both={
            "timeout-minutes": 80,
            "steps": [
                {
                    "name": "Test",
                    "run": "make coverage\nmake test NEXTEST_PROFILE=ci",
                }
            ],
        }
    )
    (job,) = _declared_jobs(documents)
    lanes = _lanes_in_job(job)
    assert [lane.command for lane in lanes] == [
        "make coverage",
        "make test NEXTEST_PROFILE=ci",
    ]
    assert {lane.job_timeout for lane in lanes} == {4800.0}


def test_a_malformed_job_is_skipped_rather_than_raising() -> None:
    """A job that is not a mapping is not a lane, and not a crash either.

    A workflow can hold anything the YAML parser accepts, and a
    contract that raised on one would report a parse failure where the
    question was whether a lane is bounded.
    """
    documents = {"ci.yml": {"jobs": {"broken": "not a mapping"}}}
    assert _declared_jobs(documents) == ()


def test_a_jobs_key_that_is_not_a_mapping_is_skipped_too() -> None:
    """`jobs: [broken]` is valid YAML, and must not raise on the way past.

    The value is normalized before iteration, so a workflow whose
    `jobs` is a list reports no lanes rather than an `AttributeError`
    from a contract that was asked about ceilings.
    """
    assert _declared_jobs({"ci.yml": {"jobs": ["broken"]}}) == ()
    assert _declared_jobs({"ci.yml": {"jobs": "broken"}}) == ()


@pytest.mark.parametrize(
    ("minutes", "expected"),
    [
        pytest.param(80, 4800.0, id="the-documented-ceiling"),
        pytest.param(1, 60.0, id="one-minute"),
        pytest.param(0, 0.0, id="zero"),
    ],
)
def test_a_ceiling_converts_from_minutes(minutes: int, expected: float) -> None:
    """`timeout-minutes` is minutes; every comparison here is in seconds."""
    documents = _document(bounded={"timeout-minutes": minutes, "steps": []})
    (job,) = _declared_jobs(documents)
    assert _job_ceiling(job) == pytest.approx(expected)


def test_an_unreadable_workflow_fails_by_name(tmp_path: pathlib.Path) -> None:
    """A workflow the contract cannot read is not a repository with fewer lanes.

    Skipping it would leave the discovery reporting exactly what a
    repository with no such workflow reports, so a file that stopped
    parsing would look like a lane somebody deleted.
    """
    (tmp_path / "broken.yml").write_text("jobs: [unclosed\n", encoding="utf-8")
    with pytest.raises(WorkflowLoadError, match=r"broken\.yml"):
        _workflow_documents(tmp_path)


def test_a_directory_of_valid_workflows_reads_both_extensions(
    tmp_path: pathlib.Path,
) -> None:
    """A lane in the other extension would otherwise escape every assertion."""
    (tmp_path / "one.yml").write_text("jobs: {a: {steps: []}}\n", encoding="utf-8")
    (tmp_path / "two.yaml").write_text("jobs: {b: {steps: []}}\n", encoding="utf-8")
    assert sorted(_workflow_documents(tmp_path)) == ["one.yml", "two.yaml"]
