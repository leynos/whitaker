"""Recognize the steps that start or feed sccache in a workflow job.

Two contract modules ask the same questions of a job: which steps start the
sccache server or talk to it, and which Ubicloud jobs sit on a given backend.
`sccache_backend_contract_test` asks them about the backend each lane reaches,
and `cache_credentials_contract_test` about where the proxy credentials must
sit. The answers live here so the two cannot drift apart.
"""

from __future__ import annotations

import typing as typ

from ubicloud_workflow_support import (
    SETUP_RUST_ACTION,
    UBICLOUD_JOBS,
    backend_for,
    job_steps,
)

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from collections.abc import Callable, Mapping
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


def jobs_on_backend(
    jobs: Mapping[str, str],
    backend_of: Callable[[str], str],
    backend: str,
) -> list[tuple[str, str]]:
    """Return the jobs whose workflow selects ``backend``, reading nothing.

    The workflow lookup is passed in, so the selection itself is a pure
    function over the job registry and can be exercised without files.

    Parameters
    ----------
    jobs : Mapping[str, str]
        Job names mapped to the workflow file that declares each.
    backend_of : Callable[[str], str]
        Returns the backend a workflow file selects.
    backend : str
        The backend to select for, such as ``"gha"`` or ``"local"``.

    Returns
    -------
    list[tuple[str, str]]
        ``(job, workflow)`` pairs whose workflow selects ``backend``, in the
        registry's order.

    >>> jobs_on_backend(
    ...     {"lint": "ci.yml", "build": "release.yml"},
    ...     {"ci.yml": "gha", "release.yml": "local"}.__getitem__,
    ...     "local",
    ... )
    [('build', 'release.yml')]
    """
    return [
        (job_name, workflow_name)
        for job_name, workflow_name in jobs.items()
        if backend_of(workflow_name) == backend
    ]


def ubicloud_jobs_on(backend: str) -> list[tuple[str, str]]:
    """Return the checked-in Ubicloud jobs whose workflow selects ``backend``.

    The loading half of `jobs_on_backend`: it supplies the reviewed registry
    and reads each workflow's declaration from disk. With both Linux
    workflows on the Actions backend, ``ubicloud_jobs_on("gha")`` yields
    `coverage-check`, `linux-full` and `coverage-upload`, and never the
    rolling-release build lanes.

    Parameters
    ----------
    backend : str
        The backend to select for, such as ``"gha"`` or ``"local"``.

    Returns
    -------
    list[tuple[str, str]]
        ``(job, workflow)`` pairs from `UBICLOUD_JOBS`.
    """
    return jobs_on_backend(UBICLOUD_JOBS, backend_for, backend)
