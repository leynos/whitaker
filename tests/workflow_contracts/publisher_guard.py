"""What keeps the one CodeScene writer writing only from the trunk.

`coverage-main.yml` answers `workflow_dispatch` as well as a push to `main`,
and a dispatch can name any branch. The trigger filter therefore does not
confine the upload; the step's own condition does. That condition is read as a
conjunction: it is split on `&&`, and one conjunct must be the `main` ref test.
A substring check would accept
`... && github.ref == 'refs/heads/main' || github.event_name == 'workflow_dispatch'`,
which makes every conjunct optional, so an unquoted `||` anywhere is refused
rather than interpreted.

The publisher's runs also queue rather than cancel. A cancelled publisher
abandons both its upload and the cache state it writes; a queued one publishes
later, and the later push's state wins.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

#: The conjunct that confines a step to the trunk, in its canonical spelling.
MAIN_REF_CONJUNCT: typ.Final[str] = "github.ref == 'refs/heads/main'"

#: A single-quoted expression string, which may itself contain `||` or `&&`.
_QUOTED: typ.Final[re.Pattern[str]] = re.compile(r"'(?:[^']|'')*'")

#: The `${{ ... }}` wrapper an `if:` may carry or omit.
_WRAPPER: typ.Final[re.Pattern[str]] = re.compile(
    r"^\$\{\{(?P<body>.*)\}\}$", re.DOTALL
)


def _normalized(expression: str) -> str:
    """Return an expression with its whitespace runs collapsed to one space."""
    return " ".join(expression.split())


def guard_conjuncts(condition: str) -> list[str] | None:
    """Return a condition's `&&` conjuncts, or `None` when it has a `||`.

    Only operators outside quoted strings count, so a literal `'||'` compared
    against does not make a condition a disjunction.

    Parameters
    ----------
    condition : str
        A step's `if:` value, with or without its `${{ }}` wrapper.

    Returns
    -------
    list[str] | None
        Each conjunct with its whitespace collapsed, or `None` when the
        condition is a disjunction, so no caller can read one as a conjunction.

    >>> guard_conjuncts("${{ env.A != '' && github.ref == 'refs/heads/main' }}")
    ["env.A != ''", "github.ref == 'refs/heads/main'"]
    >>> guard_conjuncts("env.A != '' || github.ref == 'refs/heads/main'") is None
    True
    """
    stripped = condition.strip()
    wrapped = _WRAPPER.match(stripped)
    body = wrapped.group("body") if wrapped else stripped
    # Quoted strings are blanked before the operators are looked for, then the
    # split is made on the original text at the same offsets.
    blanked = _QUOTED.sub(lambda match: "_" * len(match.group()), body)
    if "||" in blanked:
        return None
    conjuncts: list[str] = []
    start = 0
    for operator in re.finditer(r"&&", blanked):
        conjuncts.append(_normalized(body[start : operator.start()]))
        start = operator.end()
    conjuncts.append(_normalized(body[start:]))
    return conjuncts


def is_confined_to_main(condition: object) -> bool:
    """Return whether a step condition runs the step only on the trunk.

    Parameters
    ----------
    condition : object
        A step's parsed `if:` value, which may be absent or not a string.

    Returns
    -------
    bool
        True when the condition is a conjunction with the `main` ref test as
        one of its conjuncts.

    >>> is_confined_to_main("github.ref  ==  'refs/heads/main'")
    True
    >>> is_confined_to_main(None)
    False
    """
    if not isinstance(condition, str):
        return False
    conjuncts = guard_conjuncts(condition)
    return conjuncts is not None and MAIN_REF_CONJUNCT in conjuncts


def _cancels(concurrency: object) -> bool:
    """Return whether a `concurrency` value may cancel a run in progress."""
    if not isinstance(concurrency, dict):
        return False
    # Fail closed: an expression may evaluate to true, so only an explicit
    # false, or no setting at all, is read as queueing.
    value = concurrency.get("cancel-in-progress", False)
    return value not in (False, "false")


def cancelling_scopes(document: dict[str, typ.Any]) -> list[str]:
    """Return every scope of a workflow whose concurrency may cancel a run.

    Parameters
    ----------
    document : dict[str, typ.Any]
        One parsed workflow document.

    Returns
    -------
    list[str]
        `workflow` and `job <name>` for each scope whose group may cancel.

    >>> cancelling_scopes({"concurrency": {"group": "g", "cancel-in-progress": True}})
    ['workflow']
    >>> cancelling_scopes({"jobs": {"a": {"concurrency": {"group": "g"}}}})
    []
    """
    scopes = ["workflow"] if _cancels(document.get("concurrency")) else []
    jobs = document.get("jobs")
    scopes.extend(
        f"job {name}"
        for name, job in (jobs.items() if isinstance(jobs, dict) else ())
        if isinstance(job, dict) and _cancels(job.get("concurrency"))
    )
    return scopes
