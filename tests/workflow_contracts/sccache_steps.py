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
    """
    haystack = " ".join(
        str(step.get(field, "")) for field in ("name", "run", "uses")
    ).lower()
    starts_the_server = str(step.get("uses", "")).split("@", 1)[0] == SETUP_RUST_PATH
    return starts_the_server or "sccache" in haystack


def sccache_step_indices(job: dict[str, Any]) -> list[tuple[int, str]]:
    """Return the indexed names of every sccache-related step in a job."""
    return [
        (index, str(step.get("name", f"step {index}")))
        for index, step in enumerate(job_steps(job))
        if mentions_sccache(step)
    ]


def ubicloud_jobs_on(backend: str) -> list[tuple[str, str]]:
    """Return the Ubicloud jobs whose workflow selects ``backend``.

    For example, with both Linux workflows on the Actions backend,
    ``ubicloud_jobs_on("gha")`` yields `coverage-check`, `linux-full` and
    `coverage-upload`, and never the rolling-release build lanes.
    """
    return [
        (job_name, workflow_name)
        for job_name, workflow_name in UBICLOUD_JOBS.items()
        if backend_for(workflow_name) == backend
    ]
