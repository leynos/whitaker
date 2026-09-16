"""Holds each lane on the runner it is supposed to run on.

Placement used to be readable off a job's ``runs-on`` and nothing else. Two
matrix legs of the rolling release broke that: the job declares one
``runs-on``, five legs resolve it five ways, and two of those five are the
Linux placements this repository cares about. These contracts read the label
each lane actually resolves to, so a leg can be held to an image without
holding its four siblings to the same one.

The x86_64 lints leg is the one with a consequence outside this repository.
Its image decides the glibc every consumer of whitaker's Linux binaries must
have, so the contract pins the image and the check that enforces it together:
either alone is defeatable, because an image can be raised while the check
still names the old ceiling, and a ceiling can be raised while the image is
untouched.
"""

from __future__ import annotations

import pytest
from runner_lanes import (
    LEG_DISCRIMINATOR,
    GLIBC_BASELINE_IMAGE,
    GLIBC_BASELINE_LANE,
    GLIBC_BASELINE_MAXIMUM,
    GLIBC_BASELINE_SCRIPT,
    GLIBC_BASELINE_STEP,
    UBICLOUD_LABEL_PREFIX,
    UBICLOUD_LANES,
    RunnerLane,
    lane_label,
    load_lane,
    matrix_legs,
)
from ubicloud_workflow_support import (
    UBICLOUD_JOBS,
    all_jobs,
    restore_steps,
    steps_by_name,
)


@pytest.mark.parametrize("lane", UBICLOUD_LANES, ids=str)
def test_every_declared_ubicloud_lane_resolves_to_an_ubicloud_label(
    lane: RunnerLane,
) -> None:
    """Each lane this repository claims runs on Ubicloud actually does."""
    label = lane_label(lane)
    assert label.startswith(UBICLOUD_LABEL_PREFIX), (
        f"{lane} is declared an Ubicloud lane but resolves to {label!r}"
    )


def test_the_lane_list_and_the_job_list_name_the_same_whole_jobs() -> None:
    """The two vocabularies cannot be extended one without the other.

    `UBICLOUD_JOBS` predates matrix legs and still drives the cache contracts,
    which are about a step list and so apply to a matrix job whole. This list
    names placements, so a matrix job appears once per Ubicloud leg. Reducing
    the lanes to their jobs and comparing the two stops a job being added to
    one and forgotten in the other, which would leave it uncached or unplaced
    with every contract still green.
    """
    from_lanes = {(lane.workflow, lane.job) for lane in UBICLOUD_LANES}
    from_jobs = {(workflow, job) for job, workflow in UBICLOUD_JOBS.items()}
    assert from_lanes == from_jobs


def test_no_lane_takes_its_label_from_a_broken_folded_scalar() -> None:
    """A line break inside ``runs-on`` survives the parse and is evaluated.

    Read from the parsed document rather than the raw text, because that is
    where the break shows up as a character in the value. GitHub evaluates
    the broken value regardless, so a green run is not evidence.
    """
    for workflow, job_name, job in all_jobs():
        runs_on = job.get("runs-on")
        if not isinstance(runs_on, str):
            continue
        assert "\n" not in runs_on, (
            f"{workflow}:{job_name} declares a runs-on containing a line break: {runs_on!r}"
        )


def test_only_the_declared_lanes_run_on_ubicloud() -> None:
    """Nothing else has drifted onto a paid runner.

    The rule has to be narrow as well as sufficient. Without this, moving a
    macOS or Windows leg onto Ubicloud, which cannot serve it, would leave
    every other contract here green.
    """
    declared = {(lane.workflow, lane.job, lane.leg) for lane in UBICLOUD_LANES}
    found: set[tuple[str, str, str | None]] = set()
    for workflow, job_name, job in all_jobs():
        runs_on = job.get("runs-on")
        if not isinstance(runs_on, str):
            continue
        legs = matrix_legs(job)
        if legs:
            found.update(
                (workflow, job_name, selector)
                for selector, entry in legs.items()
                if str(entry.get("os", "")).startswith(UBICLOUD_LABEL_PREFIX)
            )
        elif runs_on.startswith(UBICLOUD_LABEL_PREFIX):
            found.add((workflow, job_name, None))
    assert found == declared


def test_the_glibc_baseline_leg_stays_on_the_image_that_supplies_it() -> None:
    """The x86_64 lints leg's image is the floor every consumer inherits."""
    label = lane_label(GLIBC_BASELINE_LANE)
    assert GLIBC_BASELINE_IMAGE in label, (
        f"{GLIBC_BASELINE_LANE} must build on the {GLIBC_BASELINE_IMAGE} image, "
        f"which ships the glibc its baseline check enforces; got {label!r}"
    )


def test_the_glibc_baseline_check_runs_on_that_leg_at_that_ceiling() -> None:
    """The image and the check that enforces it are pinned together.

    Asserted as the command and its argument rather than as the step's
    presence: a step renamed, or left in place with its ceiling raised, is
    the failure this exists to catch.
    """
    step = steps_by_name(load_lane(GLIBC_BASELINE_LANE)).get(GLIBC_BASELINE_STEP)
    assert step is not None, f"{GLIBC_BASELINE_LANE} must declare a {GLIBC_BASELINE_STEP!r} step"
    condition = str(step.get("if", ""))
    assert GLIBC_BASELINE_LANE.leg is not None
    assert GLIBC_BASELINE_LANE.leg in condition, (
        f"{GLIBC_BASELINE_STEP!r} must be guarded to the {GLIBC_BASELINE_LANE.leg} leg; "
        f"got {condition!r}"
    )
    script = str(step.get("run", ""))
    assert GLIBC_BASELINE_SCRIPT in script, (
        f"{GLIBC_BASELINE_STEP!r} must run {GLIBC_BASELINE_SCRIPT}; got {script!r}"
    )
    assert f"--maximum-glibc {GLIBC_BASELINE_MAXIMUM}" in script, (
        f"{GLIBC_BASELINE_STEP!r} must enforce {GLIBC_BASELINE_MAXIMUM}, the glibc the "
        f"{GLIBC_BASELINE_IMAGE} image ships; got {script!r}"
    )


def test_a_matrix_job_keys_its_caches_apart_from_its_own_legs() -> None:
    """Two legs of one job share a step list, so they share a key template.

    That is the hazard a matrix brings and a single-placement job cannot
    have. The x86_64 and arm64 lints legs run the same restore and the same
    save; if the rendered key does not vary with the leg, the second leg to
    finish overwrites the first leg's archive under the same key, and both
    legs then restore an archive built for the other architecture. The
    discriminator is `runner.arch`, which is X64 on one and ARM64 on the
    other.

    Only the restore steps declare a key template. Each save reuses the
    primary key its restore rendered, which another contract asserts, so a
    discriminated restore key is a discriminated save key.
    """
    matrix_lanes = {lane for lane in UBICLOUD_LANES if lane.leg is not None}
    assert matrix_lanes, "this contract needs at least one matrix lane to be about"
    for lane in sorted(matrix_lanes):
        job = load_lane(lane)
        restores = restore_steps(job)
        assert restores, f"{lane} is a caching lane and must declare restore steps"
        for step in restores:
            declared = step.get("with", {})
            key = str(declared.get("key", ""))
            assert LEG_DISCRIMINATOR in key, (
                f"{lane}: {step['name']!r} keys an archive without "
                f"{LEG_DISCRIMINATOR}, so its legs would overwrite each other: {key!r}"
            )
            fallback = str(declared.get("restore-keys", ""))
            if fallback.strip():
                assert LEG_DISCRIMINATOR in fallback, (
                    f"{lane}: {step['name']!r} falls back to a key without "
                    f"{LEG_DISCRIMINATOR}, so a leg could restore another leg's archive"
                )
