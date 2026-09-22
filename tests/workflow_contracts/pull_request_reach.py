"""Which workflows a pull request can cause to run.

A pull-request lane is a closure, not a trigger list. A workflow declaring only
`workflow_call` still runs on a pull request when a pull-request workflow calls
it, and `secrets: inherit` hands it every secret the caller holds. Measured on
episodic rather than argued: a `workflow_call` workflow curling the CodeScene
API with an inherited `CS_ACCESS_TOKEN`, called from a pull-request job, passed
every clause of a contract that enumerated triggers alone.

So every rule about what a pull request may touch ranges over
`pull_request_closure`: the workflows declaring a reachable trigger, and every
workflow of this repository they call, transitively.

Run via ``make test-workflow-contracts``.
"""

import collections.abc as cabc
import pathlib
import typing as typ

#: Where this repository's own reusable workflows live, relative to its root.
LOCAL_WORKFLOW_DIRECTORY: typ.Final[pathlib.PurePosixPath] = pathlib.PurePosixPath(
    ".github/workflows"
)

PULL_REQUEST_TRIGGER: typ.Final[str] = "pull_request"

#: The variant that runs in the base repository's context and therefore *can*
#: read its secrets, unlike `pull_request`. A coverage step here would be worse
#: than one in an ordinary pull-request job, not equivalent to it.
PULL_REQUEST_TARGET_TRIGGER: typ.Final[str] = "pull_request_target"

#: The trigger that resumes a run with the base repository's privileges.
SUBMISSION_TRIGGER: typ.Final[str] = "workflow_run"

REACHABLE_TRIGGERS: typ.Final[tuple[str, ...]] = (
    PULL_REQUEST_TRIGGER,
    PULL_REQUEST_TARGET_TRIGGER,
    SUBMISSION_TRIGGER,
)


def declares_trigger(document: dict[str, typ.Any], trigger: str) -> bool:
    """Return whether a parsed workflow declares the given trigger.

    Parameters
    ----------
    document : dict[str, typ.Any]
        One parsed workflow document.
    trigger : str
        The trigger name, such as `pull_request`.

    Returns
    -------
    bool
        True when the workflow declares it in any of the scalar, sequence or
        mapping forms `on:` accepts.
    """
    # PyYAML reads a bare `on:` key as the boolean True, and `on:` accepts a
    # scalar, a sequence or a mapping. All four shapes answer the same
    # question, and a reader that knew only the mapping would call a
    # `on: pull_request` workflow unreachable.
    declared = document.get("on", document.get(True))
    match declared:
        case str():
            return declared == trigger
        case list():
            return trigger in declared
        case dict():
            return trigger in declared
        case _:
            return False


def is_reachable_by_a_pull_request(document: dict[str, typ.Any]) -> bool:
    """Return whether a pull request can cause this workflow to run.

    Parameters
    ----------
    document : dict[str, typ.Any]
        One parsed workflow document.

    Returns
    -------
    bool
        True when the workflow declares `pull_request`, `pull_request_target`
        or `workflow_run`.
    """
    # All three count. `pull_request_target` and `workflow_run` resume in the
    # base repository's context, so a coverage step under either reads a
    # credential in a run a pull request's contents influenced.
    return any(declares_trigger(document, name) for name in REACHABLE_TRIGGERS)


def local_call(reference: str) -> str | None:
    """Return the workflow file a job-level `uses:` names in this repository.

    Matched by shape rather than by an enumerated prefix list: the reference
    is read as a path, which drops a leading `./` and any other `.` component,
    and asked whether it names a file directly under this repository's workflow
    directory. A matcher listing spellings has to be extended for every variant
    anyone proposes, and each omission is a workflow silently outside the lane.

    A call to another repository is not followed. Its `owner/repo/` prefix puts
    it under another directory, and its content is not in this tree for any
    rule to read.

    >>> local_call("./.github/workflows/probe.yml")
    'probe.yml'
    >>> local_call("leynos/shared-actions/.github/workflows/x.yml@abc") is None
    True
    """
    path = pathlib.PurePosixPath(reference.strip())
    return path.name if path.parent == LOCAL_WORKFLOW_DIRECTORY else None


def called_workflows(document: dict[str, typ.Any]) -> frozenset[str]:
    """Return the file names of the local reusable workflows a document calls.

    >>> sorted(called_workflows(
    ...     {"jobs": {"a": {"uses": "./.github/workflows/probe.yml"}}}
    ... ))
    ['probe.yml']
    """
    jobs = document.get("jobs")
    return frozenset(
        name
        for job in (jobs.values() if isinstance(jobs, dict) else ())
        if isinstance(job, dict) and isinstance(job.get("uses"), str)
        if (name := local_call(job["uses"])) is not None
    )


def pull_request_closure(
    documents: cabc.Mapping[str, dict[str, typ.Any]],
) -> frozenset[str]:
    """Return every workflow a pull request can cause to run.

    Parameters
    ----------
    documents : Mapping[str, dict[str, typ.Any]]
        Every workflow document in the repository, by file name.

    Returns
    -------
    frozenset[str]
        The workflows declaring a reachable trigger, and everything of this
        repository they call, transitively. A called name with no document is
        left out, because there is nothing here to hold to a rule.

    >>> sorted(pull_request_closure({
    ...     "ci.yml": {True: "pull_request",
    ...                "jobs": {"a": {"uses": "./.github/workflows/p.yml"}}},
    ...     "p.yml": {True: "workflow_call", "jobs": {}},
    ...     "q.yml": {True: "workflow_call", "jobs": {}},
    ... }))
    ['ci.yml', 'p.yml']
    """
    found: set[str] = set()
    pending = [
        name
        for name, document in documents.items()
        if is_reachable_by_a_pull_request(document)
    ]
    while pending:
        name = pending.pop()
        if name in found or name not in documents:
            continue
        found.add(name)
        pending.extend(called_workflows(documents[name]) - found)
    return frozenset(found)
