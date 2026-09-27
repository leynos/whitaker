"""Hold `setup-rust` as the single owner of every Ubicloud job's compiler cache.

shared-actions ADR 0005 makes `setup-rust` select sccache's backend from the
runner. On Ubicloud it exports the cache-proxy credentials, clears the v2
cache-service flag the proxy does not serve, and carries that cleared value
past `mozilla-actions/sccache-action`, which sets it again. On a
GitHub-hosted runner it keeps sccache on local disk, which it caches itself
when it owns the job's other caches.

This repository wired all of that by hand: an `Export the Ubicloud cache
credentials` step, a `SCCACHE_BACKEND` switch translated by
`scripts/select-sccache-backend.sh`, and a local directory for the
rolling-release lanes archived under the `sccache-rolling-v1-` and
`sccache-depbin-v1-` families. The last of those went wrong silently: its
restores ran before `Setup Rust` and read the proxy, its saves ran after it and
went to GitHub, so every run was cold. Each piece would now override
`setup-rust`'s choice, because a caller's wrapper, directory or switch wins.

The rules: every Ubicloud job calls `setup-rust` once with sccache on, the id
its report reads and the `expect-cache` its placement allows; no retired piece
survives in any of them; nothing touches sccache before `setup-rust` starts
it; and the statistics name the backend, because `Cache location` reads
`ghac` for the proxy and GitHub's own service alike.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest
from runner_lanes import GITHUB_HOSTED_LABELS, declared_labels
from sccache_steps import mentions_sccache
from ubicloud_workflow_support import (
    SCCACHE_DIRECTORY,
    SETUP_RUST_ACTION,
    UBICLOUD_JOBS,
    cache_paths,
    load_job,
    load_workflow,
    step_names,
    steps_by_name,
)

#: The id every Ubicloud job gives `setup-rust`, so its statistics can read
#: the backend from `steps.setup-rust.outputs.cache-backend`.
SETUP_RUST_ID: typ.Final[str] = "setup-rust"

#: Variables that configured the retired hand-rolled arrangement. A caller's
#: value wins over `setup-rust`'s choice, so any of these left in a Ubicloud
#: workflow silently overrides the runner-aware backend.
RETIRED_VARIABLES: typ.Final[tuple[str, ...]] = (
    "RUSTC_WRAPPER",
    "SCCACHE_BACKEND",
    "SCCACHE_DIR",
    "SCCACHE_CACHE_SIZE",
    "SCCACHE_GHA_ENABLED",
)

#: The retired steps, by the action or script each ran.
RETIRED_CREDENTIALS_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/export-ubicloud-cache-credentials"
)
RETIRED_SELECTOR: typ.Final[str] = "scripts/select-sccache-backend.sh"

#: Steps a job may not run itself, because `setup-rust` does them.
BESPOKE_STEPS: typ.Final[tuple[str, ...]] = (
    "Install sccache",
    "Reset sccache statistics",
)

#: The step whose report must name the backend.
EFFECTIVENESS_STEP: typ.Final[str] = "Record sccache effectiveness"


def _env_findings(scope: str, env: object) -> list[str]:
    """Return the retired variables one `env` mapping sets."""
    if not isinstance(env, dict):
        return []
    return [f"{scope} sets {name}" for name in RETIRED_VARIABLES if name in env]


def _step_findings(index: int, step: dict[str, typ.Any]) -> list[str]:
    """Return the retired pieces one step carries."""
    findings = _env_findings(f"step {index}", step.get("env"))
    uses = str(step.get("uses", ""))
    if uses.startswith(f"{RETIRED_CREDENTIALS_ACTION}@"):
        findings.append(f"step {index} exports the Ubicloud credentials itself")
    if RETIRED_SELECTOR in str(step.get("run", "")):
        findings.append(f"step {index} runs the retired backend selector")
    if step.get("name") in BESPOKE_STEPS:
        findings.append(f"step {index} is a bespoke {step['name']!r}")
    if uses.startswith("actions/cache") and SCCACHE_DIRECTORY in cache_paths(step):
        findings.append(f"step {index} archives {SCCACHE_DIRECTORY}")
    return findings


def retired_findings(
    workflow_env: object, job: dict[str, typ.Any]
) -> list[str]:
    """Return every retired compiler-cache piece a job still carries.

    Pure over the parsed data, so the rule runs on fixtures as well as on the
    checked-in workflows.

    Parameters
    ----------
    workflow_env : object
        The workflow's top-level `env` mapping, or anything else when it
        declares none.
    job : dict[str, Any]
        One parsed job.

    Returns
    -------
    list[str]
        One description per finding; empty for a job that leaves sccache to
        `setup-rust`.

    >>> retired_findings({}, {"steps": [{"name": "Checkout"}]})
    []
    >>> retired_findings(
    ...     {"SCCACHE_BACKEND": "gha"},
    ...     {"steps": [{"name": "Pick", "run": "bash scripts/select-sccache-backend.sh"}]},
    ... )
    ['workflow sets SCCACHE_BACKEND', 'step 0 runs the retired backend selector']
    """
    findings = _env_findings("workflow", workflow_env)
    findings += _env_findings("job", job.get("env"))
    steps = job.get("steps")
    for index, step in enumerate(steps if isinstance(steps, list) else []):
        findings += _step_findings(index, step)
    return findings


def expected_expect_cache(labels: typ.Iterable[str]) -> str:
    """Return the `expect-cache` value a job's placement calls for.

    Parameters
    ----------
    labels : Iterable[str]
        Every runner label the job can resolve to.

    Returns
    -------
    str
        ``"any"`` when a GitHub-hosted label is among them, else
        ``"ubicloud"``.

    >>> expected_expect_cache(["ubicloud-standard-2-ubuntu-2404", "windows-latest"])
    'any'
    >>> expected_expect_cache(["ubicloud-standard-2-ubuntu-2404"])
    'ubicloud'
    """
    return "any" if set(labels) & GITHUB_HOSTED_LABELS else "ubicloud"


def _setup_rust(job_name: str) -> dict[str, typ.Any]:
    """Return a job's one `Setup Rust` step."""
    job = load_job(job_name)
    calls = [s for s in job["steps"] if str(s.get("uses", "")) == SETUP_RUST_ACTION]
    assert len(calls) == 1, (
        f"{job_name} must call the reviewed setup-rust exactly once, "
        f"found {len(calls)}"
    )
    return calls[0]


JOBS = sorted(UBICLOUD_JOBS)


def test_the_rule_reaches_every_ubicloud_job() -> None:
    """The presence half: the rolling-release jobs are held too.

    They were the ones on the retired local directory, so a rule that had
    quietly lost them would pass over the very lanes it exists for.
    """
    for job_name in ("coverage-check", "linux-full", "coverage-upload",
                     "build-lints", "build-dependency-binaries"):
        assert job_name in JOBS, f"{job_name} must be held by this rule"


@pytest.mark.parametrize("job_name", JOBS)
def test_setup_rust_owns_the_compiler_cache(job_name: str) -> None:
    """One reviewed call, sccache on, and an id the report reads."""
    step = _setup_rust(job_name)
    inputs = step.get("with") or {}
    assert str(inputs.get("use-sccache", "true")) == "true", (
        f"{job_name} switches setup-rust's sccache off, so the job compiles "
        "with no compiler cache at all"
    )
    assert step.get("id") == SETUP_RUST_ID, (
        f"{job_name} must give setup-rust the id {SETUP_RUST_ID!r}, or its "
        "statistics cannot name the backend"
    )


@pytest.mark.parametrize("job_name", JOBS)
def test_each_job_demands_the_backend_its_placement_allows(job_name: str) -> None:
    """`ubicloud` fails a proxy-less job loudly; `any` spares a hosted leg.

    A matrix with GitHub-hosted legs would fail every such leg under
    `ubicloud`; a job only on Ubicloud would compile against local disk
    unnoticed under `any` whenever the proxy went missing.
    """
    expected = expected_expect_cache(declared_labels(load_job(job_name)))
    inputs = _setup_rust(job_name).get("with") or {}
    assert inputs.get("expect-cache") == expected, (
        f"{job_name} must pass expect-cache: {expected}, not "
        f"{inputs.get('expect-cache')!r}"
    )


@pytest.mark.parametrize("job_name", JOBS)
def test_no_ubicloud_job_keeps_a_retired_piece(job_name: str) -> None:
    """A caller's wrapper, directory or switch overrides `setup-rust` silently."""
    findings = retired_findings(
        load_workflow(UBICLOUD_JOBS[job_name]).get("env"), load_job(job_name)
    )
    assert not findings, f"{job_name} still hand-rolls sccache: {findings}"


@pytest.mark.parametrize("job_name", JOBS)
def test_nothing_touches_sccache_before_setup_rust_starts_it(job_name: str) -> None:
    """sccache binds its backend once, when its server starts.

    A step that touched sccache before `Setup Rust` would start a server of
    its own, bound to whatever that step's environment named.
    """
    job = load_job(job_name)
    names = step_names(job)
    setup_at = names.index("Setup Rust")
    early = [
        name
        for index, name in enumerate(names[:setup_at])
        if mentions_sccache(job["steps"][index])
    ]
    assert not early, f"{job_name} touches sccache before Setup Rust: {early}"


@pytest.mark.parametrize("job_name", JOBS)
def test_the_statistics_name_the_backend(job_name: str) -> None:
    """`Cache location` reads `ghac` for the proxy and GitHub alike."""
    step = steps_by_name(load_job(job_name)).get(EFFECTIVENESS_STEP)
    assert step is not None, f"{job_name} must run {EFFECTIVENESS_STEP!r}"
    backend = f"steps.{SETUP_RUST_ID}.outputs.cache-backend"
    assert backend in str((step.get("env") or {}).get("SETUP_RUST_CACHE_BACKEND", "")), (
        f"{job_name}'s {EFFECTIVENESS_STEP!r} must pass {backend} as "
        "SETUP_RUST_CACHE_BACKEND"
    )


@pytest.mark.parametrize(
    ("job", "expected"),
    [
        pytest.param({"env": {"SCCACHE_DIR": "x"}, "steps": []}, 1, id="job-dir"),
        pytest.param(
            {
                "steps": [
                    {
                        "uses": f"{RETIRED_CREDENTIALS_ACTION}@abc",
                        "name": "Export",
                    }
                ]
            },
            1,
            id="credentials",
        ),
        pytest.param(
            {"steps": [{"name": "Reset sccache statistics", "run": "x"}]},
            1,
            id="bespoke-reset",
        ),
        pytest.param(
            {
                "steps": [
                    {
                        "name": "Restore",
                        "uses": "actions/cache/restore@abc",
                        "with": {"path": SCCACHE_DIRECTORY},
                    }
                ]
            },
            1,
            id="archive",
        ),
        pytest.param(
            {"steps": [{"name": EFFECTIVENESS_STEP, "run": "bash x.sh"}]},
            0,
            id="report",
        ),
        pytest.param(
            {
                "steps": [
                    {
                        "name": "Restore",
                        "uses": "actions/cache/restore@abc",
                        "with": {"path": "~/.cargo/registry"},
                    }
                ]
            },
            0,
            id="registry-cache",
        ),
    ],
)
def test_the_retired_piece_reader_is_narrow_as_well_as_sufficient(
    job: dict[str, typ.Any], expected: int
) -> None:
    """Each retired form is caught, and the report and other caches are not.

    The checked-in jobs can only show that the rule passes on them. These
    fixtures show that it would catch each retired form, and that it leaves
    the statistics report and the Cargo registry cache alone.
    """
    findings = retired_findings({}, job)
    assert len(findings) == expected, (
        f"expected {expected} finding(s) for {job!r}, got {findings!r}"
    )
