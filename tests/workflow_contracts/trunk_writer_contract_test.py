"""Every pull-request lane on the Actions backend has one trunk writer.

`trunk_writer` explains the rule: the cache proxy is ref-scoped, so a pull
request's first push is warm only for the shapes a push to `main` compiles.
These tests hold the checked-in workflows to it, and drive each reader over
synthetic documents so a refusal is proved by a case rather than by the
repository happening to comply.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from pull_request_reach import declares_trigger
from trunk_writer import (
    UnreviewedConditionError,
    compiling_commands,
    push_jobs,
    runs_on,
    trunk_push_violations,
    writer_violations,
)
from ubicloud_workflow_support import (
    UBICLOUD_JOBS,
    backend_for,
    load_job,
    load_workflow,
    parse_workflow,
)

#: Each pull-request lane on the Actions backend, mapped to the one job that
#: compiles its shapes on a push to `main`. `linux-full` is its own writer:
#: `ci.yml` runs it on that push, so its shapes are the lane's by construction
#: rather than by a copy that could drift.
TRUNK_WRITERS: typ.Final[dict[str, str]] = {
    "coverage-check": "coverage-upload",
    "linux-full": "linux-full",
}


def _pull_request_lanes_on_gha() -> set[str]:
    """Return the Ubicloud jobs on the Actions backend a pull request runs."""
    lanes: set[str] = set()
    for job_name, workflow_name in UBICLOUD_JOBS.items():
        workflow = load_workflow(workflow_name)
        if (
            backend_for(workflow_name) == "gha"
            and declares_trigger(workflow, "pull_request")
            and runs_on(load_job(job_name), "pull_request")
        ):
            lanes.add(job_name)
    return lanes


def test_every_pull_request_lane_on_gha_has_a_registered_writer() -> None:
    """The presence half: a new lane cannot join without naming its writer."""
    lanes = _pull_request_lanes_on_gha()
    assert lanes == set(TRUNK_WRITERS), (
        f"pull-request lanes on the Actions backend are {sorted(lanes)}, but "
        f"TRUNK_WRITERS registers {sorted(TRUNK_WRITERS)}"
    )


@pytest.mark.parametrize(("reader", "writer"), sorted(TRUNK_WRITERS.items()))
def test_each_writer_compiles_its_readers_shapes_on_the_trunk(
    reader: str, writer: str
) -> None:
    """The writer runs on every push to `main` and runs what the reader runs."""
    writer_workflow = UBICLOUD_JOBS[writer]
    violations = writer_violations(
        load_job(reader), load_job(writer), load_workflow(writer_workflow)
    )
    assert not violations, f"{writer} as {reader}'s trunk writer: {violations}"
    assert backend_for(writer_workflow) == backend_for(UBICLOUD_JOBS[reader]), (
        f"{writer} must write the backend {reader} reads"
    )


def test_only_the_registered_writers_run_on_the_trunk_push() -> None:
    """No second job writes `main`'s scope, and no writer is missing from it.

    Read over every workflow on the Actions backend, so a job added to either
    one, or a lane whose event guard is dropped, shows up here.
    """
    gha_workflows = sorted(
        {name for name in UBICLOUD_JOBS.values() if backend_for(name) == "gha"}
    )
    running = {job for name in gha_workflows for job in push_jobs(load_workflow(name))}
    assert running == set(TRUNK_WRITERS.values()), (
        f"the jobs that run on a push to main are {sorted(running)}, but only "
        f"the trunk writers {sorted(set(TRUNK_WRITERS.values()))} may"
    )


_TRUNK: typ.Final[str] = "on:\n  push:\n    branches: [main]\n"


@pytest.mark.parametrize(
    ("triggers", "expected"),
    [
        pytest.param("on: pull_request\n", "does not run", id="no-push-trigger"),
        pytest.param("on: push\n", "not every branch", id="a-scalar-push"),
        pytest.param("on: [push, pull_request]\n", "not every branch", id="a-list"),
        pytest.param("on:\n  push:\n", "not every branch", id="an-unfiltered-push"),
        pytest.param(
            "on:\n  push:\n    branches: ['**']\n",
            "must be ['main']",
            id="every-branch",
        ),
        pytest.param(
            "on:\n  push:\n    branches: [main, develop]\n",
            "must be ['main']",
            id="a-second-branch",
        ),
        pytest.param(
            f"{_TRUNK}    paths: ['src/**']\n", "filter on paths", id="a-paths-filter"
        ),
        pytest.param(
            "on:\n  push:\n    branches-ignore: [dev]\n",
            "filter on branches-ignore",
            id="an-ignore-filter",
        ),
    ],
)
def test_a_push_that_is_not_exactly_the_trunk_is_refused(
    triggers: str, expected: str
) -> None:
    """Each way of widening, narrowing or dropping the push is caught."""
    document = parse_workflow(f"{triggers}jobs: {{}}\n")
    violations = trunk_push_violations(document)
    assert any(expected in violation for violation in violations), violations


@pytest.mark.parametrize(
    "triggers",
    [
        pytest.param(_TRUNK, id="a-list"),
        pytest.param("on:\n  push:\n    branches: main\n", id="a-scalar-branch"),
        pytest.param(f'"on":\n{_TRUNK[4:]}', id="a-quoted-on-key"),
    ],
)
def test_a_push_to_main_alone_is_accepted(triggers: str) -> None:
    """The narrow half: every spelling of "push to main" passes."""
    assert trunk_push_violations(parse_workflow(f"{triggers}jobs: {{}}\n")) == []


_READER: typ.Final = {"steps": [{"run": "make lint"}, {"run": "make publish-check"}]}


@pytest.mark.parametrize(
    ("writer", "expected"),
    [
        pytest.param(
            {"if": "github.event_name == 'pull_request'", **_READER},
            "unconditionally",
            id="a-conditional-writer",
        ),
        pytest.param(
            {"steps": [{"run": "make lint"}]},
            "make publish-check",
            id="a-writer-missing-a-command",
        ),
        pytest.param(
            {"steps": [{"run": "echo make lint"}, {"run": "make publish-check"}]},
            "make lint",
            id="a-writer-that-only-mentions-a-command",
        ),
    ],
)
def test_a_writer_that_does_not_compile_the_readers_shapes_is_refused(
    writer: dict[str, typ.Any], expected: str
) -> None:
    """The writer must run, unconditionally, every command the reader runs."""
    violations = writer_violations(_READER, writer, parse_workflow(_TRUNK))
    assert any(expected in violation for violation in violations), violations


def test_a_writer_running_the_readers_commands_is_accepted() -> None:
    """The narrow half: the same job as its own writer passes."""
    assert writer_violations(_READER, _READER, parse_workflow(_TRUNK)) == []


def test_only_make_and_cargo_steps_count_as_compiling() -> None:
    """Checks, installs, uploads and mere mentions are not shapes to repeat."""
    job = {
        "steps": [
            {"run": "make lint"},
            {"run": "cargo build --workspace"},
            {"run": "bash scripts/record-sccache-effectiveness.sh"},
            {"run": "echo make lint is next"},
            {"uses": "actions/checkout@abc"},
        ]
    }
    assert compiling_commands(job) == {"make lint", "cargo build --workspace"}


@pytest.mark.parametrize(
    ("condition", "event", "expected"),
    [
        pytest.param(None, "push", True, id="no-condition"),
        pytest.param(
            "github.event_name == 'pull_request'", "push", False, id="pr-only-on-push"
        ),
        pytest.param(
            "${{ github.event_name == 'pull_request' }}",
            "pull_request",
            True,
            id="pr-only-on-a-pr",
        ),
        pytest.param("github.event_name != 'push'", "push", False, id="not-on-push"),
        pytest.param(
            "github.event_name  !=  'push'",
            "workflow_dispatch",
            True,
            id="not-on-push-on-a-dispatch",
        ),
    ],
)
def test_reviewed_conditions_admit_the_events_they_say(
    condition: str | None, event: str, expected: bool
) -> None:
    """Each reviewed spelling admits exactly the events it names."""
    job = {} if condition is None else {"if": condition}
    assert runs_on(job, event) is expected


def test_an_unreviewed_condition_is_refused_rather_than_guessed() -> None:
    """A guess would read `always()` or `false` as whichever answer passes."""
    with pytest.raises(UnreviewedConditionError):
        runs_on({"if": "github.ref == 'refs/heads/main' || always()"}, "push")


def test_an_ungated_job_beside_the_writer_is_a_second_writer() -> None:
    """A job the push also starts writes the same scope as the writer."""
    document = parse_workflow(
        f"{_TRUNK}jobs:\n  writer: {{}}\n  windows: {{}}\n"
        "  pr-only:\n    if: github.event_name == 'pull_request'\n"
    )
    assert push_jobs(document) == ["windows", "writer"]
