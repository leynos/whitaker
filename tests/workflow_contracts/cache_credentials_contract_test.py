"""Validate where the Ubicloud cache-proxy credentials sit in each job.

Two kinds of Ubicloud job need `export-ubicloud-cache-credentials`. A job on
sccache's Actions backend needs it before its server starts, or the server
binds local disk. A job that saves an `actions/cache` archive needs it before
`Setup Rust`: `mozilla-actions/sccache-action` writes
`ACTIONS_CACHE_SERVICE_V2=on` to `GITHUB_ENV`, the pinned `setup-rust` puts
back only a value that was set before it ran, and every later save then goes
to GitHub's v2 service while every restore reads Ubicloud's proxy.

The second kind was learned from the rolling release. Its build jobs are on the
local-directory backend, so the Actions-backend rule never reached them, and
every run restored nothing and saved an archive no restore could see.
"""

from __future__ import annotations

from runner_lanes import GITHUB_HOSTED_LABELS, LINUX_LEG_GUARD, declared_labels
from sccache_steps import sccache_step_indices, ubicloud_jobs_on
from ubicloud_workflow_support import (
    CREDENTIALS_ACTION_PATH,
    CREDENTIALS_STEP,
    UBICLOUD_JOBS,
    load_job,
    save_steps,
    step_names,
    steps_by_name,
)


def _ubicloud_jobs_that_save() -> list[tuple[str, str]]:
    """Return the Ubicloud jobs that save a cache archive, whatever their backend.

    For example, the two rolling-release build jobs are on the local-directory
    backend, so `ubicloud_jobs_on("gha")` never yields them, but both save
    their registry and compiler-cache archives on a push to `main`.
    """
    return [
        (job_name, workflow_name)
        for job_name, workflow_name in UBICLOUD_JOBS.items()
        if save_steps(load_job(job_name))
    ]


def _credential_lanes() -> list[tuple[str, str]]:
    """Return every Ubicloud job that needs the proxy credentials, once each.

    Two reasons put a job here. A job on the Actions backend needs them before
    its sccache server starts. A job that saves an archive needs them before
    `Setup Rust`: `mozilla-actions/sccache-action` writes
    `ACTIONS_CACHE_SERVICE_V2=on` to `GITHUB_ENV`, `setup-rust` puts back only
    a value that was set before it ran, and a later `actions/cache/save` then
    writes to GitHub's v2 service while every restore reads Ubicloud's proxy.
    """
    lanes = dict(ubicloud_jobs_on("gha"))
    lanes.update(_ubicloud_jobs_that_save())
    return sorted(lanes.items())


def test_the_credential_rule_reaches_every_saving_lane() -> None:
    """The rule below must cover the rolling-release build jobs by name.

    Without this presence half, a reader that stopped recognizing save steps
    would shrink `_credential_lanes` to the Actions-backend jobs and the rule
    would pass over the very jobs whose archives went to the wrong store:
    every rolling-release run from 2026-09-24 restored nothing and saved to
    GitHub, 18 `sccache-rolling-v1-` entries of about 190 MB each.
    """
    lanes = {job_name for job_name, _ in _credential_lanes()}
    for job_name in ("build-lints", "build-dependency-binaries"):
        assert job_name in lanes, (
            f"{job_name} saves cache archives on Ubicloud, so the credential "
            "rule must reach it"
        )


def test_credential_lanes_export_the_proxy_credentials_first() -> None:
    """A server or a save that runs before the export talks to the wrong store.

    On Ubicloud the Actions cache service is a proxy on the runner's private
    network, advertised to action steps alone. Until it is republished through
    `GITHUB_ENV`, a `run:` step cannot see it, and sccache silently falls back
    to a directory nothing archives. On a saving lane the export must also
    precede `Setup Rust`, or nothing restores the cleared v2 flag after it and
    the saves go to GitHub; see `_credential_lanes`.
    """
    lanes = _credential_lanes()
    assert lanes, "no Ubicloud lane needs the proxy credentials; this rule is dead"
    for job_name, _ in lanes:
        job = load_job(job_name)
        names = step_names(job)
        assert CREDENTIALS_STEP in names, (
            f"{job_name} runs sccache on the Actions backend or saves a cache "
            f"archive without {CREDENTIALS_STEP!r}"
        )
        # One export, or the index below and `steps_by_name` in the identity
        # rule could each judge a different step of the same name.
        assert names.count(CREDENTIALS_STEP) == 1, (
            f"{job_name} must declare exactly one {CREDENTIALS_STEP!r} step"
        )
        credentials_index = names.index(CREDENTIALS_STEP)
        later = sccache_step_indices(job) + [
            (names.index(str(step["name"])), str(step["name"]))
            for step in save_steps(job)
        ]
        for index, name in later:
            assert credentials_index < index, (
                f"{job_name}: {CREDENTIALS_STEP!r} must precede {name!r}, "
                "because sccache binds its backend when its server starts and "
                "`Setup Rust` is what redirects a later save to GitHub"
            )


def test_the_credentials_step_uses_the_action_that_exports_them() -> None:
    """A step carrying the name but not the action satisfies order and nothing else."""
    for job_name, _ in _credential_lanes():
        step = steps_by_name(load_job(job_name)).get(CREDENTIALS_STEP)
        assert step is not None, (
            f"{job_name} has no {CREDENTIALS_STEP!r} step at all"
        )
        uses = str(step.get("uses", ""))
        assert uses.startswith(f"{CREDENTIALS_ACTION_PATH}@"), (
            f"{job_name}: {CREDENTIALS_STEP!r} must run "
            f"{CREDENTIALS_ACTION_PATH}, not {uses!r}"
        )
        _, _, ref = uses.partition("@")
        assert len(ref) == 40 and all(c in "0123456789abcdef" for c in ref), (
            f"{job_name}: {CREDENTIALS_STEP!r} must pin a full commit SHA, "
            f"not {ref!r}"
        )


def test_the_credentials_step_runs_on_every_ubicloud_leg_and_no_hosted_one() -> None:
    """The export fails closed on a GitHub-hosted runner, and is needed on Ubicloud.

    A job placed only on Ubicloud runs it unconditionally. A matrix job with
    GitHub-hosted legs runs it on its Linux legs, which are its Ubicloud legs,
    under exactly the guard its other Linux-only cache steps use: a wider
    guard turns the macOS and Windows legs red, and a narrower one leaves a
    Ubicloud leg saving to GitHub again.
    """
    for job_name, _ in _credential_lanes():
        job = load_job(job_name)
        condition = steps_by_name(job)[CREDENTIALS_STEP].get("if")
        hosted = declared_labels(job) & GITHUB_HOSTED_LABELS
        expected = LINUX_LEG_GUARD if hosted else None
        assert condition == expected, (
            f"{job_name}: {CREDENTIALS_STEP!r} must be guarded by "
            f"{expected!r}, since its hosted legs are {sorted(hosted)}, "
            f"not {condition!r}"
        )
