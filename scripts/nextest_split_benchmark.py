"""Prepare and summarize comparable Nextest runs for the issue-513 split.

The benchmark compares two configurations that differ in exactly one thing:
which capacity-one test group the seven trybuild compile contracts belong to.
The deliverable keeps the contracts in ``serial-trybuild-contracts``; the
control rewrites only those ``test-group`` assignments back to
``serial-dylint-ui``. Both sides also need JUnit output, which Nextest
configures in a profile rather than on the command line, so every measurement
runs from a generated copy of the file and never edits the deliverable.

Usage
-----
Run ``python scripts/nextest_split_benchmark.py --help``. The benchmark
workflow drives the ``prepare``, ``union-filter``, ``membership``, and
``summarize`` subcommands; ``scripts/tests`` exercises the same functions.
"""

from __future__ import annotations

import argparse
import collections
import dataclasses
import json
import pathlib
import re
import sys
import tomllib
import typing as typ
import xml.etree.ElementTree as ET
from datetime import datetime, timedelta, timezone

SERIAL_DYLINT_UI = "serial-dylint-ui"
SERIAL_TRYBUILD_CONTRACTS = "serial-trybuild-contracts"
BENCHMARK_GROUPS = (SERIAL_TRYBUILD_CONTRACTS, SERIAL_DYLINT_UI)
#: Applied to a test outside both benchmark groups. A whole-suite run reports
#: thousands of these, and they belong to no capacity-one group, so overlap
#: must not be computed for them.
UNCLASSIFIED = "other"

#: The assignment the control rewrites. Matched with the surrounding whitespace
#: captured so every other byte of the configuration stays as delivered.
_TRYBUILD_ASSIGNMENT: typ.Final[re.Pattern[str]] = re.compile(
    r'^(?P<lead>[ \t]*test-group[ \t]*=[ \t]*)"serial-trybuild-contracts"'
    r"(?P<trail>[ \t]*)$",
    re.MULTILINE,
)
_SUMMARY_SECONDS: typ.Final[re.Pattern[str]] = re.compile(r"Summary \[\s*([0-9.]+)s\]")
#: Nextest colours the summary heading, and CI runs with `CARGO_TERM_COLOR`
#: set. The escape lands between ``Summary`` and the bracket, so the duration
#: is unreadable until the escapes are removed.
_ANSI_ESCAPE: typ.Final[re.Pattern[str]] = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
_ISO_TIMESTAMP: typ.Final[re.Pattern[str]] = re.compile(
    r"^(?P<date>\d{4}-\d{2}-\d{2})T(?P<time>\d{2}:\d{2}:\d{2})"
    r"(?P<fraction>\.\d+)?(?P<zone>Z|[+-]\d{2}:\d{2})?$"
)


class BenchmarkError(ValueError):
    """Raised when a configuration or report cannot support a comparison."""


def _assigns_group(config_text: str, group: str) -> bool:
    """Return whether any profile override assigns ``group``."""
    document = tomllib.loads(config_text)
    return any(
        override.get("test-group") == group
        for section in document.get("profile", {}).values()
        for override in section.get("overrides") or []
    )


def serial_control_text(config_text: str) -> str:
    """Return the configuration with the trybuild group folded into the UI group.

    Only the ``test-group`` assignments change; every filter, allowance, retry
    policy and profile name stays as delivered. That is what makes the control
    comparable: the two runs differ in group assignment and nothing else.

    Parameters
    ----------
    config_text
        The delivered ``nextest.toml`` text.

    Returns
    -------
    str
        The same configuration with every trybuild ``test-group`` assignment
        naming ``serial-dylint-ui`` instead.

    Raises
    ------
    BenchmarkError
        If the file names no trybuild group, or an assignment survives the
        rewrite in a form the substitution does not match. Either case would
        make the control a partial or equal copy of the deliverable, so a
        comparison against it would silently measure the wrong thing.

    Examples
    --------
    >>> config = (
    ...     "[profile.default]\\n"
    ...     '[[profile.default.overrides]]\\n'
    ...     'filter = "test(contract_a)"\\n'
    ...     'test-group = "serial-trybuild-contracts"\\n'
    ... )
    >>> "serial-dylint-ui" in serial_control_text(config)
    True
    """
    rewritten, replacements = _TRYBUILD_ASSIGNMENT.subn(
        rf'\g<lead>"{SERIAL_DYLINT_UI}"\g<trail>', config_text
    )
    if replacements == 0:
        message = (
            f"no test-group assignment names {SERIAL_TRYBUILD_CONTRACTS!r}; "
            "the control would equal the deliverable"
        )
        raise BenchmarkError(message)
    if _assigns_group(rewritten, SERIAL_TRYBUILD_CONTRACTS):
        message = (
            f"a test-group assignment of {SERIAL_TRYBUILD_CONTRACTS!r} survived "
            "the rewrite; the control would be a partial split"
        )
        raise BenchmarkError(message)
    return rewritten


def with_junit(config_text: str, profile: str, junit_path: pathlib.Path) -> str:
    """Return the configuration with a JUnit report path for one profile.

    The path is absolute so Nextest writes it wherever the caller collected the
    measurement, not relative to the workspace root. An existing section is
    replaced rather than duplicated, so preparing a run twice stays idempotent.

    Parameters
    ----------
    config_text
        The configuration to extend.
    profile
        The Nextest profile that should write the report.
    junit_path
        Where the report belongs.
    Returns
    -------
    str
        The configuration with one ``[profile.<profile>.junit]`` section.
    Examples
    --------
    >>> document = with_junit(
    ...     "[profile.default]\\n", "default", pathlib.Path("/tmp/run.xml")
    ... )
    >>> document.count("[profile.default.junit]")
    1
    """
    section = f"[profile.{profile}.junit]"
    block = f'{section}\npath = "{junit_path}"\n'
    lines = config_text.splitlines(keepends=True)
    for index, line in enumerate(lines):
        if line.strip() != section:
            continue
        end = index + 1
        while end < len(lines) and not lines[end].lstrip().startswith("["):
            end += 1
        return "".join([*lines[:index], block, *lines[end:]])
    separator = "" if config_text.endswith("\n") else "\n"
    return f"{config_text}{separator}\n{block}"


def profile_overrides(
    config_text: str, profile: str, groups: typ.Sequence[str] = BENCHMARK_GROUPS
) -> list[dict[str, typ.Any]]:
    """Return the overrides in one profile that name a benchmark group.

    Parameters
    ----------
    config_text
        The configuration to read.
    profile
        The Nextest profile whose overrides are wanted.
    groups
        The test-group names that select an override.
    Returns
    -------
    list of dict
        The matching overrides, in the order the profile declares them.
    Raises
    ------
    BenchmarkError
        If the text is not valid TOML, or the profile is not declared.
    Examples
    --------
    >>> config = (
    ...     "[profile.default]\\n"
    ...     "[[profile.default.overrides]]\\n"
    ...     'filter = "test(a)"\\n'
    ...     'test-group = "serial-dylint-ui"\\n'
    ... )
    >>> [override["filter"] for override in profile_overrides(config, "default")]
    ['test(a)']
    """
    try:
        document = tomllib.loads(config_text)
    except tomllib.TOMLDecodeError as error:
        raise BenchmarkError(f"configuration is not valid TOML: {error}") from error
    section = document.get("profile", {}).get(profile)
    if section is None:
        raise BenchmarkError(f"profile {profile!r} is not declared")
    wanted = set(groups)
    return [
        override
        for override in section.get("overrides") or []
        if override.get("test-group") in wanted
    ]


def union_filter(
    config_text: str, profile: str, groups: typ.Sequence[str] = BENCHMARK_GROUPS
) -> str:
    """Return one Nextest filter selecting every test the groups cover.

    Each group is spread over several overrides, so the union is a disjunction
    of all their filters. Both configurations produce the same expression: the
    control changes group assignment, not filters, and the benchmark verifies
    that by listing both selections.

    Parameters
    ----------
    config_text
        The configuration to read.
    profile
        The Nextest profile that carries the group overrides.
    groups
        The test-group names to include in the union.
    Returns
    -------
    str
        One Nextest filter expression, ready for ``-E``.
    Raises
    ------
    BenchmarkError
        If no override in the profile selects one of the groups, which means
        the benchmark is being dispatched against a ref that does not carry the
        split.
    Examples
    --------
    >>> config = (
    ...     "[profile.default]\\n"
    ...     "[[profile.default.overrides]]\\n"
    ...     'filter = "test(a)"\\n'
    ...     'test-group = "serial-dylint-ui"\\n'
    ... )
    >>> union_filter(config, "default")
    '(test(a))'
    """
    wanted = set(groups)
    clauses = [
        f"({override['filter']})"
        for override in profile_overrides(config_text, profile, groups)
        if override.get("filter")
    ]
    if not clauses:
        message = (
            f"no override in profile {profile!r} selects any of {sorted(wanted)}; "
            "dispatch the benchmark against a ref that contains the split"
        )
        raise BenchmarkError(message)
    return " | ".join(clauses)


def _suites(listing: dict[str, typ.Any]) -> dict[str, typ.Any]:
    """Return the binary suites of a Nextest list report, whatever key holds them."""
    for key in ("rust-suites", "suites"):
        suites = listing.get(key)
        if isinstance(suites, dict):
            return suites
    return {}


def _matching_cases(
    listing: dict[str, typ.Any],
) -> typ.Iterator[tuple[str, str, dict[str, typ.Any]]]:
    """Yield ``(binary_id, test_name, case)`` for the tests a filter selects.

    Nextest lists every test it found in a binary that it had to run, marking
    each with ``filter-match``. Non-matching tests are therefore present in the
    report, and counting them would attribute unselected tests to a group.
    """
    for binary_id, suite in _suites(listing).items():
        for test_name, case in (suite.get("testcases") or {}).items():
            match = case.get("filter-match") or {}
            if match.get("status", "matches") == "matches":
                yield binary_id, test_name, case


def _qualified_names(listing: dict[str, typ.Any]) -> set[str]:
    """Return ``binary-id::test-name`` for every selected test in a list report."""
    return {
        f"{binary_id}::{test_name}"
        for binary_id, test_name, _ in _matching_cases(listing)
    }


def _ignored_names(listing: dict[str, typ.Any]) -> set[str]:
    """Return the qualified names Nextest skipped because they are ignored.

    Nextest reports an ignored test as a filter mismatch whose reason is
    ``ignored``, so it is outside the selected set and never reaches the JUnit
    report. Recording it separately keeps the wave of skipped tests visible
    without inflating ``selected_not_run``.
    """
    return {
        f"{binary_id}::{test_name}"
        for binary_id, suite in _suites(listing).items()
        for test_name, case in (suite.get("testcases") or {}).items()
        if case.get("ignored") is True
        and (case.get("filter-match") or {}).get("reason") == "ignored"
    }


def _disjoint_selection(memberships: typ.Mapping[str, list[str]]) -> list[str]:
    """Return every member, raising when two groups share one."""
    counts = collections.Counter(
        test for names in memberships.values() for test in names
    )
    overlaps = sorted(test for test, count in counts.items() if count > 1)
    if overlaps:
        raise BenchmarkError(f"test groups share members: {overlaps}")
    return sorted(counts)


def _check_expected_counts(
    memberships: typ.Mapping[str, list[str]],
    expected_counts: typ.Mapping[str, int] | None,
) -> None:
    """Raise when a group's member count differs from its pinned size."""
    for name, expected in (expected_counts or {}).items():
        actual = len(memberships.get(name, ()))
        if actual != expected:
            raise BenchmarkError(
                f"group {name!r} selects {actual} tests, expected {expected}"
            )


def _check_control_selection(selection: list[str], control: set[str]) -> None:
    """Raise when the control configuration selects a different test set."""
    if control == set(selection):
        return
    message = (
        "the control configuration selects a different test set: "
        f"missing={sorted(set(selection) - control)} "
        f"extra={sorted(control - set(selection))}"
    )
    raise BenchmarkError(message)


def membership_report(
    groups: typ.Mapping[str, dict[str, typ.Any]],
    control_selection: set[str] | None = None,
    expected_counts: typ.Mapping[str, int] | None = None,
) -> dict[str, typ.Any]:
    """Return the group membership a JUnit report is classified against.

    Memberships must be disjoint: a test counted in two groups would make the
    overlap measurement mean two things at once. The control selection is
    compared with the split selection so a control that selects a different set
    is rejected before any timing is compared.

    Parameters
    ----------
    groups
        Maps a group name to a parsed ``cargo nextest list`` report.
    control_selection
        The qualified names the control configuration selects, when the caller
        wants that equality checked.
    expected_counts
        Pins the size of a group, so a filter that silently stops selecting one
        of the seven compile contracts cannot become evidence.
    Returns
    -------
    dict
        ``groups``, ``ignored``, and ``selection``, plus
        ``control_selection_matches`` when a control was checked.
    Raises
    ------
    BenchmarkError
        If two groups share a member, a pinned count does not match, or the
        control selects a different test set.
    Examples
    --------
    >>> listing = {"rust-suites": {"crate::ui": {"testcases": {"ui": {}}}}}
    >>> membership_report({SERIAL_DYLINT_UI: listing})["selection"]
    ['crate::ui::ui']
    """
    memberships = {name: sorted(_qualified_names(report)) for name, report in groups.items()}
    # An ignored test appears in every listing whose binary was built, so the
    # record is a union rather than a per-group map that would count it twice.
    ignored = sorted(
        {test for report in groups.values() for test in _ignored_names(report)}
    )
    selection = _disjoint_selection(memberships)
    _check_expected_counts(memberships, expected_counts)
    report: dict[str, typ.Any] = {
        "groups": memberships,
        "ignored": ignored,
        "selection": selection,
    }
    if control_selection is not None:
        _check_control_selection(selection, control_selection)
        report["control_selection_matches"] = True
    return report


def group_of(membership: dict[str, typ.Any], qualified_name: str) -> str:
    """Return the originating group of a test, or ``UNCLASSIFIED``.

    A JUnit report also carries tests outside both benchmark groups. They are
    reported as ``UNCLASSIFIED`` rather than dropped, so the summary keeps a
    total that matches Nextest's own count.
    """
    for group, names in membership["groups"].items():
        if qualified_name in names:
            return group
    return UNCLASSIFIED


def _parse_timestamp(value: str | None, test_name: str) -> datetime:
    """Parse a JUnit timestamp, failing loudly when it is missing."""
    if not value:
        raise BenchmarkError(f"JUnit testcase {test_name!r} has no timestamp")
    match = _ISO_TIMESTAMP.match(value)
    if match is None:
        raise BenchmarkError(f"JUnit testcase {test_name!r} has timestamp {value!r}")
    fraction = match.group("fraction") or ".0"
    zone = match.group("zone") or "+00:00"
    zone = "+00:00" if zone == "Z" else zone
    return datetime.fromisoformat(
        f"{match.group('date')}T{match.group('time')}{fraction}{zone}"
    ).astimezone(timezone.utc)


@dataclasses.dataclass(frozen=True)
class Attempt:
    """One recorded execution of a test."""

    started: datetime
    seconds: float

    @property
    def finished(self) -> datetime:
        """Return the moment this attempt finished."""
        return self.started + timedelta(seconds=self.seconds)


@dataclasses.dataclass(frozen=True)
class MeasuredTest:
    """One completed test as the JUnit report recorded it."""

    name: str
    group: str
    started: datetime
    seconds: float
    status: str
    retried: bool
    attempts: tuple[Attempt, ...] = ()

    @property
    def finished(self) -> datetime:
        """Return the moment the deciding attempt finished."""
        return self.started + timedelta(seconds=self.seconds)

    @property
    def intervals(self) -> tuple[Attempt, ...]:
        """Return every attempt, so overlap uses real execution windows.

        A retried test contributes its retries as well as its deciding attempt.
        Keeping only the testcase element would understate the window Nextest
        actually spent on the test, and for a failing test that element is the
        *first* attempt, not the last.
        """
        return self.attempts or (Attempt(self.started, self.seconds),)

    @property
    def longest_attempt(self) -> float:
        """Return the duration of the longest attempt."""
        return max(attempt.seconds for attempt in self.intervals)


_RERUN_TAGS: typ.Final[frozenset[str]] = frozenset(
    {"flakyFailure", "flakyError", "rerunFailure", "rerunError"}
)


def _status_of(testcase: ET.Element) -> str:
    """Return the outcome a JUnit testcase records."""
    for child in testcase:
        if child.tag in {"failure", "error"}:
            return child.tag
        if child.tag == "skipped":
            return "skipped"
    return "passed"


def _attempts_of(testcase: ET.Element, name: str) -> tuple[Attempt, ...]:
    """Return the deciding attempt followed by every recorded rerun."""
    attempts = [
        Attempt(
            _parse_timestamp(testcase.get("timestamp"), name),
            float(testcase.get("time") or 0.0),
        )
    ]
    for child in testcase:
        if child.tag not in _RERUN_TAGS:
            continue
        attempts.append(
            Attempt(
                _parse_timestamp(child.get("timestamp"), name),
                float(child.get("time") or 0.0),
            )
        )
    return tuple(attempts)


def measure_junit(
    xml_text: str, membership: dict[str, typ.Any]
) -> list[MeasuredTest]:
    """Return every testcase in a JUnit report, classified by group.

    Nextest writes one ``testcase`` per test, so a repeated ``(classname,
    name)`` pair means the same test was reported twice; the last element
    decides the recorded outcome. Rerun elements inside a testcase are kept as
    separate attempts, so a retried test occupies the time it really ran.

    Parameters
    ----------
    xml_text
        The JUnit XML a Nextest run wrote.
    membership
        The group membership produced by :func:`membership_report`.

    Returns
    -------
    list of MeasuredTest
        One entry per reported test, in the order the report declared them.

    Raises
    ------
    BenchmarkError
        If the XML does not parse, or a testcase carries no readable
        timestamp, which would make its interval unusable for overlap.
    Examples
    --------
    >>> xml = (
    ...     '<testsuite><testcase classname="crate::ui" name="ui" '
    ...     'time="1.5" timestamp="2026-01-01T00:00:00.000Z"/></testsuite>'
    ... )
    >>> membership = {"groups": {SERIAL_DYLINT_UI: ["crate::ui::ui"]}}
    >>> measure_junit(xml, membership)[0].group
    'serial-dylint-ui'
    """
    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError as error:
        raise BenchmarkError(f"JUnit report is not valid XML: {error}") from error
    attempts: collections.Counter[tuple[str, str]] = collections.Counter()
    cases: dict[tuple[str, str], ET.Element] = {}
    for testcase in root.iter("testcase"):
        classname = testcase.get("classname") or ""
        name = testcase.get("name") or ""
        key = (classname, name)
        attempts[key] += 1
        # Nextest writes one testcase per test, so duplicates would mean two
        # attempts were reported as separate cases; the last one decides.
        cases[key] = testcase
    retried = {key for key, count in attempts.items() if count > 1} | {
        key
        for key, testcase in cases.items()
        if any(child.tag in _RERUN_TAGS for child in testcase)
    }

    measured: list[MeasuredTest] = []
    for (classname, name), testcase in cases.items():
        qualified = f"{classname}::{name}"
        key = (classname, name)
        intervals = _attempts_of(testcase, qualified)
        measured.append(
            MeasuredTest(
                name=qualified,
                group=group_of(membership, qualified),
                started=intervals[0].started,
                seconds=intervals[0].seconds,
                status=_status_of(testcase),
                retried=key in retried,
                attempts=intervals,
            )
        )
    return measured


def _overlaps(earlier: Attempt, later: Attempt) -> bool:
    """Return whether two measured intervals share any instant."""
    return earlier.started < later.finished and later.started < earlier.finished


#: One measured execution: its group, a label identifying the attempt, and the
#: interval it occupied.
_Window: typ.TypeAlias = tuple[str, str, Attempt]


def _overlap_pairs(
    ordered: typ.Sequence[_Window],
) -> tuple[list[tuple[str, str]], list[tuple[str, str]]]:
    """Return the cross-group and within-group overlapping label pairs."""
    cross_pairs: list[tuple[str, str]] = []
    within_pairs: list[tuple[str, str]] = []
    for index, (group, label, attempt) in enumerate(ordered):
        for other_group, other_label, other in ordered[index + 1 :]:
            if not _overlaps(attempt, other):
                continue
            pair = (label, other_label)
            if group == other_group:
                within_pairs.append(pair)
            else:
                cross_pairs.append(pair)
    return cross_pairs, within_pairs


def _peak_concurrency(windows: typ.Sequence[_Window]) -> int:
    """Return the greatest number of benchmark groups running at one instant."""
    events: list[tuple[datetime, int, str]] = []
    for group, _, attempt in windows:
        events.append((attempt.started, 1, group))
        events.append((attempt.finished, 0, group))
    # Ends sort before starts at the same instant, so touching intervals are
    # not counted as concurrent.
    events.sort(key=lambda event: (event[0], event[1]))
    active: collections.Counter[str] = collections.Counter()
    peak = 0
    for _, is_start, group in events:
        if is_start:
            active[group] += 1
        else:
            active[group] -= 1
        peak = max(peak, sum(1 for count in active.values() if count > 0))
    return peak


def overlap_report(tests: typ.Sequence[MeasuredTest]) -> dict[str, typ.Any]:
    """Return cross-group and within-group interval overlaps.

    Strict overlap is the point of the split: the two capacity-one groups may
    run at the same time, while two tests from one group may not. A zero cross
    figure on a split run means the split brought no concurrency, and a
    non-zero within figure means the group capacity did not hold. Intervals are
    per attempt, so a retried test occupies the time it really ran rather than
    only the attempt that decided its outcome.

    Parameters
    ----------
    tests
        The measured tests to inspect. Tests outside both benchmark groups are
        excluded: a whole-suite run contains thousands of them, and their
        overlaps say nothing about either capacity-one group.

    Returns
    -------
    dict
        Pair counts and the maximum concurrency, plus up to five example pairs
        from each direction.

    Examples
    --------
    >>> start = datetime(2026, 1, 1, tzinfo=timezone.utc)
    >>> tests = [
    ...     MeasuredTest("a", SERIAL_DYLINT_UI, start, 10, "passed", False),
    ...     MeasuredTest("b", SERIAL_TRYBUILD_CONTRACTS,
    ...                  start + timedelta(seconds=5), 10, "passed", False),
    ... ]
    >>> report = overlap_report(tests)
    >>> report["cross_group_pairs"], report["within_group_pairs"]
    (1, 0)
    """
    windows: list[_Window] = [
        (test.group, f"{test.name} (attempt {index})", attempt)
        for test in tests
        if test.group != UNCLASSIFIED
        for index, attempt in enumerate(test.intervals, start=1)
    ]
    ordered = sorted(windows, key=lambda window: window[2].started)
    cross_pairs, within_pairs = _overlap_pairs(ordered)
    return {
        "cross_group_pairs": len(cross_pairs),
        "within_group_pairs": len(within_pairs),
        "max_cross_group_concurrency": _peak_concurrency(windows),
        "cross_group_examples": cross_pairs[:5],
        "within_group_examples": within_pairs[:5],
    }


def _nextest_seconds(log_text: str) -> float | None:
    """Return the duration Nextest reported for the run, if it reported one."""
    match = _SUMMARY_SECONDS.search(_ANSI_ESCAPE.sub("", log_text))
    return None if match is None else float(match.group(1))


def _counts(tests: typ.Sequence[MeasuredTest]) -> dict[str, int]:
    """Return the outcome counts a Nextest summary reports."""
    statuses = collections.Counter(test.status for test in tests)
    return {
        "total": len(tests),
        "passed": statuses.get("passed", 0),
        # Nextest writes a failed assertion as ``failure`` and a harness error
        # as ``error``; both are failed repetitions for the benchmark.
        "failed": statuses.get("failure", 0) + statuses.get("error", 0),
        "skipped": statuses.get("skipped", 0),
    }


def _slowest(
    tests: typ.Sequence[MeasuredTest], limit: int = 15
) -> list[dict[str, typ.Any]]:
    """Return the longest tests, ranked by their longest attempt."""
    ranked = sorted(tests, key=lambda test: test.longest_attempt, reverse=True)
    return [
        {
            "name": test.name,
            "group": test.group,
            "seconds": test.longest_attempt,
            "status": test.status,
            "attempts": len(test.intervals),
        }
        for test in ranked[:limit]
    ]


def _test_records(tests: typ.Sequence[MeasuredTest]) -> list[dict[str, typ.Any]]:
    """Return every measured test in start order, ready for JSON."""
    return [
        {
            "name": test.name,
            "group": test.group,
            "started": test.started.isoformat(),
            "finished": test.finished.isoformat(),
            "seconds": test.seconds,
            "status": test.status,
            "retried": test.retried,
            "attempts": len(test.intervals),
        }
        for test in sorted(tests, key=lambda test: test.started)
    ]


def summarize(
    junit_text: str,
    membership: dict[str, typ.Any],
    log_text: str = "",
    meta: dict[str, typ.Any] | None = None,
) -> dict[str, typ.Any]:
    """Return a machine-readable summary of one measured run.

    Failed attempts are part of the record: a repetition that failed is
    reported, never replaced by a later one. A test in the selection that the
    report never mentions is listed as ``selected_not_run``, so an interrupted
    run cannot look complete.

    Parameters
    ----------
    junit_text
        The JUnit XML the run wrote.
    membership
        The group membership produced by :func:`membership_report`.
    log_text
        The captured Nextest output, used only for its summary duration.
    meta
        The wall-clock and exit-status record the driver wrote.

    Returns
    -------
    dict
        Meta, durations, outcome counts, retry evidence, missing tests, the
        fifteen slowest tests, the overlap report, and every test interval.

    Raises
    ------
    BenchmarkError
        If the JUnit report does not parse or a testcase has no readable
        timestamp.
    Examples
    --------
    >>> xml = (
    ...     '<testsuite><testcase classname="crate::ui" name="ui" '
    ...     'time="1.5" timestamp="2026-01-01T00:00:00.000Z"/></testsuite>'
    ... )
    >>> membership = {
    ...     "groups": {SERIAL_DYLINT_UI: ["crate::ui::ui"]},
    ...     "selection": ["crate::ui::ui"],
    ... }
    >>> summarize(xml, membership)["counts"]["passed"]
    1
    """
    tests = measure_junit(junit_text, membership)
    selection = set(membership["selection"])
    observed = {test.name for test in tests}
    record = meta or {}
    return {
        "meta": record,
        "nextest_seconds": _nextest_seconds(log_text),
        "wall_seconds": record.get("wall_seconds"),
        "exit_status": record.get("exit_status"),
        "counts": _counts(tests),
        "retried": sorted({test.name for test in tests if test.retried}),
        "selected_not_run": sorted(selection - observed),
        "unclassified": sorted(
            test.name
            for test in tests
            if test.group == UNCLASSIFIED and test.name in selection
        ),
        "slowest": _slowest(tests),
        # A skipped test records no execution window, so it would otherwise
        # contribute a zero-length interval wherever it happens to sit.
        "overlap": overlap_report(
            [test for test in tests if test.status != "skipped"]
        ),
        "tests": _test_records(tests),
    }


def _write_json(path: pathlib.Path, payload: typ.Any) -> None:
    """Write a JSON document, creating its directory."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def _read_text(path: pathlib.Path, kind: str, errors: str = "strict") -> str:
    """Read a text input, naming the file and its kind when it cannot be read."""
    try:
        return path.read_text(errors=errors)
    except OSError as error:
        raise BenchmarkError(f"cannot read the {kind} {path}: {error}") from error


def _read_json(path: pathlib.Path) -> typ.Any:
    """Read a JSON document, naming the file when it cannot be read."""
    text = _read_text(path, "JSON document")
    try:
        return json.loads(text)
    except json.JSONDecodeError as error:
        raise BenchmarkError(f"cannot read {path}: {error}") from error


#: Either key identifies a Nextest list report across the supported versions.
_LISTING_KEYS: typ.Final = ("rust-suites", "suites")


def _read_listing(path: pathlib.Path) -> dict[str, typ.Any]:
    """Read a Nextest list report captured inside a ``make test`` log.

    Nextest writes its report without a trailing newline, so whatever the build
    prints next is appended to the report's line. Requiring the whole line to
    parse therefore loses the report, so the document's end is found with the
    raw decoder instead.
    """
    text = _read_text(path, "JSON report")
    listing = _decode_listing(text)
    if listing is None:
        raise BenchmarkError(f"cannot read {path}: no Nextest list report found")
    return listing


def _decode_listing(text: str) -> dict[str, typ.Any] | None:
    """Return the first Nextest list report in ``text``, if any."""
    decoder = json.JSONDecoder()
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped.startswith("{"):
            continue
        try:
            value, _ = decoder.raw_decode(stripped)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and any(key in value for key in _LISTING_KEYS):
            return value
    return None


def _parse_group_argument(value: str) -> tuple[str, pathlib.Path]:
    """Split one ``name=path`` listing argument."""
    name, separator, path = value.partition("=")
    if not separator or not name or not path:
        raise argparse.ArgumentTypeError(f"expected name=path, got {value!r}")
    return name, pathlib.Path(path)


def _parse_count_argument(value: str) -> tuple[str, int]:
    """Split one ``name=count`` expectation."""
    name, separator, count = value.partition("=")
    if not separator or not name or not count.isdigit():
        raise argparse.ArgumentTypeError(f"expected name=count, got {value!r}")
    return name, int(count)


def _prepare(args: argparse.Namespace) -> int:
    """Write one generated configuration for a measured run."""
    config_text = _read_text(args.config, "nextest configuration")
    if args.serial_control:
        config_text = serial_control_text(config_text)
    config_text = with_junit(config_text, args.profile, args.junit.resolve())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(config_text)
    return 0


def _union_filter(args: argparse.Namespace) -> int:
    """Print the filter selecting every benchmark test."""
    groups = tuple(args.group) if args.group else BENCHMARK_GROUPS
    config_text = _read_text(args.config, "nextest configuration")
    print(union_filter(config_text, args.profile, groups))
    return 0


def _membership(args: argparse.Namespace) -> int:
    """Write the group membership used to classify JUnit reports."""
    groups = {name: _read_listing(path) for name, path in args.group}
    control = None if args.control is None else _read_listing(args.control)
    report = membership_report(
        groups,
        None if control is None else _qualified_names(control),
        dict(args.expect_count or ()),
    )
    _write_json(args.output, report)
    return 0


def _summarize(args: argparse.Namespace) -> int:
    """Write the summary for one measured run and report a failed repetition."""
    membership = _read_json(args.membership)
    log_text = (
        "" if args.log is None else _read_text(args.log, "run log", errors="replace")
    )
    meta = {} if args.meta is None else _read_json(args.meta)
    junit_text = _read_text(args.junit, "JUnit report")
    summary = summarize(junit_text, membership, log_text, meta)
    _write_json(args.output, summary)
    failed = summary["counts"]["failed"]
    status = summary["exit_status"]
    if failed or (status is not None and status != 0):
        print(
            f"{args.junit.stem}: {failed} failed tests, exit status {status}",
            file=sys.stderr,
        )
        return 1
    return 0


def _build_parser() -> argparse.ArgumentParser:
    """Return the command-line interface."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)

    prepare = commands.add_parser("prepare", help="write a run configuration")
    prepare.add_argument("--config", type=pathlib.Path, required=True)
    prepare.add_argument("--output", type=pathlib.Path, required=True)
    prepare.add_argument("--junit", type=pathlib.Path, required=True)
    prepare.add_argument("--profile", default="default")
    prepare.add_argument("--serial-control", action="store_true")
    prepare.set_defaults(handler=_prepare)

    expression = commands.add_parser("union-filter", help="print the selection filter")
    expression.add_argument("--config", type=pathlib.Path, required=True)
    expression.add_argument("--profile", default="default")
    expression.add_argument("--group", action="append")
    expression.set_defaults(handler=_union_filter)

    membership = commands.add_parser("membership", help="write group membership")
    membership.add_argument("--group", type=_parse_group_argument, action="append", required=True)
    membership.add_argument("--control", type=pathlib.Path)
    membership.add_argument("--expect-count", type=_parse_count_argument, action="append")
    membership.add_argument("--output", type=pathlib.Path, required=True)
    membership.set_defaults(handler=_membership)

    summary = commands.add_parser("summarize", help="summarize a measured run")
    summary.add_argument("--junit", type=pathlib.Path, required=True)
    summary.add_argument("--membership", type=pathlib.Path, required=True)
    summary.add_argument("--log", type=pathlib.Path)
    summary.add_argument("--meta", type=pathlib.Path)
    summary.add_argument("--output", type=pathlib.Path, required=True)
    summary.set_defaults(handler=_summarize)
    return parser


def main(argv: typ.Sequence[str] | None = None) -> int:
    """Run the requested subcommand and return its process exit status.

    A malformed input exits ``2`` with one diagnostic rather than a traceback,
    so a workflow step fails naming the file it could not use.
    """
    args = _build_parser().parse_args(argv)
    try:
        return int(args.handler(args))
    except (BenchmarkError, OSError, ET.ParseError, tomllib.TOMLDecodeError) as error:
        print(f"nextest-split-benchmark: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())