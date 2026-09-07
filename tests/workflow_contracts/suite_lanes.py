"""Reads every suite-running lane out of the workflow files.

Separated from the contract so the workflow reading and the assertions
stay legible apart, and so neither module outgrows the 400-line limit
``AGENTS.md`` sets.

The matching is deliberately line-by-line. A contract that finds the
suite command anywhere inside a multiline ``run`` passes when the step
wraps it in ``if false; then ...; fi`` or appends ``|| true``, so each
line is judged as a plain invocation and a disguised one is reported
rather than counted.
"""

from __future__ import annotations

import typing as typ

import yaml
from ubicloud_workflow_support import WORKFLOWS_DIRECTORY

SUITE_COMMANDS: typ.Final[tuple[str, ...]] = ("make test", "make coverage")

#: Commands that contain a suite command as a prefix but run something
#: else entirely. `make test-doc` is doctests, outside nextest; the other
#: two are checkers that happen to be named for what they check.
NOT_SUITE_COMMANDS: typ.Final[tuple[str, ...]] = (
    "make test-doc",
    "make test-glibc-baseline",
    "make test-workflow-contracts",
    "make test-markdown-format",
)

#: Shapes that put a suite command on a line without running it as the
#: step's own command, or without letting its failure end the step. A
#: line carrying one is neither a suite invocation nor safely ignored,
#: so the contract refuses to judge it and says so.
#:
#: `if false; then make test; fi` keeps the text and runs nothing, which
#: would drop the lane from this contract silently, taking its ceiling
#: with it. `make test || true` does run the suite but discards its
#: verdict, so the lane's budgets are checked while its result is not.
DISGUISES: typ.Final[tuple[str, ...]] = (
    "|| true",
    "|| :",
    "if ",
    "&&",
    ";",
    "|",
)

#: The environment variable the shared coverage action reads for its
#: cargo watchdog. Asserted absent: this repository does not use that
#: action, and a lane that adopted it would inherit its 1,800 s default.
WATCHDOG_VARIABLE: typ.Final[str] = "RUN_RUST_CARGO_WAIT_TIMEOUT"
COVERAGE_ACTION: typ.Final[str] = "shared-actions/.github/actions/generate-coverage"


class SuiteLane(typ.NamedTuple):
    """One step that runs the suite, with the job budget enclosing it.

    Attributes
    ----------
    workflow : str
        The workflow file's name.
    job : str
        The job the step belongs to.
    step : str
        The step's declared name.
    command : str
        The whole command line it runs, not the matched constant, so the
        profile can be read from it.
    job_timeout : float or None
        The enclosing job's ``timeout-minutes`` in seconds, or None when
        the job declares none and so inherits GitHub's six-hour default.
    condition : tuple[object, object]
        The ``if`` on the step and on its job. A skipped step runs no
        suite, so every budget here says nothing about it; the condition
        is part of what identifies a lane rather than incidental to it.
    """

    workflow: str
    job: str
    step: str
    command: str
    job_timeout: float | None
    condition: tuple[object, object] = (None, None)

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``workflow:job:step`` for this lane.
        """
        return f"{self.workflow}:{self.job}:{self.step!r}"


def _suite_command(run: str) -> str | None:
    """Return the suite command a step runs, or None.

    ``make test-doc`` and the checkers named for what they check all begin
    with a suite command's text. Matching by prefix would bind them to
    budgets they do not run under, and would let a genuine suite step
    escape by being renamed.

    Parameters
    ----------
    run : str
        A step's ``run`` script.

    Returns
    -------
    str or None
        The whole command line, or None when the step runs no suite
        command. The line rather than the matched constant, because the
        profile the lane runs under is an argument on it.
    """
    candidates = (line.strip() for line in run.splitlines())
    return next((line for line in candidates if _is_suite_line(line)), None)


def _names_a_suite_command(line: str) -> bool:
    """Return whether one line mentions a suite command at all.

    Mentioning is weaker than invoking, and deliberately so: the two are
    compared below, and a line that mentions one without invoking it is
    the case this contract refuses to judge.

    Parameters
    ----------
    line : str
        One stripped line of a step's script.

    Returns
    -------
    bool
        True when a suite command's text appears on the line.
    """
    if any(line.startswith(other) for other in NOT_SUITE_COMMANDS):
        return False
    return any(command in line for command in SUITE_COMMANDS)


def _is_suite_line(line: str) -> bool:
    """Return whether one stripped line invokes the suite plainly.

    ``make test-doc`` and the checkers named for what they check all
    begin with a suite command's text, so they are excluded first and by
    exact prefix rather than by substring.

    Plainly means the line is the command and its arguments, and nothing
    else. A line that also carries a conditional, a separator or a
    status suppressor is not judged here: :func:`_disguised_suite_lines`
    reports it instead, because such a line may run the suite, may not,
    and may discard its verdict, and this contract cannot tell which.

    Parameters
    ----------
    line : str
        One stripped line of a step's script.

    Returns
    -------
    bool
        True when the line runs a suite command and nothing else.
    """
    if not _names_a_suite_command(line):
        return False
    if any(disguise in line for disguise in DISGUISES):
        return False
    return any(
        line == command or line.startswith(f"{command} ") for command in SUITE_COMMANDS
    )


def _disguised_suite_lines(run: str) -> list[str]:
    """Return lines naming a suite command without plainly running one.

    Parameters
    ----------
    run : str
        A step's ``run`` script.

    Returns
    -------
    list[str]
        The offending lines, stripped.
    """
    return [
        line
        for raw in run.splitlines()
        if (line := raw.strip())
        and _names_a_suite_command(line)
        and not _is_suite_line(line)
    ]


def _workflow_documents() -> dict[str, dict[str, typ.Any]]:
    """Return every workflow document, keyed by file name.

    Both extensions are read. A lane in the other one would otherwise
    escape every assertion below without failing anything.

    Returns
    -------
    dict[str, dict[str, typ.Any]]
        File name to parsed document.
    """
    documents: dict[str, dict[str, typ.Any]] = {}
    for pattern in ("*.yml", "*.yaml"):
        for path in sorted(WORKFLOWS_DIRECTORY.glob(pattern)):
            parsed = yaml.safe_load(path.read_text(encoding="utf-8"))
            if isinstance(parsed, dict):
                documents[path.name] = parsed
    return documents


class _Job(typ.NamedTuple):
    """One job of one workflow, with the file it came from.

    Attributes
    ----------
    workflow : str
        The workflow file's name.
    name : str
        The job's identifier.
    body : dict[str, typ.Any]
        The job's parsed mapping.
    """

    workflow: str
    name: str
    body: dict[str, typ.Any]


def _declared_jobs() -> tuple[_Job, ...]:
    """Return every job in every workflow, with its file.

    Flattening the two levels here is what keeps the callers below to
    one loop each: a job's identity travels with it rather than being
    reconstructed from an enclosing scope.

    Returns
    -------
    tuple[_Job, ...]
        Every declared job.
    """
    return tuple(
        _Job(workflow=name, name=str(job_name), body=job)
        for name, document in _workflow_documents().items()
        for job_name, job in (document.get("jobs") or {}).items()
        if isinstance(job, dict)
    )


def _job_ceiling(job: _Job) -> float | None:
    """Return a job's ``timeout-minutes`` in seconds, or None.

    Parameters
    ----------
    job : _Job
        The job to read.

    Returns
    -------
    float or None
        The ceiling in seconds, or None when the job declares none and
        so inherits GitHub's six-hour default.
    """
    raw = job.body.get("timeout-minutes")
    return None if raw is None else float(raw) * 60.0


def _lanes_in_job(job: _Job) -> tuple[SuiteLane, ...]:
    """Return the suite-running lanes one job declares.

    Parameters
    ----------
    job : _Job
        The job to read.

    Returns
    -------
    tuple[SuiteLane, ...]
        One entry per suite-running step in that job.
    """
    ceiling = _job_ceiling(job)
    steps = (step for step in job.body.get("steps") or [] if isinstance(step, dict))
    return tuple(
        SuiteLane(
            workflow=job.workflow,
            job=job.name,
            step=str(step.get("name", "")) or job.name,
            command=command,
            job_timeout=ceiling,
            condition=(step.get("if"), job.body.get("if")),
        )
        for step in steps
        if (command := _suite_command(str(step.get("run", "")))) is not None
    )


def _watchdog_offences(job: _Job) -> tuple[str, ...]:
    """Return the ways one job would reintroduce the watchdog tier.

    Both halves are looked for: the action itself, and the variable that
    configures it. The variable is read at all three scopes GitHub
    resolves, because a value at workflow or job level is inherited by
    every step and a check reading only the step's own environment would
    miss it entirely, which is the opposite of what this asserts.

    Parameters
    ----------
    job : _Job
        The job to read.

    Returns
    -------
    tuple[str, ...]
        One entry per offence, naming the job and what it did.
    """
    where = f"{job.workflow}:{job.name}"
    offences: list[str] = []
    if _sets_watchdog(_workflow_documents()[job.workflow]):
        offences.append(f"{job.workflow} sets {WATCHDOG_VARIABLE} at workflow level")
    if _sets_watchdog(job.body):
        offences.append(f"{where} sets {WATCHDOG_VARIABLE} at job level")
    for step in job.body.get("steps") or []:
        if not isinstance(step, dict):
            continue
        if COVERAGE_ACTION in str(step.get("uses", "")):
            offences.append(f"{where} uses {COVERAGE_ACTION}")
        if _sets_watchdog(step):
            offences.append(f"{where} sets {WATCHDOG_VARIABLE} on a step")
    return tuple(offences)


def _sets_watchdog(owner: dict[str, typ.Any]) -> bool:
    """Return whether one scope sets the watchdog variable.

    Parameters
    ----------
    owner : dict[str, typ.Any]
        A workflow, job or step mapping.

    Returns
    -------
    bool
        True when its ``env`` names the variable.
    """
    environment = owner.get("env")
    return isinstance(environment, dict) and WATCHDOG_VARIABLE in environment
