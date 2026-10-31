"""Test the Nextest split benchmark's selection and result validation.

Run with ``python -m pytest scripts/tests/test_nextest_split_benchmark.py``.
The benchmark workflow runs this before measuring, so a configuration or report
the helpers misread fails before it can become evidence.
"""

from __future__ import annotations

import importlib.util
import pathlib
import sys
import types

import pytest

SCRIPTS = pathlib.Path(__file__).resolve().parents[1]

CONFIG = """\
[profile.default]
slow-timeout = { period = "300s" }

[[profile.default.overrides]]
filter = "test(ui::ui) | (binary(ui) & test(=ui))"
test-group = "serial-dylint-ui"

[[profile.default.overrides]]
filter = "test(contract_a) | test(contract_b)"
test-group = "serial-trybuild-contracts"

[test-groups]
serial-dylint-ui = { max-threads = 1 }
serial-trybuild-contracts = { max-threads = 1 }
"""


@pytest.fixture(scope="module")
def benchmark() -> types.ModuleType:
    """Import the benchmark script as a module."""
    spec = importlib.util.spec_from_file_location(
        "nextest_split_benchmark", SCRIPTS / "nextest_split_benchmark.py"
    )
    if spec is None or spec.loader is None:
        message = "could not load the nextest split benchmark"
        raise ImportError(message)
    module = importlib.util.module_from_spec(spec)
    # A dataclass looks its own module up by name while decorating, so the
    # module must be registered before it executes.
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _testcase(
    benchmark: types.ModuleType,
    *,
    name: str,
    classname: str = "crate::ui",
    start: str,
    seconds: float,
    status: str = "passed",
) -> str:
    """Return one JUnit testcase element as text."""
    body = "" if status == "passed" else f"<{status}/>"
    return (
        f'<testcase name="{name}" classname="{classname}" '
        f'timestamp="{start}" time="{seconds}">{body}</testcase>'
    )


def _report(*cases: str) -> str:
    """Return a JUnit document holding the given testcases."""
    return (
        '<?xml version="1.0"?><testsuites><testsuite name="cargo-nextest">'
        + "".join(cases)
        + "</testsuite></testsuites>"
    )


def _membership(benchmark: types.ModuleType) -> dict[str, object]:
    """Return membership for one trybuild and one Dylint UI test."""
    return {
        "groups": {
            benchmark.SERIAL_TRYBUILD_CONTRACTS: ["crate::api_contract::contract_a"],
            benchmark.SERIAL_DYLINT_UI: ["crate::ui::ui"],
        },
        "ignored": [],
        "selection": ["crate::api_contract::contract_a", "crate::ui::ui"],
    }


def _single_test_listing() -> dict[str, object]:
    """Return a Nextest list report holding exactly one test."""
    return {"rust-suites": {"crate::ui": {"testcases": {"ui": {}}}}}


class TestConfiguration:
    """The control must differ from the deliverable only by group assignment."""

    def test_control_folds_only_the_trybuild_group(
        self, benchmark: types.ModuleType
    ) -> None:
        control = benchmark.serial_control_text(CONFIG)
        folded = control.count(f'test-group = "{benchmark.SERIAL_DYLINT_UI}"')
        assert folded == 2, (
            f"both assignments must fold to {benchmark.SERIAL_DYLINT_UI}; "
            f"got {folded} in:\n{control}"
        )
        assert f'test-group = "{benchmark.SERIAL_TRYBUILD_CONTRACTS}"' not in control, (
            f"no assignment may still name the trybuild group; got:\n{control}"
        )
        # The declaration survives; only the assignments move.
        declared = f"{benchmark.SERIAL_TRYBUILD_CONTRACTS} = {{ max-threads = 1 }}"
        assert declared in control, (
            f"the capacity-one declaration must survive the rewrite; got:\n{control}"
        )
        assert "test(contract_a) | test(contract_b)" in control, (
            f"the override filters must not change; got:\n{control}"
        )

    def test_control_refuses_a_configuration_without_the_group(
        self, benchmark: types.ModuleType
    ) -> None:
        without_group = CONFIG.replace(
            "serial-trybuild-contracts", "serial-dylint-ui"
        )
        with pytest.raises(
            benchmark.BenchmarkError, match="would equal the deliverable"
        ):
            benchmark.serial_control_text(without_group)

    def test_control_refuses_an_assignment_the_rewrite_misses(
        self, benchmark: types.ModuleType
    ) -> None:
        # TOML also accepts a literal (single-quoted) string for the same value.
        # The rewrite matches only the basic-string spelling, so this file lets
        # one substitution succeed and leaves the second override in the
        # trybuild group.
        wrapped = CONFIG + (
            "\n[[profile.default.overrides]]\n"
            'filter = "test(contract_c)"\n'
            "test-group = 'serial-trybuild-contracts'\n"
        )
        with pytest.raises(benchmark.BenchmarkError, match="survived the rewrite"):
            benchmark.serial_control_text(wrapped)

    def test_junit_section_is_appended_once(self, benchmark: types.ModuleType) -> None:
        once = benchmark.with_junit(CONFIG, "default", pathlib.Path("/tmp/a.xml"))
        twice = benchmark.with_junit(once, "default", pathlib.Path("/tmp/b.xml"))
        sections = twice.count("[profile.default.junit]")
        assert sections == 1, f"preparing twice must keep one section; got:\n{twice}"
        assert 'path = "/tmp/b.xml"' in twice, (
            f"the second path must replace the first; got:\n{twice}"
        )
        assert "/tmp/a.xml" not in twice, (
            f"the earlier path must not survive; got:\n{twice}"
        )

    def test_union_filter_joins_both_groups(self, benchmark: types.ModuleType) -> None:
        expression = benchmark.union_filter(CONFIG, "default")
        expected = (
            "(test(ui::ui) | (binary(ui) & test(=ui))) | "
            "(test(contract_a) | test(contract_b))"
        )
        assert expression == expected, (
            f"the union must join every group filter; got {expression!r}"
        )

    def test_invalid_configuration_is_reported(self, benchmark: types.ModuleType) -> None:
        with pytest.raises(benchmark.BenchmarkError, match="not valid TOML"):
            benchmark.profile_overrides("[profile.default", "default")


class TestMembership:
    """Membership is what classifies a JUnit report, so it must be exact."""

    def test_report_rejects_shared_members(self, benchmark: types.ModuleType) -> None:
        listing = _single_test_listing()
        with pytest.raises(benchmark.BenchmarkError, match="share members"):
            benchmark.membership_report({"a": listing, "b": listing})

    def test_report_rejects_a_control_selecting_other_tests(
        self, benchmark: types.ModuleType
    ) -> None:
        listing = _single_test_listing()
        with pytest.raises(benchmark.BenchmarkError, match="different test set"):
            benchmark.membership_report({"a": listing}, {"crate::ui::other"})

    def test_report_pins_an_expected_group_size(
        self, benchmark: types.ModuleType
    ) -> None:
        listing = _single_test_listing()
        with pytest.raises(benchmark.BenchmarkError, match="expected 7"):
            benchmark.membership_report({"a": listing}, None, {"a": 7})
        report = benchmark.membership_report({"a": listing}, None, {"a": 1})
        assert report["selection"] == ["crate::ui::ui"], (
            f"a matching pinned count must keep the selection; got {report!r}"
        )

    def test_ignored_tests_are_recorded_but_not_selected(
        self, benchmark: types.ModuleType
    ) -> None:
        # Observed from nextest 0.9.133: an ignored test is reported as a
        # mismatch whose reason is "ignored", so it is not part of the run.
        listing = {
            "rust-suites": {
                "crate::integration_exclusion": {
                    "testcases": {
                        "excluded": {
                            "ignored": True,
                            "filter-match": {"status": "mismatch", "reason": "ignored"},
                        },
                        "included": {
                            "ignored": False,
                            "filter-match": {"status": "matches"},
                        },
                    }
                }
            }
        }
        report = benchmark.membership_report({"a": listing})
        assert report["ignored"] == ["crate::integration_exclusion::excluded"], (
            f"an ignored test must stay visible; got {report!r}"
        )
        assert report["selection"] == ["crate::integration_exclusion::included"], (
            f"an ignored test must not be selected; got {report!r}"
        )

    def test_tests_the_filter_rejected_are_not_members(
        self, benchmark: types.ModuleType
    ) -> None:
        # Nextest lists every test in a binary it had to run, so the report
        # holds mismatches as well as matches; only matches belong to a group.
        listing = {
            "rust-suites": {
                "crate::ui": {
                    "testcases": {
                        "ui": {"ignored": False, "filter-match": {"status": "matches"}},
                        "unrelated": {
                            "ignored": False,
                            "filter-match": {"status": "mismatch", "reason": "string"},
                        },
                    }
                }
            }
        }
        report = benchmark.membership_report({"a": listing})
        assert report["selection"] == ["crate::ui::ui"], (
            f"a filter mismatch must not join a group; got {report!r}"
        )


class TestMeasurement:
    """Interval and outcome parsing decides what the evidence says."""

    def test_statuses_and_retries_are_recorded(
        self, benchmark: types.ModuleType
    ) -> None:
        xml = _report(
            _testcase(
                benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=10
            ),
            _testcase(
                benchmark,
                name="contract_a",
                classname="crate::api_contract",
                start="2026-10-31T10:00:20.000Z",
                seconds=5,
                status="failure",
            ),
        )
        tests = benchmark.measure_junit(xml, _membership(benchmark))
        statuses = {test.name: test.status for test in tests}
        expected = {
            "crate::ui::ui": "passed",
            "crate::api_contract::contract_a": "failure",
        }
        assert statuses == expected, f"the report's statuses must survive; got {statuses!r}"

    def test_retried_attempts_keep_only_the_last(
        self, benchmark: types.ModuleType
    ) -> None:
        first = _testcase(
            benchmark,
            name="ui",
            start="2026-10-31T10:00:00.000Z",
            seconds=5,
            status="failure",
        )
        second = _testcase(
            benchmark, name="ui", start="2026-10-31T10:01:00.000Z", seconds=2
        )
        tests = benchmark.measure_junit(_report(first, second), _membership(benchmark))
        assert len(tests) == 1, (
            f"a repeated classname/name pair is one test; got {tests!r}"
        )
        (test,) = tests
        assert test.retried is True, f"the repeat must be marked retried; got {test!r}"
        assert test.status == "passed", (
            f"the last attempt decides the outcome; got {test!r}"
        )
        assert test.seconds == 2, f"the last attempt's duration must be kept; got {test!r}"

    def test_missing_timestamp_fails_loudly(self, benchmark: types.ModuleType) -> None:
        xml = (
            '<testsuites><testsuite><testcase name="ui" classname="c::ui" '
            'time="1"/></testsuite></testsuites>'
        )
        with pytest.raises(benchmark.BenchmarkError, match="has no timestamp"):
            benchmark.measure_junit(xml, _membership(benchmark))

    def test_invalid_junit_is_reported(self, benchmark: types.ModuleType) -> None:
        with pytest.raises(benchmark.BenchmarkError, match="not valid XML"):
            benchmark.measure_junit("<testsuite>", _membership(benchmark))

    def test_rerun_attempts_are_collected(self, benchmark: types.ModuleType) -> None:
        # A failing test that was retried keeps the *first* attempt as its
        # testcase timestamp and records later attempts as rerun children.
        xml = (
            '<?xml version="1.0"?><testsuites><testsuite name="cargo-nextest">'
            '<testcase name="ui" classname="crate::ui" '
            'timestamp="2026-10-31T10:00:00.000Z" time="5">'
            "<failure/><rerunFailure "
            'timestamp="2026-10-31T10:05:00.000Z" time="7"/></testcase>'
            "</testsuite></testsuites>"
        )
        (test,) = benchmark.measure_junit(xml, _membership(benchmark))
        assert test.retried is True, f"a rerun child marks the test; got {test!r}"
        assert [attempt.seconds for attempt in test.intervals] == [5.0, 7.0], (
            f"both attempts must be kept; got {test.intervals!r}"
        )
        assert test.longest_attempt == 7.0, (
            f"the longest attempt must win the ranking; got {test.longest_attempt!r}"
        )


class TestOverlap:
    """Cross-group overlap is the claim the split is measured against."""

    def test_cross_group_overlap_is_counted(self, benchmark: types.ModuleType) -> None:
        xml = _report(
            _testcase(
                benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=60
            ),
            _testcase(
                benchmark,
                name="contract_a",
                classname="crate::api_contract",
                start="2026-10-31T10:00:30.000Z",
                seconds=60,
            ),
        )
        report = benchmark.overlap_report(
            benchmark.measure_junit(xml, _membership(benchmark))
        )
        assert report["cross_group_pairs"] == 1, (
            f"the two groups must overlap; got {report!r}"
        )
        assert report["within_group_pairs"] == 0, (
            f"one test per group cannot overlap within a group; got {report!r}"
        )
        assert report["max_cross_group_concurrency"] == 2, (
            f"both groups must be live at once; got {report!r}"
        )

    def test_within_group_overlap_is_reported(self, benchmark: types.ModuleType) -> None:
        membership = {
            "groups": {benchmark.SERIAL_DYLINT_UI: ["c::ui::one", "c::ui::two"]},
            "ignored": [],
            "selection": ["c::ui::one", "c::ui::two"],
        }
        xml = _report(
            _testcase(
                benchmark, name="one", classname="c::ui",
                start="2026-10-31T10:00:00.000Z", seconds=60,
            ),
            _testcase(
                benchmark, name="two", classname="c::ui",
                start="2026-10-31T10:00:10.000Z", seconds=60,
            ),
        )
        report = benchmark.overlap_report(benchmark.measure_junit(xml, membership))
        assert report["within_group_pairs"] == 1, (
            f"a capacity-one group must not run two tests at once; got {report!r}"
        )

    def test_touching_intervals_do_not_overlap(
        self, benchmark: types.ModuleType
    ) -> None:
        xml = _report(
            _testcase(
                benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=60
            ),
            _testcase(
                benchmark,
                name="contract_a",
                classname="crate::api_contract",
                start="2026-10-31T10:01:00.000Z",
                seconds=60,
            ),
        )
        report = benchmark.overlap_report(
            benchmark.measure_junit(xml, _membership(benchmark))
        )
        assert report["cross_group_pairs"] == 0, (
            f"intervals that only touch are not concurrent; got {report!r}"
        )
        assert report["max_cross_group_concurrency"] == 1, (
            f"only one group is live at any instant; got {report!r}"
        )

    def test_retry_attempt_intervals_are_used(
        self, benchmark: types.ModuleType
    ) -> None:
        membership = {
            "groups": {benchmark.SERIAL_DYLINT_UI: ["c::ui::one", "c::ui::two"]},
            "ignored": [],
            "selection": ["c::ui::one", "c::ui::two"],
        }
        xml = (
            '<?xml version="1.0"?><testsuites><testsuite name="cargo-nextest">'
            '<testcase name="one" classname="c::ui" '
            'timestamp="2026-10-31T10:00:00.000Z" time="5"><failure/>'
            '<rerunFailure timestamp="2026-10-31T10:00:30.000Z" time="5"/>'
            "</testcase>"
            '<testcase name="two" classname="c::ui" '
            'timestamp="2026-10-31T10:00:28.000Z" time="10"/>'
            "</testsuite></testsuites>"
        )
        report = benchmark.overlap_report(benchmark.measure_junit(xml, membership))
        # The deciding (first) attempt ended before "two" began; only the
        # retry overlaps it, so a report built from testcases alone would miss
        # this.
        assert report["within_group_pairs"] == 1, (
            f"a retry must contribute its real interval; got {report!r}"
        )

    def test_unclassified_tests_are_excluded_from_overlap(
        self, benchmark: types.ModuleType
    ) -> None:
        # A whole-suite run has thousands of tests outside both groups; they
        # are not subject to capacity one and must not be compared.
        xml = _report(
            _testcase(
                benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=60
            ),
            _testcase(
                benchmark,
                name="elsewhere",
                classname="crate::unit",
                start="2026-10-31T10:00:10.000Z",
                seconds=60,
            ),
            _testcase(
                benchmark,
                name="elsewhere_too",
                classname="crate::unit",
                start="2026-10-31T10:00:20.000Z",
                seconds=60,
            ),
        )
        report = benchmark.overlap_report(
            benchmark.measure_junit(xml, _membership(benchmark))
        )
        assert report["within_group_pairs"] == 0, (
            f"unclassified tests have no group capacity; got {report!r}"
        )
        assert report["cross_group_pairs"] == 0, (
            f"unclassified tests cannot overlap a group; got {report!r}"
        )
        assert report["max_cross_group_concurrency"] == 1, (
            f"only the Dylint UI group is live; got {report!r}"
        )

    def test_skipped_tests_are_excluded_from_overlap(
        self, benchmark: types.ModuleType
    ) -> None:
        xml = _report(
            _testcase(
                benchmark,
                name="ui",
                start="2026-10-31T10:00:00.000Z",
                seconds=60,
                status="skipped",
            )
        )
        tests = benchmark.measure_junit(xml, _membership(benchmark))
        report = benchmark.overlap_report(
            [test for test in tests if test.status != "skipped"]
        )
        expected = {
            "cross_group_pairs": 0,
            "within_group_pairs": 0,
            "max_cross_group_concurrency": 0,
            "cross_group_examples": [],
            "within_group_examples": [],
        }
        assert report == expected, f"a skipped test has no window; got {report!r}"


class TestSummary:
    """The summary is the retained, machine-readable half of the evidence."""

    def test_summary_records_duration_and_slowest_tests(
        self, benchmark: types.ModuleType
    ) -> None:
        xml = _report(
            _testcase(
                benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=60
            ),
            _testcase(
                benchmark,
                name="contract_a",
                classname="crate::api_contract",
                start="2026-10-31T10:00:10.000Z",
                seconds=90,
            ),
        )
        summary = benchmark.summarize(
            xml,
            _membership(benchmark),
            "Summary [  92.125s] 2 tests run: 2 passed",
            {"wall_seconds": 120.5, "exit_status": 0},
        )
        assert summary["nextest_seconds"] == 92.125, (
            f"the summary duration must be parsed; got {summary!r}"
        )
        assert summary["wall_seconds"] == 120.5, (
            f"the recorded wall time must be kept; got {summary!r}"
        )
        slowest = [entry["name"] for entry in summary["slowest"]]
        assert slowest == [
            "crate::api_contract::contract_a",
            "crate::ui::ui",
        ], f"the longest test must rank first; got {slowest!r}"
        assert summary["selected_not_run"] == [], (
            f"both selected tests ran; got {summary!r}"
        )
        assert summary["counts"] == {
            "total": 2,
            "passed": 2,
            "failed": 0,
            "skipped": 0,
        }, f"the outcome counts must be exact; got {summary['counts']!r}"

    def test_coloured_summary_is_readable(self, benchmark: types.ModuleType) -> None:
        # CI forces coloured output; the reset escape sits between the
        # heading and the duration.
        log = "\x1b[32;1m     Summary\x1b[0m [   0.406s] \x1b[1m1\x1b[0m test run"
        summary = benchmark.summarize(
            _report(), {"groups": {}, "ignored": [], "selection": []}, log
        )
        assert summary["nextest_seconds"] == 0.406, (
            f"ANSI escapes must not hide the duration; got {summary!r}"
        )

    def test_selected_but_absent_tests_are_reported(
        self, benchmark: types.ModuleType
    ) -> None:
        xml = _report(
            _testcase(
                benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=1
            )
        )
        summary = benchmark.summarize(xml, _membership(benchmark))
        assert summary["selected_not_run"] == [
            "crate::api_contract::contract_a"
        ], f"an unrun selection must be visible; got {summary!r}"


class TestCommandLine:
    """A malformed input must exit 2 with one diagnostic, not a traceback."""

    def test_invalid_configuration_exits_two(
        self, benchmark: types.ModuleType, tmp_path: pathlib.Path, capsys: object
    ) -> None:
        config = tmp_path / "nextest.toml"
        config.write_text("[profile.default")
        status = benchmark.main(
            ["union-filter", "--config", str(config), "--profile", "default"]
        )
        assert status == 2, f"a malformed configuration must exit 2; got {status}"
        captured = capsys.readouterr()
        assert "not valid TOML" in captured.err, (
            f"the diagnostic must name the problem; got {captured.err!r}"
        )