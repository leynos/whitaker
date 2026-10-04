"""Recognize the steps that start or feed sccache in a workflow job.

`setup_rust_sccache_contract_test` asks of every Ubicloud job which steps
start the sccache server or talk to it, so that nothing but `setup-rust` does.
The recognizers live here, apart from the rules, so a later contract asking
the same question reads jobs the same way.

The module also holds the fallback guard's vocabulary. `setup-rust` reports
`sccache-status=fallback` when the server would not start and the job compiles
uncached, so every step that records, uploads or health-checks sccache's
statistics must stand down. `normalized_condition` reduces a step's `if:` to a
comparable string, and `guards_against_fallback` says whether that string is a
pure conjunction holding the skip. `sccache_health_contract_test` uses them on
the three suite lanes' record, upload and health steps and their order;
`sccache_fallback_guard_test` uses them on every statistics-reading step in
every workflow, found by the command it runs rather than by its job or name.
"""

from __future__ import annotations

import typing as typ

from ubicloud_workflow_support import (
    SETUP_RUST_ACTION,
    job_steps,
)

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from typing import Any

#: The shared Rust setup action, without its ref. It starts the sccache
#: server, so it binds the backend in whatever environment it finds.
SETUP_RUST_PATH: str = SETUP_RUST_ACTION.split("@", 1)[0]


def mentions_sccache(step: dict[str, Any]) -> bool:
    """Return whether a step names, runs, invokes or starts sccache.

    Deliberately generous. A false positive here only tightens the ordering
    rules that use it, while a false negative would let a step that starts a
    server sit ahead of the credentials export and go unnoticed, which is the
    whole failure being guarded.

    `Setup Rust` is the step that starts the server, and neither its name nor
    its `uses:` says so. Reading only for the word let the export and the
    selector move below it together and still pass, so the action is named.

    Parameters
    ----------
    step : dict[str, Any]
        One parsed workflow step.

    Returns
    -------
    bool
        ``True`` when the step's name, script or action mentions sccache, or
        when it is the shared Rust setup action that starts the server.

    >>> mentions_sccache({"name": "Record sccache effectiveness", "run": "true"})
    True
    >>> mentions_sccache({"name": "Checkout", "uses": "actions/checkout@v7"})
    False
    """
    haystack = " ".join(
        str(step.get(field, "")) for field in ("name", "run", "uses")
    ).lower()
    starts_the_server = str(step.get("uses", "")).split("@", 1)[0] == SETUP_RUST_PATH
    return starts_the_server or "sccache" in haystack


def sccache_step_indices(job: dict[str, Any]) -> list[tuple[int, str]]:
    """Return the position and name of every sccache-related step in a job.

    Parameters
    ----------
    job : dict[str, Any]
        One parsed workflow job with a step list.

    Returns
    -------
    list[tuple[int, str]]
        Each step `mentions_sccache` recognizes, as its index in the step list
        and its name, in declaration order.

    >>> sccache_step_indices(
    ...     {"steps": [{"name": "Checkout"}, {"name": "Start sccache", "run": "x"}]}
    ... )
    [(1, 'Start sccache')]
    """
    return [
        (index, str(step.get("name", f"step {index}")))
        for index, step in enumerate(job_steps(job))
        if mentions_sccache(step)
    ]


#: The skip every sccache evidence step carries, whitespace removed. `setup-rust`
#: reports `fallback` when sccache's server would not start and the job compiles
#: uncached (shared-actions #546). A job with no server has no statistics, and
#: `sccache --show-stats` prints empty defaults for it, so a step that records,
#: uploads or health-checks them stands down rather than publish a table of
#: zeros that reads as a broken integration.
NOT_FALLBACK: str = "steps.setup-rust.outputs.sccache-status!='fallback'"


def normalized_condition(step: dict[str, Any]) -> str:
    """Return a step's `if` with whitespace and expression braces removed.

    Parameters
    ----------
    step : dict[str, Any]
        A parsed workflow step. A step with no `if` yields an empty string.

    Returns
    -------
    str
        The condition with every space and any enclosing `${{ }}` removed, so
        two spellings of one condition compare equal.

    >>> normalized_condition({"if": "${{ always() && x }}"})
    'always()&&x'
    >>> normalized_condition({})
    ''
    """
    text = str(step.get("if", "")).replace(" ", "")
    return text.removeprefix("${{").removesuffix("}}")


def guards_against_fallback(condition: str) -> bool:
    """Return whether a normalized condition stands down on a fallback.

    Parameters
    ----------
    condition : str
        A condition already reduced by `normalized_condition`.

    Returns
    -------
    bool
        True when no `||` appears and one `&&`-separated conjunct is
        `NOT_FALLBACK`.

    The guard must be one conjunct of a pure conjunction. A disjunction
    anywhere voids it, because `always()||guard` and `x||y&&guard` both still
    run against a dead server through the other arm, and an inverted or negated
    comparison is a different conjunct, so it does not match.

    >>> guards_against_fallback(f"always()&&{NOT_FALLBACK}")
    True
    >>> guards_against_fallback(f"always()||{NOT_FALLBACK}")
    False
    >>> guards_against_fallback("always()")
    False
    """
    if "||" in condition:
        return False
    return NOT_FALLBACK in condition.split("&&")
