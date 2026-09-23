"""Workflows are parsed strictly: a key declared twice is refused.

PyYAML keeps the last value of a repeated key and says nothing, so a lane
declaring `runs-on` twice parses into a document that has discarded one label,
and every contract reading it judges a file it never saw whole.

Run via ``make test-workflow-contracts``.
"""

import pathlib

import pytest
from ubicloud_workflow_support import (
    WORKFLOWS_DIRECTORY,
    DuplicateKeyError,
    parse_workflow,
)


def test_a_repeated_key_is_refused() -> None:
    """The mutation this loader exists for: `runs-on` declared twice."""
    text = (
        "on: pull_request\njobs:\n  a:\n"
        "    runs-on: ubuntu-latest\n    runs-on: ubicloud-standard-8\n"
        "    steps: []\n"
    )
    with pytest.raises(DuplicateKeyError, match="runs-on"):
        parse_workflow(text)


def test_the_same_key_in_sibling_mappings_is_accepted() -> None:
    """The narrow half: repetition across mappings is ordinary YAML."""
    parsed = parse_workflow("jobs:\n  a:\n    runs-on: x\n  b:\n    runs-on: y\n")
    assert parsed == {"jobs": {"a": {"runs-on": "x"}, "b": {"runs-on": "y"}}}


@pytest.mark.parametrize(
    "path",
    sorted(
        path
        for path in WORKFLOWS_DIRECTORY.iterdir()
        if path.suffix.lower() in (".yml", ".yaml")
    ),
    ids=lambda path: path.name,
)
def test_every_checked_in_workflow_parses_strictly(path: pathlib.Path) -> None:
    """Every contract reads these through the strict loader, so each must load."""
    assert isinstance(parse_workflow(path.read_text(encoding="utf-8")), dict)
