"""Reading the suite commands out of one step's ``run`` script.

Split from ``suite_lanes``, which discovers the lanes those commands
form, so neither module outgrows the 400-line limit ``AGENTS.md`` sets
and so this half stays a pure reading: nothing here touches the
filesystem or the repository's own workflows, and every function takes
the text it judges.

The matching is line-by-line over logical lines rather than physical
ones. A contract that finds the suite command anywhere inside a
multiline ``run`` passes when the step wraps it in
``if false; then ...; fi`` or appends ``|| true``, so each line is
judged as a plain invocation and a disguised one is reported rather
than counted. Backslash continuations are folded first, because a
command split across two physical lines is still one command and its
arguments decide which budgets the lane runs under.
"""

from __future__ import annotations

import typing as typ

SUITE_COMMANDS: typ.Final[tuple[str, ...]] = ("make test", "make coverage")

#: Commands that contain a suite command as a prefix but run something
#: else entirely. `make test-doc` is doctests, outside nextest; the others
#: are checkers that happen to be named for what they check.
#: `make test-sccache-health` runs the sccache health checker's own pytest
#: suite on the gha lanes, not nextest.
NOT_SUITE_COMMANDS: typ.Final[tuple[str, ...]] = (
    "make test-doc",
    "make test-glibc-baseline",
    "make test-workflow-contracts",
    "make test-markdown-format",
    "make test-sccache-health",
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

def _logical_lines(run: str) -> list[str]:
    """Return a script's lines, with backslash continuations folded in."""
    # A continued command is one command. `make test \` followed by
    # `NEXTEST_PROFILE=ci` reads line-by-line as a suite invocation with
    # no profile, so the lane would be checked against the wrong
    # ceilings. A doubled backslash ends a line with a literal one and
    # continues nothing.
    folded: list[str] = []
    pending = ""
    for raw in run.splitlines():
        line = raw.strip()
        if line.endswith("\\") and not line.endswith("\\\\"):
            pending = f"{pending}{line[:-1].strip()} "
            continue
        folded.append(f"{pending}{line}".strip())
        pending = ""
    if pending:
        folded.append(pending.strip())
    return folded


def _suite_commands(run: str) -> list[str]:
    """Return the whole command line of every suite command a step runs."""
    # All of them, not the first: a step invoking `make coverage` and
    # then `make test NEXTEST_PROFILE=ci` runs the suite twice under two
    # profiles, and reporting one lane would leave the second bound to
    # no ceiling check at all. The line rather than the matched
    # constant, because the profile the lane runs under is an argument
    # on it.
    return [line for line in _logical_lines(run) if _is_suite_line(line)]


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
    if _is_plain_non_suite_command(line):
        return False
    return any(command in line for command in SUITE_COMMANDS)


def _is_plain_non_suite_command(line: str) -> bool:
    """Return whether the whole line is one command that is not the suite."""
    # The exclusion holds only for a line that runs the named command
    # and nothing else. `make test-doc; make test` begins with a
    # non-suite command and goes on to run the suite, so excluding it by
    # prefix would hide the second invocation from the lane discovery
    # and from the disguise report alike, leaving it bound to no ceiling
    # and reported nowhere. A line carrying a separator or a status
    # suppressor goes to the disguise report instead.
    if any(disguise in line for disguise in DISGUISES):
        return False
    return any(
        line == other or line.startswith(f"{other} ") for other in NOT_SUITE_COMMANDS
    )


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
        for line in _logical_lines(run)
        if line and _names_a_suite_command(line) and not _is_suite_line(line)
    ]
