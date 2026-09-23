"""Validate CI provisioning for the canonical Markdown formatter."""

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
    assert workflow["env"]["MDTABLEFIX_VERSION"] == "0.6.0", (
        "CI must pin the mdtablefix release version"
    )
    assert workflow["env"]["MDTABLEFIX_LINUX_X64_SHA256"] == (
        "b78b2ac9b396b71073ff0485d9d37717cdcd82893b660b7146ba6d3f70d43ba0"
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
    # The verified mdtablefix release lands in `~/.cargo/bin`, and the bun
    # global install for markdownlint-cli2 reuses `~/.bun/install/cache`.
    assert "~/.cargo/bin" in cache_step["with"]["path"]
    assert "~/.bun/install/cache" in cache_step["with"]["path"]
    assert "~/.cache/mdtablefix-build" not in cache_step["with"]["path"]

    install_script = steps_by_name["Install mdtablefix"]["run"]
    assert 'expected_mdtablefix_version="mdtablefix ${MDTABLEFIX_VERSION}"' in (
        install_script
    )
    assert "releases/download/v${MDTABLEFIX_VERSION}/mdtablefix-linux-x86_64" in (
        install_script
    )
    assert "${MDTABLEFIX_LINUX_X64_SHA256}" in install_script
    assert "sha256sum --check --status" in install_script
    assert 'install -m 0755 "${download}" "${destination}.new"' in install_script
    assert 'mv "${destination}.new" "${destination}"' in install_script
    assert "cargo binstall" not in install_script
    assert "cargo install" not in install_script
    assert "mdtablefix --version 2>/dev/null" in install_script
    assert "installed_mdtablefix_version=\"$(mdtablefix --version | tr -d '\\r')\"" in (
        install_script
    )
