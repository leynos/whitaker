"""Which job compiles each pull-request lane's shapes in `main`'s cache scope.

Ubicloud's cache proxy is ref-scoped, as GitHub's own Actions cache is. A run
reads its own ref's scope and the default branch's, and writes only its own.
So a pull request's first push is warm only for what a push to `main` has
compiled, and a lane whose workflow never runs on that push starts every new
pull request cold. Whitaker measured it: after the Linux lanes moved to the
proxy, a fresh branch's `linux-full` read 0 of 1,077 compilations and took
29 minutes against 11 to 14 warm (run 35825720438), because `ci.yml` ran only
on `pull_request` and `workflow_dispatch`.

The rule this module reads is therefore "every pull-request lane on the
Actions backend has exactly one trunk writer": a job that runs on a push to
`main` and nothing broader, runs unconditionally there, selects the same
backend, and runs every `make` or `cargo` command the lane runs, so the two
compile the same shapes. The other half is that nothing else in those
workflows runs on that push, so no second job writes the same scope. And
nothing may cancel the writer: a cancelled trunk run leaves `main`'s scope
cold with nothing in the run saying so, so the only cancellation accepted is
the pull-request-only one, in a group no pull request shares with the push.

A pull request cannot write `main`'s scope at all, which is the structural
half of "pull-request lanes only read": its writes land in its own ref's
scope. The archive half, that saves are confined to the trunk ref, is
`ubicloud_cache_contract_test`'s.

Job conditions are read against a table of reviewed spellings rather than
evaluated. A condition outside the table is refused, because a reader that
guessed would guess "runs", and a guess in either direction hides a writer.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

from pr_concurrency_support import CANCEL_IN_PROGRESS, GROUP_EXPRESSION
from pull_request_reach import declares_trigger

#: The one branch filter a trunk writer's push trigger may carry.
TRUNK_BRANCHES: typ.Final[tuple[str, ...]] = ("main",)

#: Every job-level condition this contract can read. Each maps to a pair: a
#: flag that is true when the condition admits only the listed events and
#: false when it admits every event except them, and the listed events.
REVIEWED_CONDITIONS: typ.Final[dict[str, tuple[bool, frozenset[str]]]] = {
    "github.event_name == 'pull_request'": (True, frozenset({"pull_request"})),
    "github.event_name != 'push'": (False, frozenset({"push"})),
}

#: A step whose script starts one of these compiles, or may compile, Rust.
COMPILING_COMMANDS: typ.Final[re.Pattern[str]] = re.compile(r"^(make|cargo)\s")

#: The `${{ ... }}` wrapper a condition may carry or omit.
_WRAPPER: typ.Final[re.Pattern[str]] = re.compile(
    r"^\$\{\{(?P<body>.*)\}\}$", re.DOTALL
)


class UnreviewedConditionError(AssertionError):
    """A job condition outside `REVIEWED_CONDITIONS`."""

    def __init__(self, condition: str) -> None:
        """Name the condition the contract cannot read.

        Parameters
        ----------
        condition : str
            The job's `if:` value, normalized.
        """
        super().__init__(
            f"job condition {condition!r} is not in REVIEWED_CONDITIONS; add it "
            "with the events it admits rather than letting the contract guess"
        )


def _normalized_condition(condition: object) -> str:
    """Return a condition without its wrapper and with whitespace collapsed."""
    text = str(condition).strip()
    if match := _WRAPPER.match(text):
        text = match.group("body")
    return " ".join(text.split())


def runs_on(job: dict[str, typ.Any], event: str) -> bool:
    """Return whether a job runs when its workflow is started by ``event``.

    Parameters
    ----------
    job : dict[str, typ.Any]
        One parsed job.
    event : str
        An event name, such as ``push``.

    Returns
    -------
    bool
        True when the job has no condition, or a reviewed one admitting the
        event.

    Raises
    ------
    UnreviewedConditionError
        When the job's condition is not in `REVIEWED_CONDITIONS`.

    >>> runs_on({"if": "${{ github.event_name != 'push' }}"}, "push")
    False
    >>> runs_on({}, "push")
    True
    """
    if "if" not in job:
        return True
    condition = _normalized_condition(job["if"])
    if condition not in REVIEWED_CONDITIONS:
        raise UnreviewedConditionError(condition)
    admits_only_listed, listed = REVIEWED_CONDITIONS[condition]
    return (event in listed) is admits_only_listed


def _push_filters(document: dict[str, typ.Any]) -> object:
    """Return the value under the push trigger, or `None` when it has none."""
    declared = document.get("on", document.get(True))
    return declared.get("push") if isinstance(declared, dict) else None


def trunk_push_violations(document: dict[str, typ.Any]) -> list[str]:
    """Return why a workflow's push trigger is not exactly "a push to main".

    A push on any other branch writes that branch's scope, which no pull
    request reads, and a `'**'` filter makes every branch a second writer. A
    `paths` filter would skip the writer on a merge that changes the shapes
    it must refresh, so any filter other than `branches` is refused too.

    Parameters
    ----------
    document : dict[str, typ.Any]
        One parsed workflow.

    Returns
    -------
    list[str]
        Empty when the workflow declares `push` with `branches: [main]` and
        nothing else.

    >>> trunk_push_violations({True: {"push": {"branches": ["main"]}}})
    []
    >>> trunk_push_violations({True: "push"})
    ['the push trigger must declare branches: [main], not every branch']
    """
    if not declares_trigger(document, "push"):
        return ["the workflow does not run on a push to main"]
    filters = _push_filters(document)
    if not isinstance(filters, dict):
        return ["the push trigger must declare branches: [main], not every branch"]
    violations: list[str] = []
    branches = filters.get("branches")
    branches = [branches] if isinstance(branches, str) else branches
    if not isinstance(branches, list) or tuple(branches) != TRUNK_BRANCHES:
        violations.append(
            f"the push trigger's branches must be {list(TRUNK_BRANCHES)}, not {branches!r}"
        )
    violations.extend(
        f"the push trigger must not filter on {key}"
        for key in sorted(filters)
        if key != "branches"
    )
    return violations


def compiling_commands(job: dict[str, typ.Any]) -> frozenset[str]:
    """Return the scripts of a job's steps that start with `make` or `cargo`.

    Over-inclusive on purpose: a `make` target that compiles nothing costs the
    writer seconds, while a missed one leaves a shape unwritten.

    >>> sorted(compiling_commands({"steps": [{"run": "make lint"}, {"uses": "x"}]}))
    ['make lint']
    """
    steps = job.get("steps") or []
    return frozenset(
        script
        for step in steps
        if COMPILING_COMMANDS.match(script := str(step.get("run", "")).strip())
    )


def writer_violations(
    reader: dict[str, typ.Any],
    writer: dict[str, typ.Any],
    writer_workflow: dict[str, typ.Any],
) -> list[str]:
    """Return why ``writer`` does not write ``reader``'s shapes in `main`'s scope.

    Parameters
    ----------
    reader : dict[str, typ.Any]
        The pull-request lane.
    writer : dict[str, typ.Any]
        The job registered as its trunk writer; it may be the same job.
    writer_workflow : dict[str, typ.Any]
        The workflow declaring the writer.

    Returns
    -------
    list[str]
        Empty when the writer runs on every push to `main`, unconditionally,
        and runs every compiling command the reader runs.
    """
    violations = trunk_push_violations(writer_workflow)
    if "if" in writer:
        violations.append(
            "the writer must run unconditionally on the push, not under "
            f"{_normalized_condition(writer['if'])!r}"
        )
    missing = sorted(compiling_commands(reader) - compiling_commands(writer))
    if missing:
        violations.append(f"the writer does not run the reader's {missing}")
    return violations


def push_jobs(document: dict[str, typ.Any]) -> list[str]:
    """Return the jobs of a workflow that run on its push trigger.

    >>> push_jobs({True: {"push": {"branches": ["main"]}}, "jobs": {
    ...     "a": {}, "b": {"if": "github.event_name == 'pull_request'"}}})
    ['a']
    """
    if not declares_trigger(document, "push"):
        return []
    jobs = document.get("jobs") or {}
    return sorted(name for name, job in jobs.items() if runs_on(job, "push"))


def _concurrency_violations(scope: str, concurrency: object) -> list[str]:
    """Return how one `concurrency` value could cancel a run on the push.

    A string, or no value, names a group without cancelling. A mapping may
    cancel only through `CANCEL_IN_PROGRESS`, which is false on a push, and
    then only in `GROUP_EXPRESSION`, which keys a pull request's run on its
    number and the push on `refs/heads/main`. A pull request's run starting
    with cancellation on cancels the in-progress runs of its own group, so a
    group the push could share would let a pull request cancel the writer.
    """
    if concurrency is None or isinstance(concurrency, str):
        return []
    if not isinstance(concurrency, dict):
        return [f"the {scope} concurrency {concurrency!r} is not a readable shape"]
    cancel = concurrency.get("cancel-in-progress", False)
    if cancel is False:
        return []
    if cancel != CANCEL_IN_PROGRESS:
        return [
            (
                f"the {scope} cancel-in-progress {cancel!r} can cancel the push "
                f"run; only {CANCEL_IN_PROGRESS!r} is reviewed"
            )
        ]
    group = concurrency.get("group")
    if group != GROUP_EXPRESSION:
        return [
            (
                f"the {scope} group {group!r} could put the push run beside a "
                f"pull request's; only {GROUP_EXPRESSION!r} is reviewed"
            )
        ]
    return []


def cancellation_violations(
    writer: dict[str, typ.Any], writer_workflow: dict[str, typ.Any]
) -> list[str]:
    """Return how the trunk writer's run on the push could be cancelled.

    Parameters
    ----------
    writer : dict[str, typ.Any]
        The trunk writer job.
    writer_workflow : dict[str, typ.Any]
        The workflow declaring it.

    Returns
    -------
    list[str]
        Empty when neither the workflow's nor the job's `concurrency` can
        cancel a run on the push.

    >>> cancellation_violations({}, {"concurrency": {
    ...     "group": "ci", "cancel-in-progress": True}})[0][:40]
    'the workflow cancel-in-progress True can'
    """
    return [
        *_concurrency_violations("workflow", writer_workflow.get("concurrency")),
        *_concurrency_violations("job", writer.get("concurrency")),
    ]
