"""Validate Ubicloud cache ownership, write policy, and observability."""

from __future__ import annotations

import re

from ubicloud_workflow_support import (
    CACHE_KEY_WRITERS,
    CACHING_JOBS,
    RESTORE_ACTION,
    SAVE_ACTION,
    cache_paths,
    duplicate_path_owners,
    job_steps,
    key_family,
    load_job,
    restore_steps,
    save_steps,
    step_names,
    steps_by_name,
)

PRIMARY_KEY_REFERENCE = re.compile(
    r"^\$\{\{\s*steps\.([\w-]+)\.outputs\.cache-primary-key\s*\}\}$"
)
TRUSTED_BRANCH_GUARD = "github.ref == 'refs/heads/main'"
FIRST_EXPENSIVE_STEP = "Setup Rust"


def _restore_step_by_id(job: dict[str, object], step_id: str) -> dict[str, object]:
    """Return the restore step carrying the supplied identifier."""
    for step in restore_steps(job):
        if step.get("id") == step_id:
            return step
    raise AssertionError(f"no restore step declares id {step_id!r}")


def test_every_lane_uses_one_pinned_cache_action() -> None:
    """One action and one pin across Ubicloud and GitHub-hosted lanes.

    Ubicloud's transparent cache intercepts `actions/cache` v6.1.0, so Linux
    archives reach Ubicloud's store and Windows archives reach GitHub's
    without the deprecated `ubicloud/cache` fork, which needs virtual-machine
    variables a GitHub-hosted runner never supplies.
    """
    for job_name in CACHING_JOBS:
        job = load_job(job_name)
        for step in job_steps(job):
            uses = str(step.get("uses", ""))
            assert not uses.startswith("ubicloud/cache"), (
                f"{job_name} must not use the deprecated ubicloud/cache fork"
            )
            assert not uses.startswith("namespacelabs/"), (
                f"{job_name} must not retain a Namespace cache action"
            )
        assert restore_steps(job), f"{job_name} must restore at least one cache"


def test_no_job_archives_a_cargo_target_tree() -> None:
    """sccache is the single owner of compiler output, for every build shape.

    An archived `target` tree would be a second owner, would be invalidated far
    more often than the registry, and cannot hold more than one of the debug
    and instrumented shapes at a time.
    """
    for job_name in CACHING_JOBS:
        job = load_job(job_name)
        for step in restore_steps(job) + save_steps(job):
            for path in cache_paths(step):
                assert "target" not in path.split("/"), (
                    f"{job_name}: {step['name']!r} must not archive {path}"
                )


def _assert_one_owner_per_path(
    job_name: str,
    steps: list[dict[str, object]],
) -> None:
    """Assert that no two of the supplied cache steps claim the same path.

    For example, passing a job's restore steps fails if both the registry and
    the tools step list ``~/.cargo/bin``.
    """
    conflicts = duplicate_path_owners(steps)
    assert not conflicts, f"{job_name}: each path needs one owner, but " + "; ".join(
        f"{path} is claimed by {owners}" for path, owners in conflicts.items()
    )


def test_each_cached_path_has_exactly_one_owner_per_job() -> None:
    """Two cache steps claiming one directory would race to define its content."""
    for job_name in CACHING_JOBS:
        job = load_job(job_name)
        _assert_one_owner_per_path(job_name, restore_steps(job))
        _assert_one_owner_per_path(job_name, save_steps(job))


def test_cache_restores_precede_every_expensive_install_or_build() -> None:
    """A restore after the first install would download what it already holds."""
    for job_name in CACHING_JOBS:
        job = load_job(job_name)
        names = step_names(job)
        setup_index = names.index(FIRST_EXPENSIVE_STEP)
        for step in restore_steps(job):
            assert names.index(str(step["name"])) < setup_index, (
                f"{job_name}: {step['name']!r} must restore before "
                f"{FIRST_EXPENSIVE_STEP!r}"
            )


def test_saves_are_restricted_to_the_trusted_branch() -> None:
    """A pull request reads the published generation and never republishes it."""
    for job_name in CACHING_JOBS:
        job = load_job(job_name)
        for step in save_steps(job):
            condition = str(step.get("if", ""))
            assert TRUSTED_BRANCH_GUARD in condition, (
                f"{job_name}: {step['name']!r} must save only on the trunk"
            )


def test_saves_reuse_the_rendered_key_and_paths_of_their_restore() -> None:
    """A save that re-renders its key can drift from the key it restored."""
    for job_name in CACHING_JOBS:
        job = load_job(job_name)
        for step in save_steps(job):
            key = str(step.get("with", {}).get("key", ""))
            match = PRIMARY_KEY_REFERENCE.match(key)
            assert match is not None, (
                f"{job_name}: {step['name']!r} must save the primary key its "
                "restore step rendered"
            )
            restore = _restore_step_by_id(job, match.group(1))
            assert cache_paths(step) == cache_paths(restore), (
                f"{job_name}: {step['name']!r} must save exactly the paths "
                f"{restore['name']!r} restored"
            )


def test_every_cache_key_family_has_exactly_one_writer() -> None:
    """Never let two lanes populate the same cold key."""
    observed: dict[str, str] = {}
    for job_name in CACHING_JOBS:
        job = load_job(job_name)
        for step in save_steps(job):
            key = str(step["with"]["key"])
            match = PRIMARY_KEY_REFERENCE.match(key)
            assert match is not None
            prefix = str(_restore_step_by_id(job, match.group(1))["with"]["key"])
            family = key_family(prefix, CACHE_KEY_WRITERS)
            assert family is not None, f"unreviewed cache key family: {prefix}"
            previous = observed.get(family)
            assert previous is None, (
                f"{family} is written by both {previous} and {job_name}"
            )
            observed[family] = job_name

    for family, writer in CACHE_KEY_WRITERS.items():
        assert observed.get(family) == writer, (
            f"{family} must be written only by {writer}"
        )


def test_restore_only_lanes_never_save() -> None:
    """`coverage-check` runs on pull requests alone, so it can never be a writer."""
    assert not save_steps(load_job("coverage-check")), (
        "coverage-check must restore the coverage lane's keys without saving"
    )


def test_every_restore_step_is_reported_in_the_job_summary() -> None:
    """An unexplained miss is indistinguishable from a broken key without this."""
    for job_name in CACHING_JOBS:
        job = load_job(job_name)
        steps = steps_by_name(job)
        observations = steps["Record cache observations"]
        assert observations.get("if") == "always()", (
            f"{job_name} must record cache observations even when the job fails"
        )
        reported = "\n".join(
            f"{key}: {value}" for key, value in observations["env"].items()
        )
        for step in restore_steps(job):
            step_id = str(step["id"])
            assert f"steps.{step_id}.outputs.cache-primary-key" in reported, (
                f"{job_name}: {step['name']!r} must report its rendered key"
            )
            assert f"steps.{step_id}.outputs.cache-hit" in reported, (
                f"{job_name}: {step['name']!r} must report its cache-hit result"
            )
            assert f"steps.{step_id}.outputs.cache-matched-key" in reported, (
                f"{job_name}: {step['name']!r} must report the key it matched, "
                "so a prefix restore is distinguishable from a cold miss"
            )


def test_cache_actions_stay_on_one_reviewed_pin() -> None:
    """Restore and save must not drift onto different versions."""
    for job_name in CACHING_JOBS:
        job = load_job(job_name)
        for step in job_steps(job):
            uses = str(step.get("uses", ""))
            if uses.startswith("actions/cache/restore"):
                assert uses == RESTORE_ACTION, f"{job_name} restore pin drifted"
            if uses.startswith("actions/cache/save"):
                assert uses == SAVE_ACTION, f"{job_name} save pin drifted"
