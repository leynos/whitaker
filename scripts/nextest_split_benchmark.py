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


def serial_control_text(config_text: str) -> str:
    """Return the configuration with the trybuild group folded into the UI group.

    Only the ``test-group`` assignments change; every filter, allowance, retry
    policy and profile name stays as delivered. That is what makes the control
    comparable: the two runs differ in group assignment and nothing else.

    Raises
    ------
    BenchmarkError
        If the file names no trybuild group, so a control that silently ran the
        deliverable configuration is never mistaken for a comparison.
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
    return rewritten


def with_junit(config_text: str, profile: str, junit_path: pathlib.Path) -> str:
    """Return the configuration with a JUnit report path for one profile.

    The path is absolute so Nextest writes it wherever the caller collected the
    measurement, not relative to the workspace root.
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
    """Return the overrides in one profile that name a benchmark group."""
    document = tomllib.loads(config_text)
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
    """
    clauses = [
        f"({override['filter']})"
        for override in profile_overrides(config_text, profile, groups)
        if override.get("filter")
    ]
    if not clauses:
        raise BenchmarkError(f"no override in profile {profile!r} carries a filter")
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


def membership_report(
    groups: typ.Mapping[str, dict[str, typ.Any]],
    control_selection: set[str] | None = None,
    expected_counts: typ.Mapping[str, int] | None = None,
) -> dict[str, typ.Any]:
    """Return the group membership a JUnit report is classified against.

    ``groups`` maps a group name to a parsed ``cargo nextest list`` report.
    Memberships must be disjoint: a test counted in two groups would make the
    overlap measurement mean two things at once. ``expected_counts`` pins the
    size of a group, so a filter that silently stops selecting one of the seven
    compile contracts cannot become evidence.
    """
    memberships = {name: sorted(_qualified_names(report)) for name, report in groups.items()}
    # An ignored test appears in every listing whose binary was built, so the
    # record is a union rather than a per-group map that would count it twice.
    ignored = sorted(
        {test for report in groups.values() for test in _ignored_names(report)}
    )
    counts = collections.Counter(
        test for names in memberships.values() for test in names
    )
    overlaps = sorted(test for test, count in counts.items() if count > 1)
    if overlaps:
        raise BenchmarkError(f"test groups share members: {overlaps}")
    selection = sorted(counts)
    for name, expected in (expected_counts or {}).items():
        actual = len(memberships.get(name, ()))
        if actual != expected:
            raise BenchmarkError(
                f"group {name!r} selects {actual} tests, expected {expected}"
            )
    report: dict[str, typ.Any] = {
        "groups": memberships,
        "ignored": ignored,
        "selection": selection,
    }
    if control_selection is not None:
        if control_selection != set(selection):
            message = (
                "the control configuration selects a different test set: "
                f"missing={sorted(set(selection) - control_selection)} "
                f"extra={sorted(control_selection - set(selection))}"
            )
            raise BenchmarkError(message)
        report["control_selection_matches"] = True
    return report


def group_of(membership: dict[str, typ.Any], qualified_name: str) -> str:
    """Return the originating group of a test, or [`UNCLASSIFIED`]."""
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
    """Return every testcase in a JUnit report, classified by group."""
    root = ET.fromstring(xml_text)
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
    retried = {
        key for key, count in attempts.items() if count > 1
    } | {
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


def overlap_report(tests: typ.Sequence[MeasuredTest]) -> dict[str, typ.Any]:
    """Return cross-group and within-group interval overlaps.

    Strict overlap is the point of the split: the two capacity-one groups may
    run at the same time, while two tests from one group may not. A zero cross
    figure on a split run means the split brought no concurrency, and a
    non-zero within figure means the group capacity did not hold.

    Intervals are per attempt, so a retried test occupies the time it really
    ran rather than only the attempt that decided its outcome. Tests outside
    both groups are excluded: a whole-suite run contains thousands of them, and
    their overlaps say nothing about either capacity-one group.
    """
    windows = [
        (test.group, f"{test.name} (attempt {index})", attempt)
        for test in tests
        if test.group != UNCLASSIFIED
        for index, attempt in enumerate(test.intervals, start=1)
    ]
    ordered = sorted(windows, key=lambda window: window[2].started)
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

    return {
        "cross_group_pairs": len(cross_pairs),
        "within_group_pairs": len(within_pairs),
        "max_cross_group_concurrency": peak,
        "cross_group_examples": cross_pairs[:5],
        "within_group_examples": within_pairs[:5],
    }


def _nextest_seconds(log_text: str) -> float | None:
    """Return the duration Nextest reported for the run, if it reported one."""
    match = _SUMMARY_SECONDS.search(_ANSI_ESCAPE.sub("", log_text))
    return None if match is None else float(match.group(1))


def summarize(
    junit_text: str,
    membership: dict[str, typ.Any],
    log_text: str = "",
    meta: dict[str, typ.Any] | None = None,
) -> dict[str, typ.Any]:
    """Return a machine-readable summary of one measured run.

    The summary keeps every test interval, the fifteen slowest tests, the
    outcome counts, retry evidence, and the overlap report. Failed attempts are
    part of the record: a repetition that failed is reported, never replaced by
    a later one.
    """
    tests = measure_junit(junit_text, membership)
    counts = collections.Counter(test.status for test in tests)
    selection = set(membership["selection"])
    observed = {test.name for test in tests}
    slowest = sorted(tests, key=lambda test: test.longest_attempt, reverse=True)[:15]
    summary: dict[str, typ.Any] = {
        "meta": meta or {},
        "nextest_seconds": _nextest_seconds(log_text),
        "wall_seconds": (meta or {}).get("wall_seconds"),
        "exit_status": (meta or {}).get("exit_status"),
        "counts": {
            "total": len(tests),
            "passed": counts.get("passed", 0),
            "failed": counts.get("failure", 0) + counts.get("error", 0),
            "skipped": counts.get("skipped", 0),
        },
        "retried": sorted({test.name for test in tests if test.retried}),
        "selected_not_run": sorted(selection - observed),
        "unclassified": sorted(
            test.name
            for test in tests
            if test.group == UNCLASSIFIED and test.name in selection
        ),
        "slowest": [
            {
                "name": test.name,
                "group": test.group,
                "seconds": test.longest_attempt,
                "status": test.status,
                "attempts": len(test.intervals),
            }
            for test in slowest
        ],
        "overlap": overlap_report(
            [test for test in tests if test.status != "skipped"]
        ),
        "tests": [
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
        ],
    }
    return summary


def _write_json(path: pathlib.Path, payload: typ.Any) -> None:
    """Write a JSON document, creating its directory."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def _read_json(path: pathlib.Path) -> typ.Any:
    """Read a JSON document, naming the file when it cannot be read.

    A Nextest list report is one JSON document on one line. The report is
    captured through ``make test``, whose later steps write after it, so the
    reader also accepts a document embedded among other output.
    """
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        message = f"cannot read {path}: {error}"
    try:
        lines = path.read_text().splitlines()
    except OSError as read_error:
        raise BenchmarkError(f"cannot read {path}: {read_error}") from read_error
    for line in lines:
        stripped = line.strip()
        if not stripped.startswith("{"):
            continue
        try:
            return json.loads(stripped)
        except json.JSONDecodeError:
            continue
    raise BenchmarkError(message)


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
    config_text = args.config.read_text()
    if args.serial_control:
        config_text = serial_control_text(config_text)
    config_text = with_junit(config_text, args.profile, args.junit.resolve())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(config_text)
    return 0


def _union_filter(args: argparse.Namespace) -> int:
    """Print the filter selecting every benchmark test."""
    groups = tuple(args.group) if args.group else BENCHMARK_GROUPS
    print(union_filter(args.config.read_text(), args.profile, groups))
    return 0


def _membership(args: argparse.Namespace) -> int:
    """Write the group membership used to classify JUnit reports."""
    groups = {name: _read_json(path) for name, path in args.group}
    control = None if args.control is None else _read_json(args.control)
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
    log_text = "" if args.log is None else args.log.read_text(errors="replace")
    meta = {} if args.meta is None else _read_json(args.meta)
    summary = summarize(args.junit.read_text(), membership, log_text, meta)
    _write_json(args.output, summary)
    failed = summary["counts"]["failed"]
    status = summary["exit_status"]
    if failed or (status is not None and status != 0):
        print(
            f"{args.junit.parent.name}: {failed} failed tests, exit status {status}",
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
    """Run the requested subcommand."""
    args = _build_parser().parse_args(argv)
    try:
        return int(args.handler(args))
    except BenchmarkError as error:
        print(f"nextest-split-benchmark: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())