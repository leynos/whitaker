"""The coverage lanes' time limits are sized from a cold compiler cache.

A pull request's first push can read only its own cache scope and `main`'s, so
either coverage lane may run cold. The cold runs took 35m54s in `Generate
coverage` alone (35825656071, cancelled at the old 40-minute limit) and 38m45s
end to end (35825720438), against 12m50s warm, so a limit of 40 left about a
minute spare. `COLD_RUN_TIME_LIMIT_MINUTES` is about one and a half times the
slowest cold job; "Coverage lane time limits" in the developers' guide records
the figures. The limit is asserted exactly, because a broad bound would pass
with the old value restored.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from ubicloud_workflow_support import load_job

#: The reviewed limit, in minutes, for a lane that may compile from cold.
COLD_RUN_TIME_LIMIT_MINUTES: typ.Final[int] = 60

#: The lanes that run `make coverage`, and so may pay a cold instrumented build.
COVERAGE_LANES: typ.Final[tuple[str, ...]] = ("coverage-check", "coverage-upload")


def time_limit_violations(job: dict[str, typ.Any]) -> list[str]:
    """Return why a job's `timeout-minutes` is not the cold-run limit.

    >>> time_limit_violations({"timeout-minutes": 60})
    []
    >>> time_limit_violations({})
    ['timeout-minutes is missing, so the job runs to the six-hour default']
    """
    if "timeout-minutes" not in job:
        return ["timeout-minutes is missing, so the job runs to the six-hour default"]
    limit = job["timeout-minutes"]
    if limit != COLD_RUN_TIME_LIMIT_MINUTES:
        return [
            (
                f"timeout-minutes is {limit!r}, not the cold-run limit "
                f"{COLD_RUN_TIME_LIMIT_MINUTES}"
            )
        ]
    return []


@pytest.mark.parametrize("lane", COVERAGE_LANES)
def test_each_coverage_lane_has_the_cold_run_limit(lane: str) -> None:
    """Both coverage lanes may run cold, so both carry the sized limit."""
    violations = time_limit_violations(load_job(lane))
    assert not violations, f"{lane}: {violations}"


@pytest.mark.parametrize(
    ("job", "expected"),
    [
        pytest.param({"timeout-minutes": 40}, "is 40", id="the-old-limit"),
        pytest.param({}, "is missing", id="no-limit"),
        pytest.param({"timeout-minutes": 90}, "is 90", id="a-wider-limit"),
        pytest.param({"timeout-minutes": "60"}, "is '60'", id="a-string"),
    ],
)
def test_a_limit_other_than_the_cold_run_limit_is_refused(
    job: dict[str, typ.Any], expected: str
) -> None:
    """The old 40, no limit, and any other value each fail."""
    violations = time_limit_violations(job)
    assert any(expected in violation for violation in violations), violations
