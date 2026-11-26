"""Contract tests for the one-way dependency rule around ``src/ports``.

The port layer is the boundary the command layer and the adapters both depend
on, so it must depend on neither of them. The shared vocabulary —
``RepairableReason``, ``HostFault``, ``Preflight``, ``PreflightFailure``,
``Decision`` and the classification functions — lives there, and both sides
import it from ``crate::ports``.

These tests read the source tree, because the rule is about which module names
which: a future import that points the arrow the wrong way is exactly the
regression a compile-time check would not catch, since Rust permits the cycle
inside one crate.

A reference can appear in a direct path (``crate::cli::x``) or inside a grouped
import (``use crate::{cli::x, ports::y};``), and a group may nest. The scanner
therefore walks brace depth from each ``crate::{`` rather than matching one
line, so a grouped or wrapped import cannot slip past.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from itertools import accumulate
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]

#: A layer named directly or as a group member.
_DIRECT = re.compile(r"\bcrate::(cli|adapters)\b")
#: The start of a grouped import or path.
_GROUP = re.compile(r"\bcrate::\s*\{")
#: A layer named by a segment inside a group.
#:
#: The group may import the module itself (`use crate::{cli};`), alias it
#: (`use crate::{cli as x};`), or name something inside it (`cli::x`), so the
#: match is the bare word rather than a `::`-terminated path.
_MEMBER = re.compile(r"\b(cli|adapters)\b")


def _code(path: Path) -> str:
    """Return the file's text with `//` comment lines removed."""
    lines = path.read_text().splitlines()
    return "\n".join(
        line for line in lines if not line.lstrip().startswith("//")
    )


def _group_members(text: str, open_brace: int) -> str:
    """Return the text inside the group whose `{` is at ``open_brace``."""
    depths = accumulate(
        1 if character == "{" else -1 if character == "}" else 0
        for character in text[open_brace:]
    )
    end = next((index for index, depth in enumerate(depths) if depth == 0), None)
    if end is None:
        return text[open_brace + 1 :]
    return text[open_brace + 1 : open_brace + end]


def _references(text: str, layers: tuple[str, ...]) -> list[str]:
    """Return the layers this text names directly or as a group member."""
    named = {match.group(1) for match in _DIRECT.finditer(text)}
    for group in _GROUP.finditer(text):
        named.update(
            match.group(1) for match in _MEMBER.finditer(_group_members(text, group.end() - 1))
        )
    return sorted(layer for layer in layers if layer in named)


def _offending(directory: Path, layers: tuple[str, ...]) -> list[str]:
    """Return ``path`` for each file that names one of ``layers``."""
    return [
        str(path.relative_to(ROOT))
        for path in sorted(directory.rglob("*.rs"))
        if _references(_code(path), layers)
    ]


def test_ports_depend_on_neither_the_command_layer_nor_the_adapters() -> None:
    """The port layer names no command or adapter module."""
    offending = _offending(ROOT / "src" / "ports", ("cli", "adapters"))

    assert not offending, f"the port layer must not depend on another layer: {offending}"


def test_adapters_do_not_depend_on_the_command_layer() -> None:
    """The adapters name no command module."""
    offending = _offending(ROOT / "src" / "adapters", ("cli",))

    assert not offending, f"an adapter must not depend on the command layer: {offending}"

@pytest.mark.parametrize(
    ("source", "expected"),
    [
        ("use crate::cli::x;", ["cli"]),
        ("use crate::{cli::x, ports::y};", ["cli"]),
        ("use crate::{cli};", ["cli"]),
        ("use crate::{cli as lifecycle};", ["cli"]),
        ("use crate::{ ports::{cli}, x };", ["cli"]),
        ("crate::adapters::check();", ["adapters"]),
        ("use crate::ports::{HostFault};", []),
    ],
)
def test_grouped_aliased_and_nested_forms_are_caught(
    source: str, expected: list[str]
) -> None:
    """Each import shape names the layers it depends on, or none."""
    assert _references(source, ("cli", "adapters")) == expected
