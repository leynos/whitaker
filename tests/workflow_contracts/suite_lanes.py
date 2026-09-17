"""Reads every suite-running lane out of the workflow files.

Separated from the contract so the workflow reading and the assertions
stay legible apart, and so neither module outgrows the 400-line limit
``AGENTS.md`` sets.

Reading the commands out of a step's script is ``suite_commands``,
which this module calls. The split keeps the file reading here and the
text judging there, so the judging can be driven with scripts this
repository does not contain.
"""

from __future__ import annotations

import pathlib
import typing as typ

import yaml
from suite_commands import _disguised_suite_lines, _suite_commands
from ubicloud_workflow_support import WORKFLOWS_DIRECTORY

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


def _mapping(value: object) -> dict[str, typ.Any] | None:
    """Return a parsed value when it is a mapping, and None when it is not."""
    # One guard rather than an `isinstance` at each use, so a malformed
    # job, step or environment is skipped in the same way wherever it is
    # read, and the skipping is named where it happens.
    match value:
        case dict() as mapping:
            return mapping
        case _:
            return None


class WorkflowLoadError(OSError):
    """Raised when a workflow file cannot be read or parsed.

    The one failure this module's boundary can have, named so a caller
    can tell it from a lane that is unbounded. An unreadable file and a
    malformed one both mean the contract saw fewer lanes than the
    repository declares, which would otherwise read as a repository
    with fewer lanes.
    """


def _workflow_documents(
    directory: pathlib.Path | None = None,
) -> dict[str, dict[str, typ.Any]]:
    """Return every workflow document, keyed by file name.

    The one place this module touches the filesystem. Everything below
    is a query over documents, so a caller can drive the lane discovery
    with documents this repository does not contain, which is the only
    way to separate a correct reading from one that happens to agree
    with the tree.

    Both extensions are read. A lane in the other one would otherwise
    escape every assertion below without failing anything.

    Parameters
    ----------
    directory : pathlib.Path or None
        Where to read from. The repository's own workflow directory
        when none is given.

    Returns
    -------
    dict[str, dict[str, typ.Any]]
        File name to parsed document.

    Raises
    ------
    WorkflowLoadError
        If a file cannot be read or is not valid YAML. Raised rather
        than skipped: a workflow the contract cannot read is one whose
        lanes it cannot judge, and skipping it would report a
        repository with fewer lanes than it has.
    """
    documents: dict[str, dict[str, typ.Any]] = {}
    for pattern in ("*.yml", "*.yaml"):
        for path in sorted((directory or WORKFLOWS_DIRECTORY).glob(pattern)):
            match _parsed_workflow(path):
                case dict() as parsed:
                    documents[path.name] = parsed
                case _:
                    continue
    return documents


def _parsed_workflow(path: pathlib.Path) -> object:
    """Return one workflow file's parsed content, or fail with its name."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        message = f"cannot read the workflow {path.name}: {error}"
        raise WorkflowLoadError(message) from error
    try:
        return yaml.safe_load(text)
    except yaml.YAMLError as error:
        message = f"the workflow {path.name} is not valid YAML: {error}"
        raise WorkflowLoadError(message) from error


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


def _declared_jobs(
    documents: dict[str, dict[str, typ.Any]] | None = None,
) -> tuple[_Job, ...]:
    """Return every job in every workflow, with its file.

    Flattening the two levels here is what keeps the callers below to
    one loop each: a job's identity travels with it rather than being
    reconstructed from an enclosing scope.

    Parameters
    ----------
    documents : dict[str, dict[str, typ.Any]] or None
        Parsed workflow documents keyed by file name. The repository's
        own are read when none are given, so the file reading stays at
        the boundary and the flattening below is a pure query that can
        be driven with documents the tree does not contain.

    Returns
    -------
    tuple[_Job, ...]
        Every declared job.
    """
    found = _workflow_documents() if documents is None else documents
    return tuple(
        _Job(workflow=name, name=str(job_name), body=body)
        for name, document in found.items()
        # `jobs:` can hold anything the YAML parser accepts. A list
        # there would raise on `.items()`, reporting a parse failure
        # where the question was whether a lane is bounded.
        for job_name, job in (_mapping(document.get("jobs")) or {}).items()
        if (body := _mapping(job)) is not None
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
    steps = (
        body
        for step in job.body.get("steps") or []
        if (body := _mapping(step)) is not None
    )
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
        for command in _suite_commands(str(step.get("run", "")))
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
    for entry in job.body.get("steps") or []:
        step = _mapping(entry)
        if step is None:
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
    match owner.get("env"):
        case dict() as environment:
            return WATCHDOG_VARIABLE in environment
        case _:
            return False


#: The variable that selects a profile, which this repository passes on
#: the command line and the lane reader therefore reads from there.
PROFILE_VARIABLE: typ.Final[str] = "NEXTEST_PROFILE"


def _step_scopes(
    where: str, job: dict[str, typ.Any]
) -> typ.Iterator[tuple[str, dict[str, typ.Any]]]:
    """Yield each step of one job, labelled by its position."""
    for index, raw_step in enumerate(job.get("steps") or []):
        step = _mapping(raw_step)
        if step is not None:
            yield f"{where}: step {index}", step


def _job_scopes(
    name: str, document: dict[str, typ.Any]
) -> typ.Iterator[tuple[str, dict[str, typ.Any]]]:
    """Yield each job of one workflow and each of its steps."""
    for job_id, raw_job in (_mapping(document.get("jobs")) or {}).items():
        job = _mapping(raw_job)
        if job is None:
            continue
        where = f"{name}:{job_id}"
        yield f"{where}: job level", job
        yield from _step_scopes(where, job)


def _env_scopes(
    documents: dict[str, dict[str, typ.Any]] | None = None,
) -> typ.Iterator[tuple[str, dict[str, typ.Any]]]:
    """Yield every scope that may carry an `env` block, with a label.

    All three, in the order GitHub resolves them, because a caller
    asking whether a variable is declared anywhere must look at each.

    Parameters
    ----------
    documents : dict[str, dict[str, typ.Any]] or None
        Parsed workflow documents keyed by file name. The repository's
        own are read when none are given, so the file reading stays at
        the boundary and the walk below is a pure query that can be
        driven with documents the tree does not contain.

    Yields
    ------
    tuple[str, dict[str, typ.Any]]
        A label naming the scope, and the scope's mapping.
    """
    found = _workflow_documents() if documents is None else documents
    for name, document in found.items():
        yield f"{name}: workflow level", document
        yield from _job_scopes(name, document)


def _scopes_declaring_profile(
    documents: dict[str, dict[str, typ.Any]] | None = None,
) -> list[str]:
    """Return every scope whose `env` names the profile variable.

    No value is read and none is needed. A declaration masks the outer
    scopes whatever it holds, and it has two blank spellings that parse
    differently: ``NEXTEST_PROFILE: ""`` is the empty string and a
    valueless ``NEXTEST_PROFILE:`` is None. Both select the default
    profile as surely as a named value selects a profile, and all three
    are a lane the command-line reader did not see. Deciding by key
    membership is what covers the valueless spelling; a truth test or an
    ``is not None`` test would walk past it.

    Parameters
    ----------
    documents : dict[str, dict[str, typ.Any]] or None
        Parsed workflow documents keyed by file name. The repository's
        own are read when none are given.

    Returns
    -------
    list[str]
        A label for each scope that declares the variable, in the order
        GitHub resolves them.
    """
    return [
        label
        for label, scope in _env_scopes(documents)
        if PROFILE_VARIABLE in (_mapping(scope.get("env")) or {})
    ]
