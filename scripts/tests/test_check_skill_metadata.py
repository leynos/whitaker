"""Test the Agent Skills metadata checker.

The checker exists because ``skills-ref`` coerces every ``metadata`` value with
``str(v)``: a YAML sequence passes schema validation and reaches a consumer as
a Python repr, and ``yamllint`` has no rule that sees a value's type. These
tests pin the shapes the checker must reject, so a manifest that the schema
check cannot see fails the lint gate instead of shipping.

Run with ``uv run pytest scripts/tests/test_check_skill_metadata.py``.
"""

from __future__ import annotations

import importlib
import types
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[1]
CONFORMANT = "---\nname: fixture\ndescription: A conformant fixture.\n---\n\n# Fixture\n"


@pytest.fixture
def checker(monkeypatch: pytest.MonkeyPatch) -> types.ModuleType:
    """Import the standalone metadata checker from the scripts directory."""
    monkeypatch.syspath_prepend(str(SCRIPTS))
    importlib.invalidate_caches()
    return importlib.import_module("check_skill_metadata")


def write_manifest(skill_dir: Path, body: str) -> Path:
    """Create a skill directory containing the given manifest text."""
    skill_dir.mkdir()
    (skill_dir / "SKILL.md").write_text(body, encoding="utf-8")
    return skill_dir


def test_a_manifest_may_omit_metadata(tmp_path: Path, checker: types.ModuleType) -> None:
    """The field is optional, and the shipped skill does not set it."""
    manifest = write_manifest(tmp_path / "fixture", CONFORMANT) / "SKILL.md"

    assert checker.metadata_problems(manifest) == []


def test_string_values_conform(tmp_path: Path, checker: types.ModuleType) -> None:
    """A mapping of strings is the shape a consumer can use unchanged."""
    manifest = write_manifest(
        tmp_path / "fixture",
        "---\nname: fixture\ndescription: A conformant fixture.\n"
        "metadata:\n  owner: platform\n---\n\n# Fixture\n",
    ) / "SKILL.md"

    assert checker.metadata_problems(manifest) == []


@pytest.mark.parametrize(
    ("body", "expected"),
    [
        ("metadata:\n  - one\n", "metadata must be a mapping, got list"),
        ("metadata: text\n", "metadata must be a mapping, got str"),
        ("metadata:\n", "metadata must be a mapping, got NoneType"),
    ],
)
def test_non_mapping_metadata_is_rejected(
    tmp_path: Path, checker: types.ModuleType, body: str, expected: str
) -> None:
    """Only a mapping reaches a consumer as a metadata map.

    A key that is present with a null value is not the same as an absent key:
    ``skills-ref`` accepts the document and a consumer receives ``None`` where
    the specification promises a mapping.
    """
    manifest = write_manifest(
        tmp_path / "fixture",
        f"---\nname: fixture\ndescription: A fixture.\n{body}---\n\n# Fixture\n",
    ) / "SKILL.md"

    assert checker.metadata_problems(manifest) == [expected]


@pytest.mark.parametrize(
    ("body", "expected"),
    [
        ("  revision: 3\n", "metadata value 'revision' is int, not a string"),
        ("  enabled: true\n", "metadata value 'enabled' is bool, not a string"),
        ("  tags:\n    - one\n", "metadata value 'tags' is list, not a string"),
    ],
)
def test_non_string_values_are_rejected(
    tmp_path: Path, checker: types.ModuleType, body: str, expected: str
) -> None:
    """A coerced value is the silent failure the checker exists to report.

    ``skills_ref.parser`` rewrites each of these with ``str(v)``, so the
    document passes schema validation while the consumer receives ``'3'``,
    ``'True'``, or ``"['one']"``.
    """
    manifest = write_manifest(
        tmp_path / "fixture",
        "---\nname: fixture\ndescription: A fixture.\n"
        f"metadata:\n{body}---\n\n# Fixture\n",
    ) / "SKILL.md"

    assert checker.metadata_problems(manifest) == [expected]


def test_every_offending_value_is_reported(
    tmp_path: Path, checker: types.ModuleType
) -> None:
    """One report per value, so a single run names every problem."""
    manifest = write_manifest(
        tmp_path / "fixture",
        "---\nname: fixture\ndescription: A fixture.\n"
        "metadata:\n  revision: 3\n  owner: platform\n  tags: [one]\n"
        "---\n\n# Fixture\n",
    ) / "SKILL.md"

    assert checker.metadata_problems(manifest) == [
        "metadata value 'revision' is int, not a string",
        "metadata value 'tags' is list, not a string",
    ]


def test_an_absent_manifest_is_reported(
    tmp_path: Path, checker: types.ModuleType
) -> None:
    """A directory without a manifest fails rather than passing vacuously."""
    problems = checker.metadata_problems(tmp_path / "fixture" / "SKILL.md")

    assert len(problems) == 1
    assert "cannot read" in problems[0]
    assert "No such file or directory" in problems[0]


@pytest.mark.parametrize(
    "body",
    [
        "name: fixture\ndescription: A fixture.\n",
        "---\nname: fixture\ndescription: A fixture.\n",
        "---\n[unclosed\n---\n",
        "---\n- one\n- two\n---\n",
    ],
    ids=["no-fence", "unterminated-fence", "invalid-yaml", "not-a-mapping"],
)
def test_unreadable_frontmatter_is_reported(
    tmp_path: Path, checker: types.ModuleType, body: str
) -> None:
    """Frontmatter the checker cannot read is a problem, not an empty pass."""
    manifest = write_manifest(tmp_path / "fixture", body) / "SKILL.md"

    assert checker.metadata_problems(manifest) != []


def test_main_fails_when_any_directory_breaks_the_contract(
    tmp_path: Path, checker: types.ModuleType, capsys: pytest.CaptureFixture[str]
) -> None:
    """The command fails as soon as one directory of several is nonconformant.

    A target that reported only the last directory would let a conformant
    trailing skill mask the broken one, which is the same masking the other
    skill targets guard against.
    """
    broken = write_manifest(
        tmp_path / "broken",
        "---\nname: broken\ndescription: A fixture.\n"
        "metadata:\n  revision: 3\n---\n\n# Broken\n",
    )
    valid = write_manifest(tmp_path / "valid", CONFORMANT)

    result = checker.main([str(broken), str(valid)])

    captured = capsys.readouterr()
    assert result == 1
    assert f"Metadata contract failed for {broken}" in captured.err
    assert "revision" in captured.err
    assert f"Valid metadata: {valid}" in captured.out


def test_main_succeeds_for_conformant_directories(
    tmp_path: Path, checker: types.ModuleType, capsys: pytest.CaptureFixture[str]
) -> None:
    """The conformant path reports each directory and exits zero."""
    valid = write_manifest(tmp_path / "valid", CONFORMANT)

    result = checker.main([str(valid)])

    captured = capsys.readouterr()
    assert result == 0
    assert captured.err == ""
    assert f"Valid metadata: {valid}" in captured.out


def test_a_directory_is_required(checker: types.ModuleType) -> None:
    """An empty run fails loudly rather than passing without checking."""
    with pytest.raises(SystemExit):
        checker.main([])
