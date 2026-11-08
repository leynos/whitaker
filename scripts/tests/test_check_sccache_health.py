"""Test the sccache health checker the gha lanes run after recording statistics.

Run with ``make test-sccache-health``.
"""

from __future__ import annotations

import importlib.util
import json
import types
import typing as typ
from fractions import Fraction
from pathlib import Path

import pytest
from hypothesis import given
from hypothesis import strategies as st

SCRIPTS = Path(__file__).resolve().parents[1]


@pytest.fixture(scope="module")
def checker() -> types.ModuleType:
    """Import the health checker script as a module."""
    spec = importlib.util.spec_from_file_location(
        "check_sccache_health", SCRIPTS / "check_sccache_health.py"
    )
    if spec is None or spec.loader is None:
        message = "could not load the sccache health checker"
        raise ImportError(message)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _document(
    location: str = "ghac, name: sccache-v0.17.0", **stats: object
) -> dict[str, object]:
    """Return a statistics document shaped like `sccache --stats-format json`.

    The defaults are the healthy shape of run 35815203116's `coverage-check`.
    """
    healthy: dict[str, object] = {
        "compile_requests": 3256,
        "cache_hits": {"counts": {"Rust": 1505, "C/C++": 912}},
        "cache_misses": {"counts": {"Rust": 69}},
        "cache_writes": 69,
        "cache_write_errors": 0,
        "cache_read_errors": 0,
        "cache_timeouts": 0,
        "cache_errors": {"counts": {}},
    }
    return {"cache_location": location, "stats": healthy | stats}


def test_a_healthy_lane_passes_quietly(checker: types.ModuleType) -> None:
    """The measured warm run: no failure and no warning."""
    assessment = checker.assess(_document(), "ghac")
    assert assessment.failures == ()
    assert assessment.warnings == ()


@pytest.mark.parametrize(
    ("document", "expected"),
    [
        pytest.param(
            _document(location='Local disk: "/home/runner/.cache/sccache"'),
            "not the 'ghac' backend",
            id="bound-local-disk",
        ),
        pytest.param(
            _document(compile_requests=0), "wrapped nothing", id="wrapped-nothing"
        ),
        pytest.param(
            _document(cache_writes=0, cache_write_errors=3788),
            "every store failed",
            id="every-store-failed",
        ),
        pytest.param(
            _document(cache_hits={"counts": {}}, cache_read_errors=12),
            "every read failed",
            id="every-read-failed",
        ),
    ],
)
def test_a_broken_integration_fails(
    checker: types.ModuleType, document: dict[str, object], expected: str
) -> None:
    """Each structural signature fails the lane on its own."""
    failures = checker.assess(document, "ghac").failures
    assert any(expected in failure for failure in failures), failures


@pytest.mark.parametrize(
    ("stats", "expected"),
    [
        pytest.param(
            {"cache_write_errors": 1}, "cache_write_errors is 1", id="one-write-error"
        ),
        pytest.param(
            {"cache_read_errors": 2}, "cache_read_errors is 2", id="read-errors"
        ),
        pytest.param({"cache_timeouts": 1}, "cache_timeouts is 1", id="a-timeout"),
        pytest.param(
            {"cache_errors": {"counts": {"Rust": 2}}},
            "cache_errors is 2",
            id="cache-errors",
        ),
    ],
)
def test_isolated_errors_warn_without_failing(
    checker: types.ModuleType, stats: dict[str, object], expected: str
) -> None:
    """A proxy hiccup costs a compile, not a red pull request.

    The first cold run on this backend had one write error against 1,572
    misses; failing on that would make the lane as flaky as the service it
    talks to.
    """
    assessment = checker.assess(_document(**stats), "ghac")
    assert assessment.failures == ()
    assert expected in assessment.warnings


#: The cold run 35672193433's `coverage-check`: one write error and one timeout
#: against 1,572 store attempts, the measured healthy rate of about 0.1%.
MEASURED_COLD_RUN: typ.Final[dict[str, object]] = {
    "compile_requests": 3333,
    "cache_hits": {"counts": {"Rust": 1073}},
    "cache_misses": {"counts": {"Rust": 1572}},
    "cache_writes": 1571,
    "cache_write_errors": 1,
    "cache_read_errors": 0,
    "cache_timeouts": 1,
}


def test_the_measured_healthy_error_rate_only_warns(checker: types.ModuleType) -> None:
    """About 0.1% of stores failing is the proxy on a normal day, not a defect."""
    assessment = checker.assess(_document(**MEASURED_COLD_RUN), "ghac")
    assert assessment.failures == ()
    assert "cache_write_errors is 1" in assessment.warnings
    assert "cache_timeouts is 1" in assessment.warnings


#: One hundred reads and one hundred store attempts, so an error count reads
#: directly as a percentage of either.
ONE_HUNDRED_OF_EACH: typ.Final[dict[str, object]] = {
    "cache_hits": {"counts": {"Rust": 50}},
    "cache_misses": {"counts": {"Rust": 50}},
    "cache_writes": 100,
}


@pytest.mark.parametrize(
    "stats",
    [
        pytest.param(
            {"cache_writes": 89, "cache_write_errors": 11},
            id="eleven-percent-of-stores-failed-to-write",
        ),
        pytest.param({"cache_timeouts": 11}, id="eleven-percent-of-reads-timed-out"),
    ],
)
def test_an_error_rate_above_the_limit_fails(
    checker: types.ModuleType, stats: dict[str, object]
) -> None:
    """Eleven per cent is past the limit, whichever rate carries it.

    Each case keeps one hundred attempts of the kind it judges, so eleven errors
    is exactly 11%, and leaves the other rate clean.
    """
    failures = checker.assess(
        _document(**(ONE_HUNDRED_OF_EACH | stats)), "ghac"
    ).failures
    assert any("exceed 1/10" in failure for failure in failures), failures


@pytest.mark.parametrize(
    "stats",
    [
        pytest.param(
            {"cache_writes": 90, "cache_write_errors": 10}, id="ten-percent-of-stores"
        ),
        pytest.param({"cache_timeouts": 10}, id="ten-percent-of-reads"),
    ],
)
def test_an_error_rate_at_the_limit_still_only_warns(
    checker: types.ModuleType, stats: dict[str, object]
) -> None:
    """The limit is exceeded, not reached: exactly 10% warns and passes."""
    assessment = checker.assess(_document(**(ONE_HUNDRED_OF_EACH | stats)), "ghac")
    assert assessment.failures == (), assessment.failures
    assert assessment.warnings, "a nonzero error counter must still warn"


def test_a_timeout_on_a_warm_lane_is_judged_against_its_reads(
    checker: types.ModuleType,
) -> None:
    """One timed-out lookup beside one store is 0.1% of reads, not 100% of stores.

    sccache counts a timeout as a lookup miss. A warm lane stores almost
    nothing, so judging timeouts against stores would fail a healthy run.
    """
    warm = {
        "cache_hits": {"counts": {"Rust": 1000}},
        "cache_misses": {"counts": {"Rust": 1}},
        "cache_writes": 1,
        "cache_timeouts": 1,
    }
    assessment = checker.assess(_document(**warm), "ghac")
    assert assessment.failures == (), assessment.failures
    assert "cache_timeouts is 1" in assessment.warnings, assessment.warnings


def test_read_errors_have_no_rate_of_their_own(checker: types.ModuleType) -> None:
    """sccache never increments `cache_read_errors`, so no rate is built on it.

    A nonzero value, should a later sccache populate it, still warns.
    """
    stats = ONE_HUNDRED_OF_EACH | {"cache_read_errors": 11}
    assessment = checker.assess(_document(**stats), "ghac")
    assert assessment.failures == (), assessment.failures
    assert "cache_read_errors is 11" in assessment.warnings, assessment.warnings


@given(
    errors=st.integers(min_value=0, max_value=10_000),
    attempts=st.integers(min_value=0, max_value=10_000),
)
def test_the_rate_limit_matches_integer_arithmetic(
    checker: types.ModuleType, errors: int, attempts: int
) -> None:
    """`_exceeds_limit` agrees with an oracle that never divides.

    Ten errors per hundred attempts is the limit, so the rate exceeds it
    exactly when ten times the errors exceeds the attempts. No attempts is no
    rate.
    """
    expected = attempts > 0 and 10 * errors > attempts
    assert checker._exceeds_limit(errors, attempts) is expected, (errors, attempts)


#: sccache counters over a range that crosses the limit in both directions.
_COUNTER: typ.Final = st.integers(min_value=0, max_value=5_000)


@given(
    stats=st.fixed_dictionaries(
        {
            name: _COUNTER
            for name in (
                "cache_hits",
                "cache_misses",
                "cache_writes",
                "cache_write_errors",
                "cache_timeouts",
            )
        }
    )
)
def test_a_lane_fails_on_rate_exactly_when_the_oracle_says(
    checker: types.ModuleType, stats: dict[str, int]
) -> None:
    """Rate failures appear exactly when either integer oracle is exceeded."""
    stores = stats["cache_writes"] + stats["cache_write_errors"]
    reads = stats["cache_hits"] + stats["cache_misses"]
    expected = [
        stores > 0 and 10 * stats["cache_write_errors"] > stores,
        reads > 0 and 10 * stats["cache_timeouts"] > reads,
    ]
    failures = checker._error_rate_failures(stats)
    observed = [
        any("write errors" in failure for failure in failures),
        any("lookup timeouts" in failure for failure in failures),
    ]
    assert observed == expected, (stats, failures)


def test_the_limit_is_ten_percent(checker: types.ModuleType) -> None:
    """The named constant is the reviewed value, not a number in a condition."""
    assert checker.ERROR_RATE_LIMIT == Fraction(1, 10)


def test_a_local_lane_is_judged_against_its_own_backend(
    checker: types.ModuleType,
) -> None:
    """The expected prefix is an argument, so the local lanes can use it too."""
    local = _document(location='Local disk: "/home/runner/.cache/sccache"')
    assert checker.assess(local, "Local disk").failures == ()


@pytest.mark.parametrize(
    "case",
    [
        pytest.param((json.dumps(_document()), 0), id="healthy"),
        pytest.param((json.dumps(_document(compile_requests=0)), 1), id="broken"),
        pytest.param(("not json", 1), id="unreadable"),
        pytest.param(("[]", 1), id="not-an-object"),
    ],
)
def test_the_exit_status_follows_the_assessment(
    checker: types.ModuleType,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    case: tuple[str, int],
) -> None:
    """The job fails exactly when the assessment does, and says why."""
    contents, status = case
    statistics = tmp_path / "sccache-stats.json"
    statistics.write_text(contents, encoding="utf-8")
    assert checker.main(["--expect-location", "ghac", str(statistics)]) == status
    output = capsys.readouterr().out
    assert ("::error::" in output) is (status == 1)


def test_an_error_rate_above_the_limit_fails_the_job(
    checker: types.ModuleType, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """A rate failure reaches the exit status and the job log, not only `assess`."""
    stats = ONE_HUNDRED_OF_EACH | {"cache_timeouts": 11}
    statistics = tmp_path / "sccache-stats.json"
    statistics.write_text(json.dumps(_document(**stats)), encoding="utf-8")
    assert checker.main(["--expect-location", "ghac", str(statistics)]) == 1
    output = capsys.readouterr().out
    expected = "::error::sccache 11 lookup timeouts in 100 reads exceed 1/10"
    assert expected in output, output


def test_a_missing_file_fails(checker: types.ModuleType, tmp_path: Path) -> None:
    """No statistics is a broken lane, not a pass."""
    assert (
        checker.main(["--expect-location", "ghac", str(tmp_path / "absent.json")]) == 1
    )
