"""Validate CI provisioning for the canonical Markdown formatter."""

from pathlib import Path
from typing import Any

import yaml

WORKFLOW_PATH: Path = Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"


def _load_workflow() -> dict[str, Any]:
    """Load the CI workflow as a mapping."""
    loaded = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    assert isinstance(loaded, dict), "CI workflow must parse to a mapping"
    return loaded


def test_linux_full_provisions_pinned_markdown_tools_before_checking() -> None:
    """Require verified Markdown tool installations before the format gate."""
    workflow = _load_workflow()
    assert workflow["env"]["MDTABLEFIX_VERSION"] == "0.5.0", (
        "CI must pin the mdtablefix release version"
    )
    assert workflow["env"]["MDTABLEFIX_LINUX_X64_SHA256"] == (
        "bd38cd30f0405120c453b3e80b0d4e78a34d93d2c2121a0fd4ace4a54bacaeeb"
    ), "CI must pin the verified mdtablefix Linux x86_64 checksum"
    assert workflow["env"]["MARKDOWNLINT_CLI2_VERSION"] == "0.20.0", (
        "CI must pin the Markdown lint CLI version"
    )
    steps = workflow["jobs"]["linux-full"]["steps"]
    steps_by_name = {step["name"]: step for step in steps if "name" in step}
    step_names = [step["name"] for step in steps if "name" in step]

    assert (
        step_names.index("Restore the Rust toolchain and installed tools")
        < step_names.index("Install bun")
        < step_names.index("Install mdtablefix")
        < step_names.index("Install Markdown lint CLI")
        < step_names.index("Check formatting")
    ), "CI must cache and install Markdown tools before checking formatting"

    cache_step = steps_by_name["Restore the Rust toolchain and installed tools"]
    assert cache_step["uses"] == (
        "ubicloud/cache/restore@92361f338d82d2c58a98875f1b5c95cd14cd6b2a"
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

    markdownlint_install_script = steps_by_name["Install Markdown lint CLI"]["run"]
    assert (
        'bun install --no-progress --global "markdownlint-cli2@${MARKDOWNLINT_CLI2_VERSION}"'
        in markdownlint_install_script
    )
    assert 'markdownlint_version_output="$(markdownlint-cli2 --version)"' in (
        markdownlint_install_script
    )
    assert (
        "installed_markdownlint_version=\"${markdownlint_version_output%%$'\\n'*}\""
        in (markdownlint_install_script)
    )
    assert (
        'expected_markdownlint_version="markdownlint-cli2 v${MARKDOWNLINT_CLI2_VERSION}"'
        in markdownlint_install_script
    )
