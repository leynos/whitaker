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
import yaml
from runner_lanes import (
    GITHUB_HOSTED_LABELS,
    LEG_DISCRIMINATOR,
    MAXIMUM_LANE_TIMEOUT_MINUTES,
    billable_labels,
    declared_labels,
    GLIBC_BASELINE_IMAGE,
    GLIBC_BASELINE_LANES,
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
    REPOSITORY_ROOT,
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


@pytest.mark.parametrize("lane", GLIBC_BASELINE_LANES, ids=str)
def test_the_glibc_baseline_leg_stays_on_the_image_that_supplies_it(
    lane: RunnerLane,
) -> None:
    """Each x86_64 leg's image is a floor a consumer inherits.

    Both rolling-release matrix jobs publish x86_64 Linux artefacts that a
    consumer installs together, so a raised image on either one raises the
    floor for the pair.
    """
    label = lane_label(lane)
    assert GLIBC_BASELINE_IMAGE in label, (
        f"{lane} must build on the {GLIBC_BASELINE_IMAGE} image, "
        f"which ships the glibc its baseline check enforces; got {label!r}"
    )


@pytest.mark.parametrize("lane", GLIBC_BASELINE_LANES, ids=str)
def test_the_glibc_baseline_check_runs_on_that_leg_at_that_ceiling(
    lane: RunnerLane,
) -> None:
    """The image and the check that enforces it are pinned together.

    Asserted as the command and its argument rather than as the step's
    presence: a step renamed, or left in place with its ceiling raised, is
    the failure this exists to catch.
    """
    step = steps_by_name(load_lane(lane)).get(GLIBC_BASELINE_STEP)
    assert step is not None, f"{lane} must declare a {GLIBC_BASELINE_STEP!r} step"
    condition = str(step.get("if", ""))
    assert lane.leg is not None
    assert lane.leg in condition, (
        f"{lane}: {GLIBC_BASELINE_STEP!r} must be guarded to the {lane.leg} leg; "
        f"got {condition!r}"
    )
    script = str(step.get("run", ""))
    assert GLIBC_BASELINE_SCRIPT in script, (
        f"{lane}: {GLIBC_BASELINE_STEP!r} must run {GLIBC_BASELINE_SCRIPT}; got {script!r}"
    )
    assert f"--maximum-glibc {GLIBC_BASELINE_MAXIMUM}" in script, (
        f"{lane}: {GLIBC_BASELINE_STEP!r} must enforce {GLIBC_BASELINE_MAXIMUM}, the glibc "
        f"the {GLIBC_BASELINE_IMAGE} image ships; got {script!r}"
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

def test_the_actionlint_registry_matches_the_labels_in_use() -> None:
    """The registry and the workflows say the same thing, both ways.

    actionlint knows GitHub's own labels and whatever it has been told, so
    a label it has not been told about is reported as unknown. Adding a leg
    without registering its label leaves the linter failing on a workflow
    that is correct, which trains a reader to ignore it.

    Asserted as an equality rather than as a subset, because a stale
    registration is its own fault: it names a runner assignment that has
    already been retired and hides that the lane moved. A subset catches
    the first and not the second.

    "In use" is read from every job in every workflow, through both arms of
    any conditional, less the labels GitHub hosts. Keyed on what GitHub
    hosts rather than on a vendor's prefix, so the fork fallback's hosted
    arm never reaches this question and a second paid provider needs a
    registration rather than a second prefix to match against. This is the
    estate's shape, taken from chutoro, which had it right first.
    """
    registry = yaml.safe_load(
        (REPOSITORY_ROOT / ".github/actionlint.yaml").read_text(encoding="utf-8")
    )
    registered = set(registry["self-hosted-runner"]["labels"])
    in_use = {label for _, _, job in all_jobs() for label in billable_labels(job)}
    assert registered == in_use, (
        f".github/actionlint.yaml registers {sorted(registered)} but the "
        f"workflows bill for {sorted(in_use)}"
    )


@pytest.mark.parametrize("lane", UBICLOUD_LANES, ids=str)
def test_every_ubicloud_lane_declares_a_timeout(lane: RunnerLane) -> None:
    """A hung Ubicloud job bills until something stops it.

    Ubicloud runners register as self-hosted just-in-time runners, so GitHub's
    five-day self-hosted ceiling applies rather than the six-hour hosted one.
    The developers' guide has required a timeout on every Ubicloud job since
    the migration; until now nothing enforced it, and two matrix jobs reached
    a pull request without one. Documented policy that no contract asserts is
    how that happened.

    Declared on the job, because a matrix job cannot give one leg a different
    timeout from another and the bound is wanted on all of them anyway.
    """
    job = load_lane(lane)
    timeout = job.get("timeout-minutes")
    assert isinstance(timeout, int), (
        f"{lane} runs on a paid runner and must declare timeout-minutes; "
        f"got {timeout!r}"
    )
    assert 0 < timeout <= MAXIMUM_LANE_TIMEOUT_MINUTES, (
        f"{lane} declares timeout-minutes {timeout}, outside the reviewed "
        f"range of 1 to {MAXIMUM_LANE_TIMEOUT_MINUTES}"
    )


def test_an_unrecognised_label_is_reported_rather_than_excused() -> None:
    """The hosted set and a vendor prefix are not the same rule.

    Over this repository's own workflows the two readings agree exactly, so
    swapping one for the other changes no answer and neither can be said to be
    proved by the lanes alone. This is the case that separates them, and it is
    driven directly for that reason.

    `ubuntu-20.04` is a GitHub-hosted family that the estate does not use. A
    vendor-prefix reading excuses it silently, because it does not begin with
    the Ubicloud prefix, and the registry contract then says nothing about a
    lane that has moved to a retired image or acquired a typo. Subtracting a
    named set of what GitHub hosts instead reports it, which is what a lane
    running on something nobody reviewed should do.

    Raised by jm-tiers-c-4 while porting this contract to lille and netsuke.
    """
    unrecognised = {"runs-on": "ubuntu-20.04"}
    assert "ubuntu-20.04" not in GITHUB_HOSTED_LABELS, (
        "this case only discriminates while the label is absent from the "
        "hosted set; adding it there makes the test vacuous"
    )
    assert billable_labels(unrecognised) == {"ubuntu-20.04"}, (
        "a label that is neither hosted nor reviewed must be reported, so the "
        "registry contract demands it be accounted for rather than ignoring it"
    )
    excused_by_a_prefix_reading = {
        label
        for label in declared_labels(unrecognised)
        if label.startswith(UBICLOUD_LABEL_PREFIX)
    }
    assert not excused_by_a_prefix_reading, (
        "the prefix reading is what this contract deliberately does not use; "
        "if it reports this label too, the two rules have converged and the "
        "choice between them no longer needs defending"
    )
