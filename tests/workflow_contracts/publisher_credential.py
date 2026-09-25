"""Where the publisher's CodeScene credential may appear, and where it may not.

The upload is `upload-codescene-coverage`, a composite action, and a composite
action's nested steps inherit the calling step's `env`. A token bound there, or
on the job, or on the workflow, therefore reaches every step inside the action.
So the credential is in no `env` at all. A check step publishes only whether it
exists, with one exact command whose expression GitHub evaluates before the
shell starts, and the upload takes it directly as its `access-token` input.

The positive half matters as much as the prohibition. Deleting the token
satisfies "no `env` names it" while the upload's guard goes false and the
upload skips on every run, so the check's command and the upload's input are
asserted exactly, and every other mention of the credential is refused.

Run via ``make test-workflow-contracts``.
"""

import collections.abc as cabc
import re
import typing as typ

#: The secret's name. Expression contexts are case-insensitive, so every
#: search for it folds case.
CREDENTIAL_KEY: typ.Final[str] = "CS_ACCESS_TOKEN"

#: The id of the step that reports whether the credential exists.
CHECK_ID: typ.Final[str] = "codescene_token"

#: The check step's one command. GitHub evaluates the expression before it
#: sends the command to the runner, so the check's shell receives only a
#: literal `true` or `false`.
CHECK_COMMAND: typ.Final[str] = (
    'echo "available=${{ secrets.CS_ACCESS_TOKEN != \'\' }}" >> "$GITHUB_OUTPUT"'
)

#: The action input that receives the credential, and the expression that
#: fills it, without its `${{ }}` wrapper.
TOKEN_INPUT: typ.Final[str] = "access-token"
SECRET_REFERENCE: typ.Final[str] = f"secrets.{CREDENTIAL_KEY}"

#: A whole-value expression, with the whitespace GitHub allows inside it.
_EXPRESSION: typ.Final[re.Pattern[str]] = re.compile(
    r"^\$\{\{\s*(?P<body>.*?)\s*\}\}$", re.DOTALL
)


def _expression_body(value: object) -> str | None:
    """Return a whole-value `${{ }}` expression's body, or `None`."""
    if not isinstance(value, str):
        return None
    match = _EXPRESSION.match(value.strip())
    return match.group("body") if match else None


def _mapping(value: object) -> dict[str, typ.Any]:
    """Return a value when it is a mapping, and an empty one otherwise."""
    return value if isinstance(value, dict) else {}


def _strings(value: object) -> cabc.Iterator[str]:
    """Yield every string in a parsed YAML value, mapping keys included.

    Parameters
    ----------
    value : object
        A parsed workflow fragment.

    Yields
    ------
    str
        Each string in document order, a mapping's key before its value.

    >>> list(_strings({"env": {"TOKEN": "x"}, "steps": ["a", 1]}))
    ['env', 'TOKEN', 'x', 'steps', 'a']
    """
    match value:
        case str():
            yield value
        case dict():
            for key, item in value.items():
                yield from _strings(key)
                yield from _strings(item)
        case list():
            for item in value:
                yield from _strings(item)
        case _:
            return


def _names_credential(value: object) -> bool:
    """Return whether any string in a value names the credential."""
    folded = CREDENTIAL_KEY.casefold()
    return any(folded in text.casefold() for text in _strings(value))


def checks_availability(step: dict[str, typ.Any]) -> bool:
    """Return whether a step is exactly the credential's availability check.

    Its id, its one command, no `if:` and no `env`. A condition would leave
    the output unset whenever it was false, so the upload would skip forever,
    and an `env` would put the credential back into an environment.

    Parameters
    ----------
    step : dict[str, typ.Any]
        One parsed workflow step.

    Returns
    -------
    bool
        True when the step is the check and nothing else.

    >>> checks_availability({"id": "codescene_token", "run": CHECK_COMMAND})
    True
    >>> checks_availability(
    ...     {"id": "codescene_token", "run": CHECK_COMMAND, "if": "always()"}
    ... )
    False
    """
    return (
        step.get("id") == CHECK_ID
        and str(step.get("run", "")).strip() == CHECK_COMMAND
        and "if" not in step
        and "env" not in step
    )


def passes_the_credential(step: dict[str, typ.Any]) -> bool:
    """Return whether an upload step takes the credential as its input.

    Parameters
    ----------
    step : dict[str, typ.Any]
        One parsed workflow step.

    Returns
    -------
    bool
        True when the step's `access-token` input is
        `${{ secrets.CS_ACCESS_TOKEN }}`.

    >>> passes_the_credential({"with": {"access-token": "${{secrets.CS_ACCESS_TOKEN}}"}})
    True
    >>> passes_the_credential({"with": {"access-token": "${{ env.CS_ACCESS_TOKEN }}"}})
    False
    """
    passed = _expression_body(_mapping(step.get("with")).get(TOKEN_INPUT))
    return passed == SECRET_REFERENCE


def credential_scopes(document: dict[str, typ.Any]) -> list[str]:
    """Return every workflow, job or step `env` that names the credential.

    Keys and values both count, case folded, so neither an `env` entry named
    after the credential nor one reading it under another name is missed.

    Parameters
    ----------
    document : dict[str, typ.Any]
        One parsed workflow document.

    Returns
    -------
    list[str]
        `workflow`, `job <name>` and `job <name> step <index>` for each scope
        whose `env` names the credential.

    >>> credential_scopes({"env": {"T": "${{ secrets.cs_access_token }}"}, "jobs": {}})
    ['workflow']
    """
    scopes = ["workflow"] if _names_credential(document.get("env")) else []
    for name, job in _mapping(document.get("jobs")).items():
        job_mapping = _mapping(job)
        if _names_credential(job_mapping.get("env")):
            scopes.append(f"job {name}")
        steps = job_mapping.get("steps")
        scopes.extend(
            f"job {name} step {index}"
            for index, step in enumerate(steps if isinstance(steps, list) else ())
            if _names_credential(_mapping(step).get("env"))
        )
    return scopes


def _located_strings(value: object, path: str) -> cabc.Iterator[tuple[str, str]]:
    """Yield every string in a parsed YAML value with the path that reached it.

    A mapping key is reported at the path of the entry it names, so an `env`
    entry named after the credential is found at that entry.

    Parameters
    ----------
    value : object
        A parsed workflow fragment.
    path : str
        The path taken to reach it.

    Yields
    ------
    tuple[str, str]
        The path and the string found there.

    >>> list(_located_strings({"env": {"T": "x"}}, "job"))
    [('job.env', 'env'), ('job.env.T', 'T'), ('job.env.T', 'x')]
    """
    match value:
        case str():
            yield path, value
        case dict():
            for key, item in value.items():
                child = f"{path}.{key}" if path else str(key)
                if isinstance(key, str):
                    yield child, key
                yield from _located_strings(item, child)
        case list():
            for index, item in enumerate(value):
                yield from _located_strings(item, f"{path}[{index}]")
        case _:
            return


def credential_sites(document: dict[str, typ.Any]) -> list[str]:
    """Return the path of every string in a workflow naming the credential.

    Locations rather than values, so a correct expression in the wrong place
    is still found, and a spelling GitHub accepts in the right place is not
    mistaken for a stray mention.

    Parameters
    ----------
    document : dict[str, typ.Any]
        One parsed workflow document.

    Returns
    -------
    list[str]
        Each path once, sorted, such as `jobs.up.steps[1].with.access-token`.

    >>> credential_sites({"jobs": {"up": {"steps": [{"run": "${{ secrets.cs_access_token }}"}]}}})
    ['jobs.up.steps[0].run']
    """
    folded = CREDENTIAL_KEY.casefold()
    return sorted({
        path
        for path, text in _located_strings(document, "")
        if folded in text.casefold()
    })
