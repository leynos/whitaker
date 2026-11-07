"""Contract for packaging the root CLI before the publish dry run.

`whitaker` is the workspace's published CLI crate and depends on the
unpublished `whitaker_lint_core`, so `cargo package -p whitaker` is the one
check that the published manifest resolves without the workspace around it. It
runs in `linux-full` immediately before the publish dry run, so a packaging
regression names the crate that caused it instead of surfacing as a confusing
resolver failure in the dry run.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from ubicloud_workflow_support import load_job, step_names, steps_by_name

PACKAGE_STEP = "Package the root CLI"
PUBLISH_STEP = "Publish dry run"
PACKAGE_COMMAND = "cargo package --locked -p whitaker --no-verify"


def test_the_root_cli_is_packaged_just_before_the_publish_dry_run() -> None:
    """Package the root CLI immediately before the dry run publishes."""
    names = step_names(load_job("linux-full"))
    assert PACKAGE_STEP in names, f"linux-full must run the {PACKAGE_STEP!r} step"

    package_index = names.index(PACKAGE_STEP)
    following = names[package_index + 1 : package_index + 2]
    assert following == [PUBLISH_STEP], (
        f"{PACKAGE_STEP!r} must immediately precede {PUBLISH_STEP!r}"
    )


def test_the_package_step_uses_the_locked_manifest() -> None:
    """Pin the command, so the check cannot drift to an unlocked run."""
    step = steps_by_name(load_job("linux-full"))[PACKAGE_STEP]
    assert step.get("run") == PACKAGE_COMMAND