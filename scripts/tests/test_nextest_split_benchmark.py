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


class TestConfiguration:
    """The control must differ from the deliverable only by group assignment."""

    def test_control_folds_only_the_trybuild_group(self, benchmark: types.ModuleType) -> None:
        control = benchmark.serial_control_text(CONFIG)
        assert control.count(f'test-group = "{benchmark.SERIAL_DYLINT_UI}"') == 2
        assert f'test-group = "{benchmark.SERIAL_TRYBUILD_CONTRACTS}"' not in control
        # The declaration survives; only the assignments move.
        assert f"{benchmark.SERIAL_TRYBUILD_CONTRACTS} = {{ max-threads = 1 }}" in control
        assert "test(contract_a) | test(contract_b)" in control

    def test_control_refuses_a_configuration_without_the_group(
        self, benchmark: types.ModuleType
    ) -> None:
        with pytest.raises(benchmark.BenchmarkError, match="would equal the deliverable"):
            benchmark.serial_control_text(CONFIG.replace("serial-trybuild-contracts", "serial-dylint-ui"))

    def test_junit_section_is_appended_once(self, benchmark: types.ModuleType) -> None:
        once = benchmark.with_junit(CONFIG, "default", pathlib.Path("/tmp/a.xml"))
        twice = benchmark.with_junit(once, "default", pathlib.Path("/tmp/b.xml"))
        assert twice.count("[profile.default.junit]") == 1
        assert 'path = "/tmp/b.xml"' in twice
        assert "/tmp/a.xml" not in twice

    def test_union_filter_joins_both_groups(self, benchmark: types.ModuleType) -> None:
        expression = benchmark.union_filter(CONFIG, "default")
        assert expression == "(test(ui::ui) | (binary(ui) & test(=ui))) | (test(contract_a) | test(contract_b))"


class TestMembership:
    """Membership is what classifies a JUnit report, so it must be exact."""

    def test_report_rejects_shared_members(self, benchmark: types.ModuleType) -> None:
        listing = {"rust-suites": {"crate::ui": {"testcases": {"ui": {}}}}}
        with pytest.raises(benchmark.BenchmarkError, match="share members"):
            benchmark.membership_report({"a": listing, "b": listing})

    def test_report_rejects_a_control_selecting_other_tests(
        self, benchmark: types.ModuleType
    ) -> None:
        listing = {"rust-suites": {"crate::ui": {"testcases": {"ui": {}}}}}
        with pytest.raises(benchmark.BenchmarkError, match="different test set"):
            benchmark.membership_report({"a": listing}, {"crate::ui::other"})

    def test_report_pins_an_expected_group_size(
        self, benchmark: types.ModuleType
    ) -> None:
        listing = {"rust-suites": {"crate::ui": {"testcases": {"ui": {}}}}}
        with pytest.raises(benchmark.BenchmarkError, match="expected 7"):
            benchmark.membership_report({"a": listing}, None, {"a": 7})
        assert benchmark.membership_report({"a": listing}, None, {"a": 1})["selection"]

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
        assert report["ignored"] == ["crate::integration_exclusion::excluded"]
        assert report["selection"] == ["crate::integration_exclusion::included"]

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
        assert report["selection"] == ["crate::ui::ui"]


class TestMeasurement:
    """Interval and outcome parsing decides what the evidence says."""

    def test_statuses_and_retries_are_recorded(self, benchmark: types.ModuleType) -> None:
        xml = _report(
            _testcase(benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=10),
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
        assert statuses == {
            "crate::ui::ui": "passed",
            "crate::api_contract::contract_a": "failure",
        }

    def test_retried_attempts_keep_only_the_last(self, benchmark: types.ModuleType) -> None:
        first = _testcase(
            benchmark,
            name="ui",
            start="2026-10-31T10:00:00.000Z",
            seconds=5,
            status="failure",
        )
        second = _testcase(benchmark, name="ui", start="2026-10-31T10:01:00.000Z", seconds=2)
        tests = benchmark.measure_junit(_report(first, second), _membership(benchmark))
        assert len(tests) == 1
        assert tests[0].retried is True
        assert tests[0].status == "passed"
        assert tests[0].seconds == 2

    def test_missing_timestamp_fails_loudly(self, benchmark: types.ModuleType) -> None:
        xml = '<testsuites><testsuite><testcase name="ui" classname="c::ui" time="1"/></testsuite></testsuites>'
        with pytest.raises(benchmark.BenchmarkError, match="has no timestamp"):
            benchmark.measure_junit(xml, _membership(benchmark))

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
        assert test.retried is True
        assert [attempt.seconds for attempt in test.intervals] == [5.0, 7.0]
        assert test.longest_attempt == 7.0


class TestOverlap:
    """Cross-group overlap is the claim the split is measured against."""

    def test_cross_group_overlap_is_counted(self, benchmark: types.ModuleType) -> None:
        xml = _report(
            _testcase(benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=60),
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
        assert report["cross_group_pairs"] == 1
        assert report["within_group_pairs"] == 0
        assert report["max_cross_group_concurrency"] == 2

    def test_within_group_overlap_is_reported(self, benchmark: types.ModuleType) -> None:
        membership = {
            "groups": {benchmark.SERIAL_DYLINT_UI: ["c::ui::one", "c::ui::two"]},
            "ignored": [],
            "selection": ["c::ui::one", "c::ui::two"],
        }
        xml = _report(
            _testcase(benchmark, name="one", classname="c::ui", start="2026-10-31T10:00:00.000Z", seconds=60),
            _testcase(benchmark, name="two", classname="c::ui", start="2026-10-31T10:00:10.000Z", seconds=60),
        )
        report = benchmark.overlap_report(benchmark.measure_junit(xml, membership))
        assert report["within_group_pairs"] == 1

    def test_touching_intervals_do_not_overlap(self, benchmark: types.ModuleType) -> None:
        xml = _report(
            _testcase(benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=60),
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
        assert report["cross_group_pairs"] == 0
        assert report["max_cross_group_concurrency"] == 1

    def test_retry_attempt_intervals_are_used(self, benchmark: types.ModuleType) -> None:
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
        assert report["within_group_pairs"] == 1

    def test_unclassified_tests_are_excluded_from_overlap(
        self, benchmark: types.ModuleType
    ) -> None:
        # A whole-suite run has thousands of tests outside both groups; they
        # are not subject to capacity one and must not be compared.
        xml = _report(
            _testcase(benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=60),
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
        report = benchmark.overlap_report(benchmark.measure_junit(xml, _membership(benchmark)))
        assert report["within_group_pairs"] == 0
        assert report["cross_group_pairs"] == 0
        assert report["max_cross_group_concurrency"] == 1

    def test_skipped_tests_are_excluded_from_overlap(self, benchmark: types.ModuleType) -> None:
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
        assert benchmark.overlap_report([t for t in tests if t.status != "skipped"]) == {
            "cross_group_pairs": 0,
            "within_group_pairs": 0,
            "max_cross_group_concurrency": 0,
            "cross_group_examples": [],
            "within_group_examples": [],
        }


class TestSummary:
    """The summary is the retained, machine-readable half of the evidence."""

    def test_summary_records_duration_and_slowest_tests(
        self, benchmark: types.ModuleType
    ) -> None:
        xml = _report(
            _testcase(benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=60),
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
        assert summary["nextest_seconds"] == 92.125
        assert summary["wall_seconds"] == 120.5
        assert [entry["name"] for entry in summary["slowest"]] == [
            "crate::api_contract::contract_a",
            "crate::ui::ui",
        ]
        assert summary["selected_not_run"] == []
        assert summary["counts"] == {"total": 2, "passed": 2, "failed": 0, "skipped": 0}

    def test_coloured_summary_is_readable(self, benchmark: types.ModuleType) -> None:
        # CI forces coloured output; the reset escape sits between the
        # heading and the duration.
        log = "\x1b[32;1m     Summary\x1b[0m [   0.406s] \x1b[1m1\x1b[0m test run"
        summary = benchmark.summarize(_report(), {"groups": {}, "ignored": [], "selection": []}, log)
        assert summary["nextest_seconds"] == 0.406

    def test_selected_but_absent_tests_are_reported(self, benchmark: types.ModuleType) -> None:
        xml = _report(
            _testcase(benchmark, name="ui", start="2026-10-31T10:00:00.000Z", seconds=1)
        )
        summary = benchmark.summarize(xml, _membership(benchmark))
        assert summary["selected_not_run"] == ["crate::api_contract::contract_a"]