"""Validate which store each Ubicloud lane's compiler cache actually reaches.

sccache binds its backend once, when its server starts, and then reports a
plausible hit rate whatever it bound. Both ways of getting this wrong are
silent in a green run: a lane on the Actions backend whose server started
before the Ubicloud proxy's credentials were published writes to local disk
that nothing archives, and a lane on the local-directory backend whose
archive no job saves reads an empty directory every time. The rules here are
what makes either visible in the checked-in workflows.

Whitaker has paid for both. Runs 33748602187 and 33756048103 reported
`Cache location ghac` with a write error for every one of 3,788 and 2,245
store attempts, because nothing had cleared `ACTIONS_CACHE_SERVICE_V2` and
sccache was addressing GitHub's v2 service past the proxy. The repository
then spent weeks on a local directory whose pull-request restores matched
only what a `workflow_dispatch` run on `main` had happened to save.
"""

from __future__ import annotations

import typing as typ

from sccache_steps import ubicloud_jobs_on
from ubicloud_workflow_support import (
    CACHE_KEY_WRITERS,
    SCCACHE_DIRECTORY,
    UBICLOUD_JOBS,
    all_jobs,
    backend_for,
    cache_paths,
    job_steps,
    key_family,
    load_job,
    load_workflow,
    restore_steps,
    save_steps,
    sccache_directory_steps,
)

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from typing import Any

#: Backends `scripts/select-sccache-backend.sh` accepts. Anything else is a
#: typo the script rejects at the lane's first step.
KNOWN_BACKENDS: frozenset[str] = frozenset({"gha", "local"})

#: The workflows that declare the Ubicloud lanes, deduplicated.
UBICLOUD_WORKFLOWS: frozenset[str] = frozenset(UBICLOUD_JOBS.values())


def _workflow_env(workflow_name: str) -> dict[str, Any]:
    """Return one workflow's top-level environment mapping."""
    return load_workflow(workflow_name).get("env") or {}


def test_every_ubicloud_workflow_names_a_backend_the_selector_understands() -> None:
    """An unrecognized selector fails the lane's first step, not its build."""
    for workflow_name in sorted(UBICLOUD_WORKFLOWS):
        backend = backend_for(workflow_name)
        assert backend in KNOWN_BACKENDS, (
            f"{workflow_name} selects {backend!r}, which "
            "scripts/select-sccache-backend.sh does not understand"
        )


def test_the_two_linux_workflows_select_the_same_backend() -> None:
    """`coverage-check` reads the compiler cache `coverage-upload` writes.

    The rule is about these two files and no others: they are one lane's
    reader and writer split across a pull-request workflow and a push
    workflow. `rolling-release.yml` publishes from its own key family and is
    free to differ, which is why this is not a repository-wide equality.
    """
    declared = {name: backend_for(name) for name in ("ci.yml", "coverage-main.yml")}
    assert len(set(declared.values())) == 1, (
        "the pull-request lanes and the trunk writer must share one backend, "
        f"not {declared}"
    )


def test_ghac_lanes_own_no_compiler_cache_directory() -> None:
    """Two configured backends make the reported hit rate unattributable.

    A lane that archives `~/.cache/sccache` while its server writes to the
    proxy pays for an archive nothing reads, and the next reviewer cannot tell
    from the hit rate which store produced it.
    """
    for job_name, _ in ubicloud_jobs_on("gha"):
        owned = sccache_directory_steps(load_job(job_name))
        assert not owned, (
            f"{job_name} is on the Actions backend but still archives "
            f"{SCCACHE_DIRECTORY} in {[step['name'] for step in owned]}"
        )


def test_local_backend_lanes_restore_the_directory_they_are_pointed_at() -> None:
    """The narrow half: a `local` lane must still own its directory.

    Without this, `test_ghac_lanes_own_no_compiler_cache_directory` could be
    satisfied everywhere by deleting every cache step in the repository and
    leaving every lane compiling from cold.
    """
    lanes = ubicloud_jobs_on("local")
    assert lanes, (
        "no Ubicloud lane is on the local-directory backend; this rule is dead"
    )
    for job_name, _ in lanes:
        job = load_job(job_name)
        restored = [
            step for step in restore_steps(job) if SCCACHE_DIRECTORY in cache_paths(step)
        ]
        assert restored, (
            f"{job_name} selects the local-directory backend, so sccache "
            f"writes {SCCACHE_DIRECTORY}, but no step restores it"
        )


def _jobs_with_steps() -> list[tuple[str, str, dict[str, Any]]]:
    """Return every job that declares its own step list.

    A job that calls a reusable workflow has none, and so can own no cache
    step here.
    """
    return [
        (workflow_name, job_name, job)
        for workflow_name, job_name, job in all_jobs()
        if isinstance(job.get("steps"), list)
    ]


def _owns_the_directory(step: dict[str, Any]) -> bool:
    """Return whether one cache step claims the local sccache directory."""
    return SCCACHE_DIRECTORY in cache_paths(step)


def _directory_writers() -> set[str]:
    """Return ``workflow:job`` for every job that saves the sccache directory."""
    return {
        f"{workflow_name}:{job_name}"
        for workflow_name, job_name, job in _jobs_with_steps()
        if any(_owns_the_directory(step) for step in save_steps(job))
    }


def _directory_readers() -> dict[str, list[str]]:
    """Return each restored key family mapped to the jobs that restore it.

    Fails where a restore names a family nobody has reviewed, because the
    writer lookup below could not then say anything about it either.
    """
    readers: dict[str, list[str]] = {}
    for workflow_name, job_name, job in _jobs_with_steps():
        for step in restore_steps(job):
            if not _owns_the_directory(step):
                continue
            family = key_family(str(step["with"]["key"]), CACHE_KEY_WRITERS)
            assert family is not None, (
                f"{workflow_name}:{job_name} restores {SCCACHE_DIRECTORY} under "
                "an unreviewed key family"
            )
            readers.setdefault(family, []).append(f"{workflow_name}:{job_name}")
    return readers


def test_every_restored_compiler_cache_directory_has_a_writer() -> None:
    """A directory nothing saves is read empty on every run, forever.

    This is the rule the pull-request lanes failed for weeks: they restored a
    family whose only writer was a `workflow_dispatch` run on `main`, so an
    ordinary pull request compiled almost everything.
    """
    written = _directory_writers()
    for family, readers in sorted(_directory_readers().items()):
        writer = CACHE_KEY_WRITERS[family]
        assert any(entry.endswith(f":{writer}") for entry in written), (
            f"{family} is restored by {readers} but {writer} saves no "
            f"{SCCACHE_DIRECTORY} archive, so every restore reads nothing"
        )


#: The one job exempt from the rule below. `windows-compat` is GitHub-hosted,
#: runs no selector step, and declares its own reviewed Actions-backend arm;
#: `runner_placement_contract_test` holds its shape instead.
SELECTOR_EXEMPT_JOBS: frozenset[tuple[str, str]] = frozenset(
    {("ci.yml", "windows-compat")}
)

#: Variables only `scripts/select-sccache-backend.sh` may export. Either one
#: set elsewhere configures a second backend the script cannot see.
BACKEND_VARIABLES: tuple[str, ...] = ("SCCACHE_GHA_ENABLED", "SCCACHE_DIR")


def _backend_variables_set_outside_the_selector() -> list[str]:
    """Return every place a backend variable is declared, with its scope.

    For example, a workflow that added `SCCACHE_DIR` to its top-level
    environment yields ``"ci.yml:linux-full sets SCCACHE_DIR at workflow
    level"``.
    """
    offenders: list[str] = []
    for workflow_name, job_name, job in _jobs_with_steps():
        if (workflow_name, job_name) in SELECTOR_EXEMPT_JOBS:
            continue
        scopes = [
            ("workflow", _workflow_env(workflow_name)),
            ("job", job.get("env") or {}),
        ]
        # A step's own `env` configures that step's sccache as surely as a
        # wider scope, and `Setup Rust` is the step that starts the server.
        scopes.extend(
            (f"step {index}", step.get("env") or {})
            for index, step in enumerate(job_steps(job))
        )
        offenders.extend(
            f"{workflow_name}:{job_name} sets {variable} at {scope} level"
            for scope, env in scopes
            for variable in BACKEND_VARIABLES
            if variable in env
        )
    return offenders


def test_no_lane_configures_both_backends_at_once() -> None:
    """sccache prefers whichever backend it finds first and reports neither.

    The selector script is the single place that exports these variables, so a
    workflow or job that also sets one has configured a second backend the
    script cannot see.
    """
    offenders = _backend_variables_set_outside_the_selector()
    assert not offenders, (
        "scripts/select-sccache-backend.sh must be the only exporter of the "
        f"backend variables, but {offenders}"
    )
