"""Every pull-request lane on the Actions backend has one trunk writer.

`trunk_writer` explains the rule: the cache proxy is ref-scoped, so a pull
request's first push is warm only for the shapes a push to `main` compiles.
These tests hold the checked-in workflows to it, and drive each reader over
synthetic documents so a refusal is proved by a case rather than by the
repository happening to comply.

Run via ``make test-workflow-contracts``.
"""

import re
import shlex
import typing as typ

import pytest
from mutation_features import resolved_cargo
from pr_concurrency_support import CANCEL_IN_PROGRESS, GROUP_EXPRESSION
from pull_request_reach import declares_trigger
from trunk_writer import (
    EQUIVALENT_WRITER_COMMANDS,
    NON_WRITING_PUSH_JOBS,
    TRUNK_WRITERS,
    UnreviewedConditionError,
    cancellation_violations,
    compiling_commands,
    push_jobs,
    runs_on,
    trunk_push_violations,
    writer_violations,
)
from ubicloud_workflow_support import (
    REPOSITORY_ROOT,
    SUITE_JOBS,
    UBICLOUD_JOBS,
    load_job,
    load_workflow,
    parse_workflow,
)


def _is_pull_request_lane_on_gha(
    job: dict[str, typ.Any], workflow: dict[str, typ.Any]
) -> bool:
    """Return whether a pull request runs this job.

    Every Ubicloud job reaches the cache proxy through `setup-rust`, so the
    question is only whether a pull request runs it.
    """
    if not declares_trigger(workflow, "pull_request"):
        return False
    return runs_on(job, "pull_request")


def _pull_request_lanes_on_gha() -> set[str]:
    """Return the Ubicloud jobs on the Actions backend a pull request runs."""
    return {
        job_name
        for job_name, workflow_name in UBICLOUD_JOBS.items()
        if _is_pull_request_lane_on_gha(load_job(job_name), load_workflow(workflow_name))
    }


def test_every_pull_request_lane_on_gha_has_a_registered_writer() -> None:
    """The presence half: a new lane cannot join without naming its writer."""
    lanes = _pull_request_lanes_on_gha()
    assert lanes == set(TRUNK_WRITERS), (
        f"pull-request lanes on the Actions backend are {sorted(lanes)}, but "
        f"TRUNK_WRITERS registers {sorted(TRUNK_WRITERS)}"
    )


@pytest.mark.parametrize(
    ("job", "triggers", "expected"),
    [
        pytest.param({}, "on: pull_request\n", True, id="a-lane"),
        pytest.param({}, "on:\n  push:\n", False, id="a-push-workflow"),
        pytest.param(
            {"if": "github.event_name != 'pull_request'"},
            "on: [push, pull_request]\n",
            False,
            id="a-job-kept-off-pull-requests",
        ),
    ],
)
def test_only_jobs_a_pull_request_runs_need_a_writer(
    job: dict[str, typ.Any], triggers: str, expected: bool
) -> None:
    """A job no pull request runs reads nothing from `main`'s scope."""
    workflow = parse_workflow(f"{triggers}jobs: {{}}\n")
    assert _is_pull_request_lane_on_gha(job, workflow) is expected, (
        f"expected {expected} for triggers {triggers!r} and job {job!r}"
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


_MAKEFILE: typ.Final = REPOSITORY_ROOT / "Makefile"


def _make_recipe(makefile: str, target: str) -> str:
    """Return a Make target's recipe, stopping at the next rule."""
    lines = makefile.splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.startswith(f"{target}:")),
        None,
    )
    assert start is not None, f"{target} must be a Makefile target"
    body: list[str] = []
    for line in lines[start + 1 :]:
        if line and not line.startswith("\t"):
            break
        body.append(line)
    return "\n".join(body)


def _makefile_variable(makefile: str, name: str) -> str:
    """Return one Makefile variable's raw definition."""
    match = re.search(rf"^{re.escape(name)} \??= (.*)$", makefile, re.MULTILINE)
    assert match is not None, f"{name} must be defined in the Makefile"
    return match.group(1)


def _recipe_commands(makefile: str, target: str) -> list[list[str]]:
    """Return each executable recipe line's words, comments excluded.

    ``$(CARGO)`` and ``$(CARGO_LOCKED)`` are resolved, so a difference in
    package or feature selection is visible in the tokens rather than hidden
    behind a variable. A continued shell line ends with a backslash, which
    ``shlex`` reads as an unterminated escape; splitting on whitespace still
    exposes the flags on that line.
    """
    cargo = resolved_cargo(_makefile_variable(makefile, "CARGO"))
    commands: list[list[str]] = []
    for line in _make_recipe(makefile, target).splitlines():
        stripped = line.strip().lstrip("@-+")
        if not stripped or stripped.startswith("#"):
            continue
        resolved = stripped.replace("$(CARGO)", cargo).replace(
            "$(CARGO_LOCKED)", _makefile_variable(makefile, "CARGO_LOCKED")
        )
        try:
            commands.append(shlex.split(resolved, comments=True))
        except ValueError:
            commands.append(resolved.split())
    return commands


@pytest.mark.parametrize(
    ("reader_command", "writer_command"), sorted(EQUIVALENT_WRITER_COMMANDS.items())
)
def test_an_equivalent_writer_command_compiles_without_running(
    reader_command: str, writer_command: str
) -> None:
    """A twin earns its place only by building the reader's shape and stopping.

    A writer recipe such as ``cargo test --no-run -p unrelated`` would satisfy
    a bare ``--no-run`` check while warming nothing the reader builds. When the
    reader is a single plain command, the writer must match it token for token
    except for that flag. A reader that runs a script of overrides, such as
    ``make coverage``, is compared on the one difference the mapping claims:
    its twin passes ``--no-run`` rather than running the tests.
    """
    makefile = _MAKEFILE.read_text(encoding="utf-8")
    reader = _recipe_commands(makefile, reader_command.removeprefix("make "))
    writer = _recipe_commands(makefile, writer_command.removeprefix("make "))
    assert any("--no-run" in words for words in writer), (
        f"{writer_command} must compile the reader's shape without running it; "
        f"got {writer!r}"
    )
    if len(reader) == 1 and reader[0][:1] == ["cargo"]:
        assert writer == [reader[0] + ["--no-run"]], (
            f"{writer_command} must compile exactly what {reader_command} "
            f"builds and stop; got {writer!r}"
        )


def test_only_the_registered_writers_run_on_the_trunk_push() -> None:
    """No second job writes `main`'s scope, and no writer is missing from it.

    Read over the workflows the suite jobs live in, so a job added to either
    one, or a lane whose event guard is dropped, shows up here.
    `rolling-release.yml` also runs on a push to `main` and writes `main`'s
    scope, but it compiles only release shapes that no pull-request lane
    compiles, so it is a writer nothing reads rather than a second writer.
    """
    gha_workflows = sorted(set(SUITE_JOBS.values()))
    running = {
        job
        for name in gha_workflows
        for job in push_jobs(load_workflow(name))
        if job not in NON_WRITING_PUSH_JOBS
    }
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
    violations = trunk_push_violations(parse_workflow(f"{triggers}jobs: {{}}\n"))
    assert violations == [], f"{triggers!r} was refused: {violations}"


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


def test_a_readers_command_inside_a_block_script_still_needs_a_writer() -> None:
    """A command beneath `set -euo pipefail` is a shape the writer must compile."""
    reader = {"steps": [{"run": "set -euo pipefail\ncargo build --workspace\n"}]}
    violations = writer_violations(reader, {"steps": []}, parse_workflow(_TRUNK))
    assert any("cargo build --workspace" in v for v in violations), violations


def test_a_writer_running_the_readers_commands_is_accepted() -> None:
    """The narrow half: the same job as its own writer passes."""
    violations = writer_violations(_READER, _READER, parse_workflow(_TRUNK))
    assert violations == [], f"the reader as its own writer was refused: {violations}"


def test_only_make_and_cargo_steps_count_as_compiling() -> None:
    """Checks, installs, uploads and mere mentions are not shapes to repeat."""
    job = {
        "steps": [
            {"run": "make lint"},
            {"run": "cargo build --workspace"},
            {"run": "bash scripts/record-sccache-effectiveness.sh"},
            {"run": "echo make lint is next"},
            {"run": "set -euo pipefail\n  make publish-check  \necho cargo\n"},
            {"uses": "actions/checkout@abc"},
        ]
    }
    commands = compiling_commands(job)
    expected = {"make lint", "cargo build --workspace", "make publish-check"}
    assert commands == expected, f"compiling commands were {sorted(commands)}"


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
    assert runs_on(job, event) is expected, (
        f"{condition!r} should {'admit' if expected else 'refuse'} {event}"
    )


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
    jobs = push_jobs(document)
    assert jobs == ["windows", "writer"], f"push jobs were {jobs}"


@pytest.mark.parametrize("writer", sorted(set(TRUNK_WRITERS.values())))
def test_nothing_can_cancel_a_writer_on_the_trunk_push(writer: str) -> None:
    """A cancelled trunk writer leaves `main`'s scope cold, and says nothing."""
    violations = cancellation_violations(
        load_job(writer), load_workflow(UBICLOUD_JOBS[writer])
    )
    assert not violations, f"{writer}: {violations}"


def _concurrency(group: str, cancel: object) -> dict[str, object]:
    """Return a `concurrency` mapping with the supplied group and setting."""
    return {"group": group, "cancel-in-progress": cancel}


@pytest.mark.parametrize(
    ("workflow", "job", "expected"),
    [
        pytest.param(
            {"concurrency": _concurrency(GROUP_EXPRESSION, True)},
            {},
            "cancel-in-progress True",
            id="a-literal-true",
        ),
        pytest.param(
            {"concurrency": _concurrency(GROUP_EXPRESSION, "${{ always() }}")},
            {},
            "can cancel the push run",
            id="an-unreviewed-expression",
        ),
        pytest.param(
            {"concurrency": _concurrency("${{ github.workflow }}", CANCEL_IN_PROGRESS)},
            {},
            "beside a pull request's",
            id="a-group-a-pull-request-shares",
        ),
        pytest.param(
            {},
            {"concurrency": _concurrency("writer", True)},
            "the job cancel-in-progress",
            id="a-job-level-cancel",
        ),
        pytest.param(
            {"concurrency": ["ci"]},
            {},
            "not a readable shape",
            id="an-unreadable-shape",
        ),
    ],
)
def test_a_cancellable_writer_is_refused(
    workflow: dict[str, typ.Any], job: dict[str, typ.Any], expected: str
) -> None:
    """Each way the push run could be cancelled is caught."""
    violations = cancellation_violations(job, workflow)
    assert any(expected in violation for violation in violations), violations


@pytest.mark.parametrize(
    "concurrency",
    [
        pytest.param(None, id="none"),
        pytest.param("ci-main", id="a-named-group"),
        pytest.param(_concurrency("ci", False), id="cancellation-off"),
        pytest.param(
            _concurrency(GROUP_EXPRESSION, CANCEL_IN_PROGRESS), id="the-reviewed-pair"
        ),
    ],
)
def test_a_writer_the_push_cannot_cancel_is_accepted(concurrency: object) -> None:
    """The narrow half: every shape that never cancels the push passes."""
    workflow = {} if concurrency is None else {"concurrency": concurrency}
    violations = cancellation_violations({}, workflow)
    assert violations == [], f"{concurrency!r} was refused: {violations}"
