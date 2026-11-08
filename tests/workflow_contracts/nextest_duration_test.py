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
below were measured against humantime 2.3.0, the version the lockfile
of the pinned cargo-nextest release resolves.
"""

from __future__ import annotations

import re

import pytest
from nextest_durations import (
    display_seconds,
    _SPACE_CHARS,
    _digits,
    UNIT_SECONDS,
    NextestConfigurationError,
    seconds,
)


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
        pytest.param("1 0s", 10.0, id="whitespace-inside-the-number"),
        pytest.param("1 2 . 5 s", 12.5, id="whitespace-through-a-fraction"),
        pytest.param("0", 0.0, id="a-bare-zero-with-no-unit"),
        pytest.param("1.5h", 5400.0, id="a-fraction-of-an-hour-in-whole-seconds"),
        pytest.param("0.5ms", 0.0005, id="a-fraction-of-a-millisecond"),
        pytest.param("0.000000001m", 6e-8, id="a-fractional-minute-in-nanoseconds"),
        pytest.param("1nanos", 1e-9, id="the-long-nanosecond-spelling"),
        pytest.param("1millis", 0.001, id="the-long-millisecond-spelling"),
    ],
)
def test_the_shapes_the_repository_does_not_use(duration: str, expected: float) -> None:
    """Each shape humantime accepts that a one-pair reader would refuse.

    A refusal here is not a caught fault but a manufactured one: the
    configuration would run, and only this contract would fail.

    ``0.000000001m`` is sixty nanoseconds rather than one, and is the
    one input here that separates a minute's fraction scale from a
    second's: every other fraction reads the same under either. Nothing
    in the tree carries a fractional minute, so a reader that scaled one
    by a second would agree with every file in the repository and with
    every other case in this list.
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
        pytest.param(" 0 ", id="a-padded-bare-zero"),
        pytest.param("00", id="a-repeated-bare-zero"),
        pytest.param("s", id="no-value"),
        pytest.param("five minutes", id="words"),
        pytest.param("-30s", id="signed"),
        pytest.param(".5s", id="a-leading-point"),
        pytest.param("1.s", id="a-trailing-point"),
        pytest.param("1.5.5m", id="a-second-point"),
        pytest.param("1_000s", id="a-digit-separator"),
        pytest.param("30 fortnights", id="a-unit-humantime-lacks"),
        pytest.param("1.5ns", id="a-fractional-nanosecond"),
        pytest.param("0.5ns", id="half-a-nanosecond"),
        pytest.param("0.123h", id="a-fraction-of-an-hour-below-a-second"),
        pytest.param("0.0000000001s", id="a-fraction-below-a-nanosecond"),
    ],
)
def test_text_humantime_would_reject_is_refused(duration: str) -> None:
    """A reader that guessed at these would report a budget nextest never had.

    Accepting them is the worse failure of the two: the contract would
    then assert an ordering over a number nothing in the configuration
    produced.

    `` 0 `` and ``00`` are here rather than above because humantime
    special-cases the exact text ``0`` before it reads a character, so
    neither is that case and nextest refuses both.

    The four fractions are the other half of the same fault. humantime
    carries a fraction as a numerator over a power of ten and divides
    with a remainder check, so it has no step below a nanosecond and
    refuses one outright; and for hours and longer it divides whole
    seconds, which is why ``0.123h`` is refused where ``0.123s`` is
    exact. A reader using floating point would accept all four and
    report a budget the runner would have rejected at startup.
    """
    with pytest.raises(NextestConfigurationError):
        seconds(duration)


@pytest.mark.parametrize(
    ("duration", "expected"),
    [
        pytest.param(
            "18446744073709551615s",
            1.8446744073709552e19,
            id="the-largest-duration-humantime-holds",
        ),
        pytest.param(
            "18446744073709551615s 999999999ns",
            1.8446744073709552e19,
            id="one-nanosecond-short-of-the-ceiling",
        ),
        pytest.param("0.5s 0.5s", 1.0, id="two-half-seconds-carrying-to-one"),
        pytest.param(
            "18446744073709551614s 1999999999ns",
            1.8446744073709552e19,
            id="a-composite-that-carries-up-to-it",
        ),
        pytest.param(
            "584542046090y",
            1.8446744073689784e19,
            id="the-largest-year-count",
        ),
        # The only input in the estate differential that separates when
        # the carry happens. Every other case accepts or refuses the same
        # way whether the nanosecond part is carried as each pair is read
        # or once at the end; this one is accepted only by the first,
        # because carrying late leaves the accumulator holding 2**64.
        # Deferring the carry refuses it while humantime accepts, which
        # is the accept-what-the-runner-refuses direction inverted, and
        # is the mutation that proves this case load-bearing.
        pytest.param(
            "18446744073709551615ns 1ns",
            18446744073.709551616,
            id="a-nanosecond-part-carried-before-the-next-pair",
        ),
        pytest.param(
            "1.0000000000000000000s",
            1.0,
            id="a-fraction-of-nineteen-digits",
        ),
    ],
)
def test_the_largest_durations_humantime_holds_are_accepted(
    duration: str, expected: float
) -> None:
    """The size check must be a bound, not a ceiling somebody guessed at.

    A reader that refused these would fail a configuration nextest
    starts happily, which is the manufactured failure the whole module
    is written to avoid. Each is at or one step inside humantime's own
    limit, and each was read back from it: see the refusals below for
    the step past.

    ``18446744073709551615s 999999999ns`` is the acceptance companion
    the estate cites, one nanosecond short of the ceiling the refusals
    below step over. ``0.5s 0.5s`` is here for a different reason: its
    nanosecond part lands on exactly one second, the same value that
    overflows the ceiling in the refusal below, and it must carry into
    one whole second rather than being refused. One mechanism has to
    give both verdicts, so a reader made merely stricter to pass the
    refusal fails here.
    """
    assert seconds(duration) == pytest.approx(expected), (
        f"{duration!r} is inside humantime's range and must be accepted"
    )


@pytest.mark.parametrize(
    "duration",
    [
        pytest.param("18446744073709551616s", id="a-value-past-the-64-bit-range"),
        pytest.param("18446744073709551615s 1s", id="a-sum-of-seconds-past-the-range"),
        pytest.param(
            "18446744073709551615s 500ms 500ms",
            id="a-carry-past-the-ceiling-from-two-half-seconds",
        ),
        pytest.param(
            "18446744073709551615s 1000000000ns",
            id="a-carry-past-the-ceiling-in-one-value",
        ),
        pytest.param(
            "18446744073709551615ns 18446744073709551615ns",
            id="a-nanosecond-accumulator-overflow",
        ),
        pytest.param("18446744073709551615.5us", id="a-value-past-its-unit-scale"),
        pytest.param("584542046091y", id="a-year-count-past-the-range"),
        pytest.param("1.00000000000000000000s", id="a-fraction-of-twenty-digits"),
    ],
)
def test_durations_past_humantime_s_range_are_refused(duration: str) -> None:
    """Python's integers do not overflow, and humantime's do.

    Left unchecked, the reader would report a budget for a
    configuration nextest refuses at startup, and the ordering
    assertions would then compare a number nothing in the tree
    produced. The bound is not a single total: the nanosecond
    accumulator case above names thirty-seven seconds and is refused
    all the same, because the second value overflows before it is
    carried into seconds. The two carry cases are refused rather than
    read because humantime aborts the process on them; nextest cannot
    run either way. They are separate because they break at different
    steps: the pair of half-seconds overflows on the carry into
    seconds, while ``18446744073709551615s 1s`` overflows on the
    addition of seconds before any carry arises, and a reader can be
    wrong at one and right at the other.
    """
    with pytest.raises(NextestConfigurationError):
        seconds(duration)


@pytest.mark.parametrize(
    "duration",
    [
        pytest.param("\u0665s", id="an-arabic-indic-five"),
        pytest.param("\u096ems", id="a-devanagari-four"),
        pytest.param("1\u0660s", id="an-arabic-indic-zero-inside-a-number"),
        pytest.param("1.\u0665s", id="an-arabic-indic-digit-in-a-fraction"),
    ],
)
def test_digits_outside_ascii_are_refused(duration: str) -> None:
    """humantime matches ``'0'..='9'``; Python's ``\\d`` matches far more.

    Every one of these is a decimal digit to Python and not a digit to
    humantime, so a reader spelling its digit class ``\\d`` converts
    them happily and reports a budget for a configuration nextest
    refuses at startup. The last two matter most: a non-ASCII digit
    sitting inside an otherwise ordinary number is the shape nobody
    would notice in a file.
    """
    with pytest.raises(NextestConfigurationError):
        seconds(duration)


@pytest.mark.parametrize(
    "duration",
    [
        pytest.param("1\x1cs", id="a-file-separator-inside-a-number"),
        pytest.param("\x1c45m", id="a-file-separator-leading"),
        pytest.param("45m\x1f", id="a-unit-separator-trailing"),
        pytest.param("1\x1d0s", id="a-group-separator-between-digits"),
    ],
)
def test_c0_separators_are_not_whitespace_and_are_refused(duration: str) -> None:
    """Python calls U+001C to U+001F whitespace; humantime does not.

    Rust's ``char::is_whitespace`` is the Unicode White_Space property,
    which excludes the file, group, record and unit separators. Python's
    ``\\s``, ``str.strip`` and ``str.split`` all include them, so a
    reader written the obvious way reads the first of these as one
    second and the second as forty-five minutes, and reports a budget
    for a configuration nextest refuses at startup.

    The last is the one nobody would notice: a separator between two
    digits of an ordinary number, which ``str.split`` silently removes.
    """
    with pytest.raises(NextestConfigurationError):
        seconds(duration)


def test_the_whitespace_class_is_rusts_own() -> None:
    """The class is the Unicode White_Space property, and nothing more.

    Pins the reasoning rather than the consequence. The four separators
    the previous test refuses are refused because they are absent from
    this set, and this asserts both directions of that: Python's ``\\s``
    exceeds the set by exactly those four, and the set exceeds ``\\s``
    by nothing. If either language's notion of whitespace moves, this
    fails here rather than in a runner.
    """
    ours = set(_SPACE_CHARS)
    pythons = {c for c in map(chr, range(0x11000)) if re.fullmatch(r"\s", c)}
    assert pythons - ours == set("\x1c\x1d\x1e\x1f"), (
        "Python's whitespace must exceed humantime's by exactly the four "
        f"C0 separators, exceeds it by {sorted(pythons - ours)!r}"
    )
    assert not ours - pythons, (
        "every character this reader treats as whitespace must be one "
        f"Python agrees is whitespace, {sorted(ours - pythons)!r} are not"
    )


def test_the_digit_join_removes_only_what_the_pattern_tolerated() -> None:
    """``_digits`` strips the same class the pattern matched, not Python's.

    Reached only through a match, so while the pattern holds it never
    sees a C0 separator and ``str.split`` here would give the same
    answer. It is tested directly because that is the whole of the
    argument: the helper's correctness rests on the pattern's, and a
    later widening of the pattern would otherwise make ``str.split``
    delete a separator instead of the reader refusing it.

    A separator is not whitespace, so it survives; a real space does
    not.
    """
    assert _digits("1\x1c0") == "1\x1c0", (
        "a C0 separator is not whitespace and must survive the join, so "
        "that a pattern which let one through refuses the duration "
        "rather than silently reading a different number"
    )
    assert _digits("1 0") == "10", (
        "an ASCII space is whitespace and must be removed from the digits"
    )
    assert _digits("1\u20080") == "10", (
        "a Unicode space humantime skips must be removed like any other"
    )


def test_two_budgets_one_second_apart_are_distinguishable() -> None:
    """Exactness is not pedantry here: a float cannot tell these apart.

    humantime's range reaches 2**64 seconds and a float carries 53 bits
    of significand, so above 2**53 the representable values are further
    apart than one second. Both of these are in the differential this
    reader is measured against, and they are adjacent, so this is the
    narrowest gap the reader must preserve at the top of its range.
    """
    smaller = seconds("18446744073709551614s")
    larger = seconds("18446744073709551615s")
    assert smaller != larger, "two budgets a second apart are not one budget"
    assert float(smaller) == float(larger), (
        "this is the defect being avoided rather than a property to rely "
        "on: converted to float the two are the same number, so the "
        "assertion above would hold whichever way round it was written"
    )


def test_an_ordering_between_them_has_a_direction() -> None:
    """The ordering assertions are the reason exactness matters.

    Every tier comparison in this repository is an inequality between
    two of these values. Under float both `<` and `>` are false for the
    pair below, so an ordering assertion between them passes when it is
    written backwards, which is the failure that does not look like one.
    """
    smaller = seconds("18446744073709551614s")
    larger = seconds("18446744073709551615s")
    assert smaller < larger, "the smaller budget must order below the larger"
    assert not float(smaller) < float(larger), (
        "again the defect, not a property: in float neither is less than "
        "the other, so an inequality written either way round holds"
    )


def test_a_period_multiplied_by_its_terminate_after_stays_exact() -> None:
    """`terminate-after` multiplies a period, and tenths have no float.

    A per-test allowance is a period times a count, and the guide states
    some of them as decimals. Multiplying the float and comparing with
    the written total differs by a rounding error, which a tolerance
    would paper over and an exact value does not need.
    """
    period = seconds("0.1s")
    assert period * 3 == seconds("0.3s"), (
        "a tenth taken three times is three tenths"
    )
    assert float(period) * 3 != float(seconds("0.3s")), (
        "in float it is not, by 5.55e-17; that is what this reader stops "
        "reaching the comparisons"
    )


def test_the_display_helper_is_the_lossy_one_on_purpose() -> None:
    """`display_seconds` is for messages, and says so by being separate.

    Named apart from `seconds` so a caller chooses the lossy conversion
    rather than receiving it by default, which is how the float got into
    the comparisons in the first place.
    """
    assert display_seconds("45m") == 2700.0, (
        "the display helper must still convert the duration it is given"
    )
    assert isinstance(display_seconds("45m"), float), (
        "the display helper returns a float, which is what makes it lossy"
    )
    assert display_seconds("18446744073709551614s") == display_seconds(
        "18446744073709551615s"
    ), "the display helper is lossy, which is why it is not the comparison"
