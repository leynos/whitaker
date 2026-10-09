"""Validate CI provisioning for the canonical Markdown formatter."""

import re
from pathlib import Path
from typing import Any

from ubicloud_workflow_support import parse_workflow

WORKFLOW_PATH: Path = Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"


def _load_workflow() -> dict[str, Any]:
    """Load the CI workflow as a mapping."""
    loaded = parse_workflow(WORKFLOW_PATH.read_text(encoding="utf-8"))
    assert isinstance(loaded, dict), "CI workflow must parse to a mapping"
    return loaded


def test_linux_full_provisions_pinned_markdown_tools_before_checking() -> None:
    """Require verified Markdown tool installations before the format gate."""
    workflow = _load_workflow()
    assert workflow["env"]["MDTABLEFIX_VERSION"] == "0.6.1", (
        "CI must pin the mdtablefix release version"
    )
    assert workflow["env"]["MDTABLEFIX_LINUX_X64_SHA256"] == (
        "e166e2daecd8b8820a521840ee363acfd38af7c53968ec9e65d2223aa0c272e1"
    ), "CI must pin the verified mdtablefix Linux x86_64 checksum"
    # Markdown linting runs through the pinned markdownlint-cli2 action, as
    # the estate's markdown-formatting-baseline rule requires, so CI carries
    # no shell install of the linter.
    assert "MARKDOWNLINT_CLI2_VERSION" not in workflow["env"]
    steps = workflow["jobs"]["linux-full"]["steps"]
    steps_by_name = {step["name"]: step for step in steps if "name" in step}
    step_names = [step["name"] for step in steps if "name" in step]

    assert (
        step_names.index("Restore the Rust toolchain and installed tools")
        < step_names.index("Install bun")
        < step_names.index("Install mdtablefix")
        < step_names.index("Check formatting")
        < step_names.index("Markdown lint")
    ), "CI must cache and install mdtablefix before checking formatting"
    assert "Install Markdown lint CLI" not in step_names
    lint_step = steps_by_name["Markdown lint"]
    assert lint_step["uses"].startswith("DavidAnson/markdownlint-cli2-action@")
    assert lint_step["with"]["globs"] == "**/*.md"

    cache_step = steps_by_name["Restore the Rust toolchain and installed tools"]
    assert cache_step["uses"] == (
        "actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
    )
    # The shared action installs mdtablefix under `~/.local/bin`, and the bun
    # global install for markdownlint-cli2 reuses `~/.bun/install/cache`.
    assert "~/.local/bin" in cache_step["with"]["path"]
    assert "~/.bun/install/cache" in cache_step["with"]["path"]
    assert "~/.cache/mdtablefix-build" not in cache_step["with"]["path"]

    install_step = steps_by_name["Install mdtablefix"]
    assert re.fullmatch(
        r"leynos/shared-actions/\.github/actions/install-mdtablefix@[0-9a-f]{40}",
        install_step["uses"],
    ), "mdtablefix must install through the shared action at a full commit SHA"
    assert install_step["with"]["version"] == "${{ env.MDTABLEFIX_VERSION }}", (
        "the action must install the workflow-level pin"
    )
    assert install_step["with"]["sha256"] == "${{ env.MDTABLEFIX_LINUX_X64_SHA256 }}", (
        "the action must verify the executable against the workflow-level digest"
    )
    assert "run" not in install_step, "mdtablefix must not be installed by a script"
