"""Validate CI provisioning for the canonical Markdown formatter."""

import re
from pathlib import Path
from typing import Any

import yaml

WORKFLOW_PATH: Path = Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"


def _load_workflow() -> dict[str, Any]:
    """Load the CI workflow as a mapping."""
    loaded = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    assert isinstance(loaded, dict), "CI workflow must parse to a mapping"
    return loaded


def test_linux_full_provisions_the_pinned_formatter_before_checking() -> None:
    """Require a verified mdtablefix installation before the format gate."""
    workflow = _load_workflow()
    assert workflow["env"]["MDTABLEFIX_VERSION"] == "0.5.0"
    steps = workflow["jobs"]["linux-full"]["steps"]
    steps_by_name = {step["name"]: step for step in steps if "name" in step}
    step_names = [step["name"] for step in steps if "name" in step]

    assert (
        step_names.index("Cache mdtablefix")
        < step_names.index("Install mdtablefix")
        < step_names.index("Check formatting")
    )

    cache_action = steps_by_name["Cache mdtablefix"]["uses"]
    assert re.fullmatch(r"actions/cache@[0-9a-f]{40}", cache_action)
    cache_key = steps_by_name["Cache mdtablefix"]["with"]["key"]
    assert "runner.os" in cache_key
    assert "runner.arch" in cache_key
    assert "env.MDTABLEFIX_VERSION" in cache_key

    install_script = steps_by_name["Install mdtablefix"]["run"]
    assert 'expected_mdtablefix_version="mdtablefix ${MDTABLEFIX_VERSION}"' in (
        install_script
    )
    assert (
        'cargo binstall --no-confirm --locked "mdtablefix@${MDTABLEFIX_VERSION}"'
        in (install_script)
    )
    assert 'cargo install --locked mdtablefix --version "${MDTABLEFIX_VERSION}"' in (
        install_script
    )
    assert "mdtablefix --version 2>/dev/null" in install_script
    assert "installed_mdtablefix_version=\"$(mdtablefix --version | tr -d '\\r')\"" in (
        install_script
    )
