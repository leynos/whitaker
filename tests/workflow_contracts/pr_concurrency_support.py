"""Readers for the pull-request concurrency contract.

A superseded pull-request run costs the same minutes as the run that
replaced it. GitHub cancels one only when the workflow declares a
concurrency group and asks for it, so the fact is a property of every
workflow a pull request can start, not of any one job.

The readers here are pure: :func:`trigger_names` and
:func:`concurrency_violations` take a parsed document, so the contract
can drive them with a synthetic workflow the repository does not
contain. A rule exercised only over files that already satisfy it
passes whether or not it works.
"""

import typing as typ
from pathlib import Path

import yaml
from ubicloud_workflow_support import parse_workflow

ROOT: typ.Final = Path(__file__).resolve().parents[2]

#: The repository's workflow directory, the loaders' default.
WORKFLOWS_DIRECTORY: typ.Final = ROOT / ".github" / "workflows"

#: GitHub accepts either extension, so a sweep that scans one is a gap.
WORKFLOW_SUFFIXES: typ.Final = (".yml", ".yaml")

#: The trigger a pull request starts. ``pull_request_target`` runs with
#: the base repository's token; the workflows on it here push commits
#: and merge, and cancelling one mid-write is not a saving.
PULL_REQUEST_TRIGGER: typ.Final = "pull_request"

#: The only accepted ``cancel-in-progress`` value. A literal ``true``
#: would cancel a push to main or a scheduled run sharing the group, so
#: the contract requires the guarded expression, not a truthy setting.
CANCEL_IN_PROGRESS: typ.Final = "${{ github.event_name == 'pull_request' }}"

#: The only accepted ``group``. It must be stable across pushes to one pull
#: request and distinct between workflows and pull requests: a group keyed on
#: ``github.run_id``, ``github.sha`` or ``github.run_number`` changes on every
#: push and cancels nothing, and a static one lets unrelated pull requests
#: cancel each other. The ``github.ref`` fallback keys a dispatch on its branch.
GROUP_EXPRESSION: typ.Final = (
    "${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}"
)


class WorkflowShapeError(AssertionError):
    """A workflow document is not the shape these readers can read.

    Derives from :class:`AssertionError` so a malformed workflow reads
    as a failed expectation rather than an unexpected crash, and names
    the workflow instead of a Python attribute. Each subclass carries
    its own message, so a caller cannot weaken one by passing a vaguer
    string at the raise site.
    """


class MissingTriggersError(WorkflowShapeError):
    """A workflow declared no triggers under either key."""

    def __init__(self) -> None:
        """Name the missing key rather than the Python that read it."""
        super().__init__("a workflow must declare triggers")


class UnreadableTriggersError(WorkflowShapeError):
    """A trigger value was none of the three shapes GitHub accepts."""

    def __init__(self) -> None:
        """Name the three shapes, so the fix is in the message."""
        super().__init__("triggers must be a string, a list or a mapping")


class NotAMappingError(WorkflowShapeError):
    """A workflow document did not parse to a mapping."""

    def __init__(self, name: str) -> None:
        """Name the workflow whose document is malformed.

        Parameters
        ----------
        name : str
            The workflow file name.
        """
        super().__init__(f"{name} must parse to a mapping")


class UnreadableWorkflowError(WorkflowShapeError):
    """A workflow file could not be read from disk."""

    def __init__(self, name: str) -> None:
        """Name the workflow that could not be read.

        Parameters
        ----------
        name : str
            The workflow file name.
        """
        super().__init__(f"{name} could not be read")


class UnreadableWorkflowDirectoryError(WorkflowShapeError):
    """The workflow directory could not be listed."""

    def __init__(self, directory: Path) -> None:
        """Name the directory, so a missing tree is not read as an empty one.

        Parameters
        ----------
        directory : Path
            The directory that could not be listed.
        """
        super().__init__(f"{directory} could not be listed as a workflow directory")


class UnparsableWorkflowError(WorkflowShapeError):
    """A workflow document is not parsable YAML."""

    def __init__(self, name: str) -> None:
        """Name the workflow that failed to parse.

        Parameters
        ----------
        name : str
            The workflow file name.
        """
        super().__init__(f"{name} is not parsable YAML")


def trigger_names(document: dict[str, object]) -> frozenset[str]:
    """Return the event names a workflow document declares.

    YAML 1.1 reads an unquoted ``on:`` key as the boolean ``True``, so a
    reader that looks only under the string key finds no triggers at all
    and silently reports a workflow as startable by nothing.

    An unreadable trigger value raises :class:`WorkflowShapeError` from
    :func:`_event_names`.

    Parameters
    ----------
    document : dict[str, object]
        A parsed workflow document.

    Returns
    -------
    frozenset[str]
        The declared event names.

    Raises
    ------
    MissingTriggersError
        If the document declares no triggers under either key.
    """
    for key in ("on", True):
        if key in document:
            return _event_names(document[key])
    raise MissingTriggersError


def _event_names(triggers: object) -> frozenset[str]:
    """Normalize the three shapes GitHub accepts under ``on:``."""
    match triggers:
        case str():
            return frozenset({triggers})
        case dict() | list():
            return frozenset(str(name) for name in triggers)
        case _:
            raise UnreadableTriggersError


def is_pull_request_startable(document: dict[str, object]) -> bool:
    """Report whether a pull request can start this workflow.

    Parameters
    ----------
    document : dict[str, object]
        A parsed workflow document.

    Returns
    -------
    bool
        True when the document declares the ``pull_request`` trigger.
    """
    return PULL_REQUEST_TRIGGER in trigger_names(document)


def concurrency_violations(document: dict[str, object]) -> list[str]:
    """Return every way a document fails the concurrency contract.

    Parameters
    ----------
    document : dict[str, object]
        A parsed workflow document.

    Returns
    -------
    list[str]
        One message per violation; empty when the document conforms.
    """
    match document.get("concurrency"):
        case None:
            return ["declares no concurrency: block"]
        case dict() as concurrency:
            return _group_violations(concurrency.get("group")) + _cancel_violations(
                concurrency.get("cancel-in-progress")
            )
        case _:
            return ["declares a concurrency: that is not a mapping"]


def _group_violations(group: object) -> list[str]:
    """Return the violations of the concurrency group itself."""
    if not isinstance(group, str) or not group.strip():
        return ["declares no concurrency group"]
    if " ".join(group.split()) != GROUP_EXPRESSION:
        return [f"keys its concurrency group on {group!r} and not {GROUP_EXPRESSION}"]
    return []


def _cancel_violations(cancel: object) -> list[str]:
    """Return the violations of the ``cancel-in-progress`` setting."""
    if cancel is None:
        return ["sets no cancel-in-progress"]
    if not isinstance(cancel, str) or " ".join(cancel.split()) != CANCEL_IN_PROGRESS:
        return [f"sets cancel-in-progress to {cancel!r} and not {CANCEL_IN_PROGRESS}"]
    return []


def workflow_documents(
    directory: Path = WORKFLOWS_DIRECTORY,
) -> dict[str, dict[str, object]]:
    """Parse every workflow in a directory.

    The directory is a parameter so the contract can drive the loader over a
    synthetic tree; the repository's own is the default.

    Parameters
    ----------
    directory : Path
        The directory holding the workflow files.

    Returns
    -------
    dict[str, dict[str, object]]
        Each workflow document, keyed by file name.

    Raises
    ------
    WorkflowShapeError
        A subclass naming the directory when it cannot be listed, or the
        workflow when one cannot be read, is not parsable YAML, repeats a
        key, or does not parse to a mapping.
    """
    return {path.name: _parse(path) for path in _workflow_paths(directory)}


def _workflow_paths(directory: Path) -> list[Path]:
    """List a directory's workflow files, refusing to read a failure as empty."""
    # `Path.glob` suppresses the `OSError` a missing or unlistable directory
    # raises and yields nothing, which a sweep would read as "no workflows".
    try:
        entries = list(directory.iterdir())
    except OSError as error:
        raise UnreadableWorkflowDirectoryError(directory) from error
    return sorted(path for path in entries if path.suffix.lower() in WORKFLOW_SUFFIXES)


def _parse(path: Path) -> dict[str, object]:
    """Read and parse one workflow file, naming it in any failure."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise UnreadableWorkflowError(path.name) from error
    try:
        document = parse_workflow(text)
    except yaml.YAMLError as error:
        raise UnparsableWorkflowError(path.name) from error
    if not isinstance(document, dict):
        raise NotAMappingError(path.name)
    return document


def pull_request_workflows(
    directory: Path = WORKFLOWS_DIRECTORY,
) -> dict[str, dict[str, object]]:
    """Load the workflows a pull request can start.

    Parameters
    ----------
    directory : Path
        The directory holding the workflow files.

    Returns
    -------
    dict[str, dict[str, object]]
        Each pull-request-startable workflow, keyed by file name.
    """
    return {
        name: document
        for name, document in workflow_documents(directory).items()
        if is_pull_request_startable(document)
    }
