"""Hold the suite-revision wiring to the version the lints report.

`no_std_fs_operations` stamps its configuration warnings with
`WHITAKER_SUITE_REVISION`, so a diagnostic can name the build that produced it.
`common/src/i18n` and `crates/no_std_fs_operations/src/config.rs` read the
variable with `option_env!`, which means a build that does not pass it reports
`unknown` forever and no test of the lint can see the difference.

Three build paths must pass it:

* `make publish-check` and `make package-lints`, which build the shipped
  libraries locally, and
* the `Build lint crates` step of `rolling-release.yml`, which builds the
  published artefacts.

Before this test existed the variable was set in all three places and asserted
nowhere, so dropping one assignment would have produced a release whose warnings
all said `unknown` -- a silent provenance loss no Rust test can catch, because
the value is baked in at compile time.

These tests live in `tests/workflow_contracts` because
``make test-workflow-contracts`` runs inside the required ``linux-full`` job.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ

from ubicloud_workflow_support import REPOSITORY_ROOT, load_workflow

MAKEFILE_PATH: typ.Final = REPOSITORY_ROOT / "Makefile"
WORKFLOW_NAME: typ.Final[str] = "rolling-release.yml"
VARIABLE: typ.Final[str] = "WHITAKER_SUITE_REVISION"

_SHORT_SHA_PATTERN: typ.Final[re.Pattern[str]] = re.compile(
    r"git rev-parse --short HEAD"
)
_SHORT_SHA_SUBSTITUTION: typ.Final[re.Pattern[str]] = re.compile(
    r"WHITAKER_SUITE_REVISION=\"\$\$\(git rev-parse --short HEAD\)\""
)
_SHORT_SHA_ASSIGNMENT: typ.Final[re.Pattern[str]] = re.compile(
    r"(?P<name>[A-Z][A-Z0-9_]*)=\$\$\(git rev-parse --short HEAD\)"
)
_VARIABLE_FROM_SHELL_VAR: typ.Final[re.Pattern[str]] = re.compile(
    r"WHITAKER_SUITE_REVISION=\"\$\$(?P<name>[A-Za-z_][A-Za-z0-9_]*)\""
)
_STEP_OUTPUT_REFERENCE: typ.Final[re.Pattern[str]] = re.compile(
    r"steps\.(?P<step>[A-Za-z0-9_-]+)\.outputs\."
)


def _makefile_recipe(target: str) -> str:
    """Return the recipe body of one Makefile target, without its dependencies.

    A recipe runs until the first line that is neither indented nor blank, so a
    following target cannot leak into the text under test.
    """
    source = MAKEFILE_PATH.read_text(encoding="utf-8")
    start = re.search(rf"^{re.escape(target)}:[^\n]*\n", source, re.MULTILINE)
    assert start, f"{MAKEFILE_PATH} has no `{target}` target"
    body = source[start.end() :]
    end = re.search(r"^\S", body, re.MULTILINE)
    return body[: end.start()] if end else body


def test_publish_check_derives_the_revision_from_the_checkout() -> None:
    """`make publish-check` must pass the sha of the tree it clones.

    The target checks out `GIT_TAG`/`WHITAKER_REV` and builds from there, so the
    revision it publishes has to come from the same checkout rather than from
    the caller's repository. The assertion is on the value: the build line must
    take the command substitution itself, so replacing it with a literal fails.
    """
    recipe = _makefile_recipe("publish-check")

    assert _variable_receives_substitution(recipe), (
        f"`make publish-check` must set {VARIABLE} from "
        "`$(git rev-parse --short HEAD)` on the build line itself; a literal "
        "would publish `unknown`-style provenance that never goes stale"
    )


def test_package_lints_derives_the_revision_from_the_checkout() -> None:
    """`make package-lints` must publish the same revision it builds from.

    This recipe assigns `SHA` once and passes `$$SHA`, so the assertion is that
    the build line takes the variable *and* that the variable is assigned from
    the short-sha command earlier in the same recipe.
    """
    recipe = _makefile_recipe("package-lints")
    assigned = _SHORT_SHA_ASSIGNMENT.search(recipe)
    build_match = _VARIABLE_FROM_SHELL_VAR.search(recipe)
    build = _variable_assignment_in_build_line(recipe)

    assert assigned, (
        f"`make package-lints` must assign its revision from "
        "`git rev-parse --short HEAD`"
    )
    assert build, f"`make package-lints` must pass {VARIABLE} on the build line"
    assert build == assigned.group("name"), (
        f"`make package-lints` publishes {VARIABLE} from `${build}`, but only "
        f"`${assigned.group('name')}` is assigned the short sha"
    )
    assert build_match and assigned.start() < build_match.start(), (
        "the revision must be assigned before the build line uses it"
    )


def _variable_receives_substitution(recipe: str) -> bool:
    """Report whether the build line sets the variable to the sha command."""
    return bool(_SHORT_SHA_SUBSTITUTION.search(recipe))


def _variable_assignment_in_build_line(recipe: str) -> str | None:
    """Return the shell variable the build line passes as the revision."""
    match = _VARIABLE_FROM_SHELL_VAR.search(recipe)
    return match.group("name") if match else None


def _job_of(step: dict[str, typ.Any], jobs: dict[str, typ.Any]) -> str | None:
    """Return the job id that owns `step`."""
    for job_id, job in jobs.items():
        if step in (job.get("steps") or []):
            return job_id
    return None


def _producer_step(
    jobs: dict[str, typ.Any], job_id: str, producer_id: str
) -> tuple[int, dict[str, typ.Any]] | None:
    """Return the index and step carrying `producer_id` in `job_id`."""
    return next(
        (
            (position, candidate)
            for position, candidate in enumerate(jobs[job_id].get("steps") or [])
            if candidate.get("id") == producer_id
        ),
        None,
    )


def _check_build_step(
    jobs: dict[str, typ.Any], job_id: str, index: int, step: dict[str, typ.Any]
) -> None:
    """Assert one build step publishes a revision produced earlier in its job."""
    value = (step.get("env") or {}).get(VARIABLE)
    assert value is not None, (
        f"the `Build lint crates` step of {job_id} does not set {VARIABLE}; "
        "published warnings would report `unknown` provenance"
    )
    reference = _STEP_OUTPUT_REFERENCE.search(str(value))
    assert reference, (
        f"the `Build lint crates` step of {job_id} sets {VARIABLE} to "
        f"{value!r}; it must reference a step output so the value is this "
        "build's own commit"
    )

    producer_id = reference.group("step")
    producer = _producer_step(jobs, job_id, producer_id)
    assert producer, (
        f"`{producer_id}` is referenced by the build step of {job_id} but "
        "no step in that job has that id"
    )
    position, producer_step = producer
    assert _SHORT_SHA_PATTERN.search(producer_step.get("run") or ""), (
        f"the `{producer_id}` step of {job_id} does not run "
        "`git rev-parse --short HEAD`"
    )
    assert position < index, (
        f"the `{producer_id}` step of {job_id} must run before the build "
        "that publishes its output"
    )


def test_rolling_release_publishes_the_revision_it_builds() -> None:
    """The release workflow must pass a revision into the lint build step.

    The check is value-based: the build step must take its revision from a
    step's output, that step must run the short-sha command, and it must run
    earlier in the same job. Moving `Determine git SHA` after the build, or
    replacing the reference with a literal, fails.
    """
    jobs = load_workflow(WORKFLOW_NAME).get("jobs") or {}
    builds = [
        (job_id, index, step)
        for job_id, job in jobs.items()
        for index, step in enumerate(job.get("steps") or [])
        if step.get("name") == "Build lint crates"
    ]
    assert builds, f"{WORKFLOW_NAME} has no `Build lint crates` step"

    for job_id, index, step in builds:
        _check_build_step(jobs, job_id, index, step)
