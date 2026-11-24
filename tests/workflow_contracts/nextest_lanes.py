"""The single-filter lanes that each carry their own nextest budget.

Split from ``nextest_overrides`` so no module outgrows the 400-line limit
``AGENTS.md`` sets. That module keeps the clauses that select a family of
tests — the compile contracts, the whole-UI harnesses and the broad UI
disjunction — while this one holds the lanes budgeted one filter at a time.

Every lane here declares the same allowance in both profiles, so the table is
written once and composed into each profile. Restating seven identical entries
twice is how the divergence these tables exist to catch would start.

The ``Allowance`` type lives here because these lanes state a budget, a test
group and a retry policy together; ``nextest_overrides`` imports it for the
clauses it keeps and re-exports it for the contract tests.
"""

from __future__ import annotations

import typing as typ

# Parser-pin fixtures share one integration binary and Cargo package cache, so
# the allowance must cover that whole binary and serialize it.
BUILD_SCRIPT_INTEGRATION_FILTER: typ.Final[str] = "binary(build_script_integration)"
NO_DEFAULT_FEATURES_FILTER: typ.Final[str] = "binary(no_default_features_build)"
ISOLATED_CARGO_CHECKS_FILTER: typ.Final[str] = (
    "test(crate_builds_without_dylint_driver_feature) | "
    "test(fluent_bundles_are_included_in_the_package_tarball)"
)
#: The other whole-binary UI clause. `macro_aware_cognitive_complexity` has
#: four harnesses in one `tests/ui.rs` binary; only the one named exactly `ui`
#: is caught by the broad clause's integration-binary term, so the remaining
#: three fell to the base 300 s bound and were killed mid-build. Matching the
#: package's own binary covers every harness the crate adds later.
MACRO_AWARE_UI_FILTER: typ.Final[str] = (
    "package(macro_aware_cognitive_complexity) & binary(ui)"
)
#: The logging feature-graph matrix compares the same four `tracing` calls with
#: and without `tracing`'s `log` compatibility feature. That feature changes how
#: the macros expand, so the score must not move (ExecPlan V5). The exact test
#: name is pinned rather than a `feature_matrix` substring, so a later test in
#: the same crate whose name contains that word cannot join the 120-minute lane.
FEATURE_MATRIX_FILTER: typ.Final[str] = (
    "package(macro_aware_cognitive_complexity) & "
    "test(feature_matrix_log_feature_does_not_change_score)"
)
#: The macro-free parity test builds the lint library, then runs `cargo clippy`
#: and `cargo dylint` in a generated fixture. Three nested builds wait on
#: compiler admission, so a cold run measured 380 s against the 300 s base
#: bound and a loaded run measured 1,451 s: it takes a nested-build lane with
#: no retry, because each retry repeats all three builds. The lane is the
#: 75-minute build-sized allowance because a later run was killed at the
#: 30-minute bound while admission was deeply queued.
PARITY_FILTER: typ.Final[str] = (
    "package(macro_aware_cognitive_complexity) & "
    "test(macro_free_scores_match_pinned_clippy)"
)
#: The suite smoke test stages the suite cdylib and then runs `cargo dylint`
#: twice over a fixed consumer. Several nested builds wait on compiler
#: admission, so it shares the parity test's hazard and its nested build lane.
#: The exact test name is pinned so a later smoke test in the package cannot
#: silently join that lane.
SUITE_SMOKE_FILTER: typ.Final[str] = (
    "package(whitaker_suite) & "
    "test(suite_runs_the_macro_aware_lint_in_a_real_driver)"
)


class Allowance(typ.NamedTuple):
    """One override's whole contribution to a profile.

    Attributes
    ----------
    slow_timeout : dict[str, object]
        The whole ``slow-timeout`` table the override must declare.
    test_group : str or None
        The group it must name, or None when it names none. The budget
        and group are one claim: nested Cargo builds with long allowances
        share one serial group so cold fixtures do not compete for admission.
    retries : integer, dict[str, object], or None
        The whole ``retries`` setting, or None when the override declares
        none. The Dylint UI harnesses retry transient Windows rename failures;
        the large nested UI harnesses and compile contracts disable retries
        because each retry repeats a costly Cargo build.
    """

    slow_timeout: dict[str, object]
    test_group: str | None
    retries: int | dict[str, object] | None = None


#: The 75-minute build-sized allowance the nested-Cargo lanes share.
#:
#: One long attempt covers every nested build a lane makes, which is why the
#: lanes below also disable retries: a retry repeats the whole build. The
#: value is stated once because these lanes rise and fall together — a run
#: killed at 30 minutes with admission deeply queued raised all of them, and a
#: later measurement should move them together too.
BUILD_SIZED_SLOW_TIMEOUT: typ.Final[dict[str, object]] = {
    "period": "75m",
    "terminate-after": 1,
    "grace-period": "5s",
}

#: The feature-matrix override's whole contribution.
#: One long attempt covers two cold dependency graphs and two nested Dylint
#: builds; a retry would repeat both. The dedicated serial group keeps it off
#: the nested-build lanes.
FEATURE_MATRIX_ALLOWANCE: typ.Final[Allowance] = Allowance(
    slow_timeout={
        "period": "120m",
        "terminate-after": 1,
        "grace-period": "5s",
    },
    test_group="serial-feature-matrix",
    retries=0,
)

#: The parity override's whole contribution. One long attempt covers three
#: nested builds; the nested-build group keeps it off the serialized UI lanes
#: while still sharing compiler admission. It carries the build-sized
#: allowance, the same as the suite smoke test: a measured run was killed at 30
#: minutes with admission deeply queued.
PARITY_ALLOWANCE: typ.Final[Allowance] = Allowance(
    slow_timeout=BUILD_SIZED_SLOW_TIMEOUT,
    test_group="serial-nested-cargo-builds",
    retries=0,
)

#: The suite smoke override's whole contribution.
#: It stages the suite cdylib and runs `cargo dylint` twice over a fixed
#: consumer; a retry would repeat every build, and the nested-build group keeps
#: it from racing the parity test's three builds. A measured 30-minute attempt
#: was still waiting on compiler admission for the consumer build, so it takes
#: the build-sized allowance the nested example and whole UI harness fixtures
#: get.
SUITE_SMOKE_ALLOWANCE: typ.Final[Allowance] = Allowance(
    slow_timeout=BUILD_SIZED_SLOW_TIMEOUT,
    test_group="serial-nested-cargo-builds",
    retries=0,
)

#: Every lane both profiles declare, keyed by the filter that selects it.
#:
#: Composed into each profile's table by ``nextest_overrides`` rather than
#: restated, so a lane cannot exist in one profile and be missing from the
#: other. The keys are stated whole and matched exactly, per profile, because a
#: substring would accept a narrowed filter that leaves each selected test name
#: intact while dropping the allowance it needs.
LANE_OVERRIDES: typ.Final[dict[str, Allowance]] = {
    BUILD_SCRIPT_INTEGRATION_FILTER: Allowance(
        slow_timeout=BUILD_SIZED_SLOW_TIMEOUT,
        test_group="serial-nested-cargo-builds",
    ),
    NO_DEFAULT_FEATURES_FILTER: Allowance(
        slow_timeout={
            "period": "20m",
            "terminate-after": 1,
            "grace-period": "5s",
        },
        test_group="serial-nested-cargo-builds",
    ),
    ISOLATED_CARGO_CHECKS_FILTER: Allowance(
        slow_timeout={
            "period": "30m",
            "terminate-after": 1,
            "grace-period": "5s",
        },
        test_group="serial-nested-cargo-builds",
    ),
    MACRO_AWARE_UI_FILTER: Allowance(
        slow_timeout={
            "period": "30m",
            "terminate-after": 1,
            "grace-period": "5s",
        },
        test_group="serial-dylint-ui",
        retries={"backoff": "exponential", "count": 2, "delay": "5s"},
    ),
    FEATURE_MATRIX_FILTER: FEATURE_MATRIX_ALLOWANCE,
    PARITY_FILTER: PARITY_ALLOWANCE,
    SUITE_SMOKE_FILTER: SUITE_SMOKE_ALLOWANCE,
}