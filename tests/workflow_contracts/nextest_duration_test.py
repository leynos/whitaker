"""What the duration reader accepts, and what it refuses.

Every ordering the timeout contract asserts is an inequality between
two numbers this reader produced, so a wrong unit or a refused spelling
turns the whole contract into a comparison of plausible wrong values.
This repository's own ``.config/nextest.toml`` exercises four unit
spellings and no composite, so the file in the tree says nothing about
the rest of the grammar.

The reader had previously accepted ``ms``, ``s``, ``m`` and ``h`` alone,
one pair per value. nextest reads durations with humantime, which
accepts a sequence of pairs and many more spellings, so that reader
would have rejected configuration the runner runs happily. The values
below were measured against humantime 2.4.0, the version nextest
resolves.
"""

from __future__ import annotations

import pytest
from nextest_durations import UNIT_SECONDS, NextestConfigurationError, seconds


@pytest.mark.parametrize("unit", sorted(UNIT_SECONDS), ids=sorted(UNIT_SECONDS))
def test_every_unit_humantime_accepts_converts_exactly(unit: str) -> None:
    """One of each, exhaustively: the unit table is a finite, written-down set.

    Sampling it would leave whichever entry was wrong undetected, and a
    single wrong entry is enough to make every downstream inequality
    compare the wrong pair of numbers.
    """
    assert seconds(f"1{unit}") == pytest.approx(UNIT_SECONDS[unit]), (
        f"`1{unit}` must convert to {UNIT_SECONDS[unit]}s"
    )


@pytest.mark.parametrize(
    ("duration", "expected"),
    [
        pytest.param("  45s  ", 45.0, id="surrounding-whitespace"),
        pytest.param("2h 30m", 9000.0, id="a-composite-with-a-space"),
        pytest.param("1m30s", 90.0, id="a-composite-without-a-space"),
        pytest.param("1.5m", 90.0, id="a-fractional-value"),
        pytest.param("0.5s", 0.5, id="a-fraction-below-one"),
        pytest.param("1 . 5 m", 90.0, id="a-fraction-spaced-around-the-point"),
        pytest.param("2wks", 1209600.0, id="the-abbreviated-plural-week"),
        pytest.param("3yrs", 94672800.0, id="the-abbreviated-plural-year"),
    ],
)
def test_the_shapes_the_repository_does_not_use(duration: str, expected: float) -> None:
    """Each shape humantime accepts that a one-pair reader would refuse.

    A refusal here is not a caught fault but a manufactured one: the
    configuration would run, and only this contract would fail.
    """
    assert seconds(duration) == pytest.approx(expected), (
        f"{duration!r} must convert to {expected}s"
    )


@pytest.mark.parametrize(
    "duration",
    [
        pytest.param("", id="empty"),
        pytest.param("   ", id="whitespace-only"),
        pytest.param("300", id="no-unit"),
        pytest.param("s", id="no-value"),
        pytest.param("five minutes", id="words"),
        pytest.param("-30s", id="signed"),
        pytest.param(".5s", id="a-leading-point"),
        pytest.param("1.s", id="a-trailing-point"),
        pytest.param("1.5.5m", id="a-second-point"),
        pytest.param("1_000s", id="a-digit-separator"),
        pytest.param("30 fortnights", id="a-unit-humantime-lacks"),
    ],
)
def test_text_humantime_would_reject_is_refused(duration: str) -> None:
    """A reader that guessed at these would report a budget nextest never had.

    Accepting them is the worse failure of the two: the contract would
    then assert an ordering over a number nothing in the configuration
    produced.
    """
    with pytest.raises(NextestConfigurationError):
        seconds(duration)
