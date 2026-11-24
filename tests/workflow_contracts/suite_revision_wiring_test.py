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
    the caller's repository.
    """
    recipe = _makefile_recipe("publish-check")

    assert VARIABLE in recipe, (
        f"`make publish-check` does not pass {VARIABLE}; every shipped warning "
        "would report `unknown` provenance"
    )
    assert _SHORT_SHA_PATTERN.search(recipe), (
        f"`make publish-check` must derive {VARIABLE} from `git rev-parse --short HEAD` "
        "in the cloned checkout"
    )


def test_package_lints_derives_the_revision_from_the_checkout() -> None:
    """`make package-lints` must publish the same revision it builds from."""
    recipe = _makefile_recipe("package-lints")

    assert VARIABLE in recipe, (
        f"`make package-lints` does not pass {VARIABLE}; the packaged libraries "
        "would report `unknown` provenance"
    )
    assert _SHORT_SHA_PATTERN.search(recipe), (
        f"`make package-lints` must derive {VARIABLE} from `git rev-parse --short HEAD`"
    )


def _workflow_steps() -> list[dict[str, typ.Any]]:
    """Return every step of every job in the release workflow."""
    workflow = load_workflow(WORKFLOW_NAME)
    return [
        step
        for job in (workflow.get("jobs") or {}).values()
        for step in (job.get("steps") or [])
    ]


def _steps_named(steps: list[dict[str, typ.Any]], name: str) -> list[dict[str, typ.Any]]:
    """Return the steps called `name`."""
    return [step for step in steps if step.get("name") == name]


def test_rolling_release_publishes_the_revision_it_builds() -> None:
    """The release workflow must pass a revision into the lint build step."""
    steps = _workflow_steps()

    build_steps = _steps_named(steps, "Build lint crates")
    assert build_steps, f"{WORKFLOW_NAME} has no `Build lint crates` step"
    for step in build_steps:
        env = step.get("env") or {}
        assert VARIABLE in env, (
            f"the `Build lint crates` step of {WORKFLOW_NAME} does not set "
            f"{VARIABLE}; published warnings would report `unknown` provenance"
        )

    # The value must be the checkout's short sha, not a literal.
    short_sha_steps = [
        step
        for step in steps
        if _SHORT_SHA_PATTERN.search(step.get("run") or "")
    ]
    assert short_sha_steps, (
        f"{WORKFLOW_NAME} never runs `git rev-parse --short HEAD`; the revision "
        "it publishes cannot be the build's own commit"
    )
