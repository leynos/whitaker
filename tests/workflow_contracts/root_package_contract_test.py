"""Contract for packaging the root CLI before the publish dry run.

`whitaker` is the workspace's published CLI crate and depends on
`whitaker-installer`, whose matching version reaches crates.io only when the
release publishes it. The step therefore runs the Makefile's
`package-root-cli` target, which points those registry requirements at the
workspace sources for that command only, so the packaged manifest and contents
are validated before publication. It runs in `linux-full` immediately before
the publish dry run, so a packaging regression names the crate that caused it
instead of surfacing as a confusing resolver failure in the dry run.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from ubicloud_workflow_support import REPOSITORY_ROOT, load_job, step_names, steps_by_name

PACKAGE_STEP = "Package the root CLI"
PUBLISH_STEP = "Publish dry run"
PACKAGE_COMMAND = "make package-root-cli"
PACKAGE_TARGET = "package-root-cli"

#: The sibling crates whose versions the release publishes before the root CLI
#: can be packaged against the registry.
PATCHED_CRATES = ("whitaker-installer", "whitaker-common")


def test_the_root_cli_is_packaged_just_before_the_publish_dry_run() -> None:
    """Package the root CLI immediately before the dry run publishes."""
    names = step_names(load_job("linux-full"))
    assert PACKAGE_STEP in names, f"linux-full must run the {PACKAGE_STEP!r} step"

    package_index = names.index(PACKAGE_STEP)
    following = names[package_index + 1 : package_index + 2]
    assert following == [PUBLISH_STEP], (
        f"{PACKAGE_STEP!r} must immediately precede {PUBLISH_STEP!r}"
    )


def _make_recipe(target: str) -> str:
    """Return one Make target's recipe, blank lines included."""
    lines = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8").splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.startswith(f"{target}:")),
        None,
    )
    assert start is not None, f"{target} must be defined in the Makefile"
    body: list[str] = []
    for line in lines[start + 1 :]:
        if line and not line.startswith("\t"):
            break
        body.append(line)
    return "\n".join(body)


def test_the_package_step_uses_the_locked_manifest() -> None:
    """Pin the command, so the check cannot drift to an unlocked run."""
    step = steps_by_name(load_job("linux-full"))[PACKAGE_STEP]
    assert step.get("run") == PACKAGE_COMMAND

    recipe = _make_recipe(PACKAGE_TARGET)
    assert "$(CARGO_LOCKED)" in recipe, "package-root-cli must stay locked"
    for crate in PATCHED_CRATES:
        assert f"patch.crates-io.{crate}.path=" in recipe, (
            f"package-root-cli must patch {crate} to the workspace source"
        )