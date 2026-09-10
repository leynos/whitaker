#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = ["pyyaml==6.0.2"]
# ///
"""Reject Agent Skills manifests whose ``metadata`` values are not strings.

``skills_ref.parser`` rewrites every ``metadata`` value with ``str(v)`` rather
than rejecting another shape, so a YAML sequence survives schema validation and
reaches a consumer as a Python repr. ``yamllint`` parses the same frontmatter
but has no rule that sees a value's type, so neither pinned tool reports the
coercion. This checker parses the frontmatter itself and fails when
``metadata`` is not a mapping, or when any of its values is not a string.

The specification permits string values only, so the shapes rejected here are
exactly the ones the schema check cannot see.

Run through uv rather than the ambient interpreter, so the checker selects both
its interpreter and its YAML parser.

Example
-------
    scripts/check_skill_metadata.py skills/addressing-whitaker-findings/
"""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

import yaml

MANIFEST_NAME = "SKILL.md"
FENCE = "---"


class FrontmatterError(Exception):
    """Represent a manifest whose frontmatter cannot be read."""


def frontmatter(manifest: Path) -> str:
    """Return the YAML frontmatter block of a skill manifest.

    The block is the text between the opening fence on the first line and the
    next fence, which is the same region the ``awk`` stage of
    ``skill-frontmatter-lint`` hands to ``yamllint``.

    Parameters
    ----------
    manifest : Path
        Path to a ``SKILL.md`` file.

    Returns
    -------
    str
        The frontmatter text, without either fence.

    Raises
    ------
    FrontmatterError
        If the manifest cannot be read, or has no complete frontmatter block.
    """
    try:
        text = manifest.read_text(encoding="utf-8")
    except OSError as error:
        message = f"cannot read {manifest}: {error.strerror}"
        raise FrontmatterError(message) from error
    lines = text.splitlines()
    if not lines or lines[0] != FENCE:
        message = f"{manifest} does not open with a {FENCE} fence"
        raise FrontmatterError(message)
    try:
        closing = lines.index(FENCE, 1)
    except ValueError:
        message = f"{manifest} has no closing {FENCE} fence"
        raise FrontmatterError(message) from None
    return "\n".join(lines[1:closing])


def metadata_problems(manifest: Path) -> list[str]:
    """Return every reason the manifest's ``metadata`` breaks the contract.

    An absent ``metadata`` key is legitimate: the field is optional, and the
    shipped skill omits it. A key that is present must carry a mapping of
    strings, because anything else reaches a consumer as a coerced value.

    Parameters
    ----------
    manifest : Path
        Path to a ``SKILL.md`` file.

    Returns
    -------
    list[str]
        One message per problem. An empty list means the manifest conforms.
    """
    try:
        document = yaml.safe_load(frontmatter(manifest))
    except FrontmatterError as error:
        return [str(error)]
    except yaml.YAMLError as error:
        return [f"{manifest} frontmatter is not valid YAML: {error}"]
    if not isinstance(document, dict):
        return [f"{manifest} frontmatter must be a mapping"]
    if "metadata" not in document:
        return []
    metadata = document["metadata"]
    if not isinstance(metadata, dict):
        return [f"metadata must be a mapping, got {type(metadata).__name__}"]
    return [
        f"metadata value {key!r} is {type(value).__name__}, not a string"
        for key, value in metadata.items()
        if not isinstance(value, str)
    ]


def parse_arguments(arguments: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse the skill directories to check.

    Parameters
    ----------
    arguments : Sequence[str] | None
        Argument vector, or ``None`` to read ``sys.argv``.

    Returns
    -------
    argparse.Namespace
        Namespace whose ``skill_dirs`` holds one ``Path`` per directory.
    """
    parser = argparse.ArgumentParser(
        description="Reject Skills manifests whose metadata values are not strings."
    )
    parser.add_argument(
        "skill_dirs",
        metavar="SKILL_DIR",
        nargs="+",
        type=Path,
        help="Skill directory holding the SKILL.md to check.",
    )
    return parser.parse_args(arguments)


def main(arguments: Sequence[str] | None = None) -> int:
    """Check every skill directory and report the ones that break the contract.

    Parameters
    ----------
    arguments : Sequence[str] | None
        Argument vector, or ``None`` to read ``sys.argv``.

    Returns
    -------
    int
        ``0`` when every manifest conforms, otherwise ``1``.
    """
    options = parse_arguments(arguments)
    failed = False
    for skill_dir in options.skill_dirs:
        problems = metadata_problems(skill_dir / MANIFEST_NAME)
        if problems:
            failed = True
            print(f"Metadata contract failed for {skill_dir}:", file=sys.stderr)
            for problem in problems:
                print(f"  - {problem}", file=sys.stderr)
        else:
            print(f"Valid metadata: {skill_dir}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
