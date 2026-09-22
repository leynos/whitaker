"""Read the commands a workflow `run:` block executes.

A contract that requires a lane to *run* a command cannot match the command as
a substring of the script: `echo make coverage` and `# make coverage` both
contain it and run nothing. So a script is split into simple commands the way
the shell would, and a command counts only when its words begin one of them.

Scope and re-use: this is a reader for requirements ("the lane runs X"). A
prohibition ("no lane mentions X") should stay a substring test, because there
over-matching is the safe direction and this reader deliberately under-matches.
It models lists (`;`, `&&`, `||`, `&`), pipelines, subshell parentheses, line
continuations, comments and leading variable assignments; it does not model
functions, `eval`, or command substitution, and a line it cannot tokenize
contributes no command rather than a guessed one.

Run via ``make test-workflow-contracts``.
"""

import itertools
import re
import shlex
import typing as typ

#: The characters shlex groups into operator tokens.
_OPERATOR_CHARACTERS: typ.Final[str] = ";&|()"

#: A leading `NAME=value` word, which sets the environment of the command
#: after it rather than being the command.
_ASSIGNMENT: typ.Final[re.Pattern[str]] = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")


def _is_operator(token: str) -> bool:
    """Return whether a token separates one simple command from the next."""
    return bool(token) and set(token) <= set(_OPERATOR_CHARACTERS)


def _line_tokens(line: str) -> list[str]:
    """Return one line's words and operators, stopping at a comment."""
    lexer = shlex.shlex(line, posix=True, punctuation_chars=_OPERATOR_CHARACTERS)
    lexer.whitespace_split = True
    # shlex would end a word at a `#` anywhere; the shell starts a comment only
    # at the start of a word, so comments are cut here instead.
    lexer.commenters = ""
    try:
        tokens = list(lexer)
    except ValueError:
        # An unbalanced quote: the line is not read as any command at all,
        # which under-matches, the safe direction for a requirement.
        return []
    cut = next(
        (index for index, token in enumerate(tokens) if token.startswith("#")),
        len(tokens),
    )
    return tokens[:cut]


def _split_on_operators(tokens: list[str]) -> list[list[str]]:
    """Return the runs of words between operator tokens."""
    return [
        list(words)
        for is_operator, words in itertools.groupby(tokens, key=_is_operator)
        if not is_operator
    ]


def command_segments(script: str) -> list[list[str]]:
    """Return each simple command in a script as its list of words.

    Parameters
    ----------
    script : str
        The body of a `run:` step.

    Returns
    -------
    list[list[str]]
        One word list per simple command, with any leading variable
        assignments removed, in script order.

    >>> command_segments("set -e; FOO=1 make coverage  # measure")
    [['set', '-e'], ['make', 'coverage']]
    """
    joined = script.replace("\\\n", " ")
    return [
        command
        for line in joined.splitlines()
        for words in _split_on_operators(_line_tokens(line))
        if (command := list(itertools.dropwhile(_ASSIGNMENT.match, words)))
    ]


def runs_command(script: str, command: str) -> bool:
    """Return whether a script executes a command, not merely mentions it.

    Parameters
    ----------
    script : str
        The body of a `run:` step.
    command : str
        The command's leading words, such as `make coverage`.

    Returns
    -------
    bool
        True when some simple command in the script begins with those words.

    >>> runs_command("make coverage", "make coverage")
    True
    >>> runs_command("echo make coverage", "make coverage")
    False
    """
    words = command.split()
    return any(segment[: len(words)] == words for segment in command_segments(script))
