"""Contract tests for the one-way dependency rule around the port layer.

The port layer is the boundary the command layer and the adapters both depend
on, so it must depend on neither of them, and it must not depend on the pure
library-path rules either: the value a direct run carries is a port type
(``src/ports/direct_path.rs``), and the adapter records the classification
that approved it. ``src/library_path`` is a leaf of domain rules, so it names
nothing above it. The shared vocabulary — ``RepairableReason``, ``HostFault``,
``Preflight``, ``PreflightFailure``, ``Decision`` and the classification
functions — lives in the port layer, and both sides import it from
``crate::ports``.

These tests read the source tree, because the rule is about which module names
which: a future import that points the arrow the wrong way is exactly the
regression a compile-time check would not catch, since Rust permits the cycle
inside one crate.

A reference can appear in a direct path (``crate::cli::x``) or as a root branch
of a grouped import (``use crate::{cli::x, ports::y};``), and a group may nest.
The scanner therefore walks brace depth from each ``crate::{`` and reads the
leading identifier of each top-level branch, so a wrapped import cannot slip
past and ``use crate::{ports::{cli}};`` is a dependency on ``ports`` rather
than on the ``cli`` command layer. String literals and comments are removed
first, because prose is not a dependency.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from itertools import accumulate
from pathlib import Path
from typing import Iterator

import pytest

ROOT = Path(__file__).resolve().parents[2]

#: The start of a grouped import or path.
_GROUP = re.compile(r"\bcrate\s*::\s*\{")
#: The leading identifier of a branch, which is the module the branch starts
#: from. An alias (`cli as x`) or a deeper segment (`ports::cli`) therefore
#: contributes only its first name, so an aliased import is still a dependency
#: while a submodule that merely happens to be called `cli` is not.
_BRANCH_ROOT = re.compile(r"\s*([A-Za-z_][A-Za-z0-9_]*)")
#: A string literal, whose text is prose or data rather than code.
_STRING = re.compile(r'"(?:\\.|[^"\\])*"')
#: A line comment, including a doc comment.
_COMMENT = re.compile(r"//[^\n]*")


def _code(source: str) -> str:
    """Return the source's code, with string literals emptied and comments gone.

    Strings are emptied before comments are removed so a `//` inside a literal
    cannot swallow the rest of a line.
    """
    return _COMMENT.sub("", _STRING.sub('""', source))


def _direct_pattern(layers: tuple[str, ...]) -> re.Pattern[str]:
    """Return the pattern for a layer named directly by a `crate::` path.

    Rust permits whitespace around `::`, so `crate :: cli :: x` is the same
    dependency as `crate::cli::x`. The layer set is a parameter because each
    caller forbids a different one.
    """
    alternation = "|".join(re.escape(layer) for layer in layers)
    return re.compile(rf"\bcrate\s*::\s*({alternation})\b")


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


def _branches(body: str) -> Iterator[str]:
    """Yield each top-level comma-separated branch of a group body."""
    depth = 0
    start = 0
    for index, character in enumerate(body):
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
        elif character == "," and depth == 0:
            yield body[start:index]
            start = index + 1
    yield body[start:]


def _references(source: str, layers: tuple[str, ...]) -> list[str]:
    """Return the layers this source names directly or as a branch root."""
    text = _code(source)
    named = {match.group(1) for match in _direct_pattern(layers).finditer(text)}
    for group in _GROUP.finditer(text):
        body = _group_members(text, group.end() - 1)
        named.update(
            root.group(1)
            for branch in _branches(body)
            if (root := _BRANCH_ROOT.match(branch))
        )
    return sorted(layer for layer in layers if layer in named)


def _offending(directory: Path, layers: tuple[str, ...]) -> list[str]:
    """Return ``path`` for each file that names one of ``layers``."""
    return [
        str(path.relative_to(ROOT))
        for path in sorted(directory.rglob("*.rs"))
        if _references(path.read_text(), layers)
    ]


def test_ports_depend_on_neither_the_command_layer_nor_the_adapters() -> None:
    """The port layer names no command or adapter module."""
    offending = _offending(ROOT / "src" / "ports", ("cli", "adapters"))

    assert not offending, f"the port layer must not depend on another layer: {offending}"


def test_adapters_do_not_depend_on_the_command_layer() -> None:
    """The adapters name no command module."""
    offending = _offending(ROOT / "src" / "adapters", ("cli",))

    assert not offending, f"an adapter must not depend on the command layer: {offending}"


def test_ports_do_not_depend_on_the_library_path_rules() -> None:
    """The port layer names no library-path rule module."""
    offending = _offending(ROOT / "src" / "ports", ("library_path",))

    assert not offending, (
        f"the port layer must not depend on the library-path rules: {offending}"
    )


def test_library_path_depends_on_no_layer_above_it() -> None:
    """The pure library-path rules name no command, adapter or port module."""
    offending = _offending(ROOT / "src" / "library_path", ("ports", "cli", "adapters"))

    assert not offending, (
        f"the library-path rules are a leaf and must name no layer above them: {offending}"
    )


def test_an_adapter_owns_the_classification_it_records() -> None:
    """The port's direct value is built only by the adapter that judged it.

    ``DirectLibraryPath`` is a port type precisely so the pure rules can refuse
    a path without importing the port's error vocabulary. Its constructor is
    crate-visible, and this pins the only call site to the adapter.
    """
    users = [
        str(path.relative_to(ROOT))
        for path in sorted((ROOT / "src").rglob("*.rs"))
        if "DirectLibraryPath::from_approved" in path.read_text()
    ]

    assert users == ["src/adapters/check/preflight.rs"], (
        "only the adapter that listed the path may record the approval: " + str(users)
    )

@pytest.mark.parametrize(
    ("source", "expected"),
    [
        ("use crate::cli::x;", ["cli"]),
        ("use crate :: cli :: x;", ["cli"]),
        ("use crate::{cli::x, ports::y};", ["cli"]),
        ("use crate::{cli};", ["cli"]),
        ("use crate::{cli as lifecycle};", ["cli"]),
        ("use crate::{ ports::{cli}, x };", []),
        ("use crate::{ports::cli::x};", []),
        ("use crate::{ports as cli};", []),
        ("crate::adapters::check();", ["adapters"]),
        ("use crate::ports::{HostFault};", []),
        ('const DOC: &str = "use crate::cli::x;";', []),
        ("// use crate::cli::x;", []),
        ("use crate::ports::y; // crate::cli for now", []),
    ],
)
def test_grouped_aliased_and_nested_forms_are_caught(
    source: str, expected: list[str]
) -> None:
    """Each import shape names the layers it depends on, or none."""
    found = _references(source, ("cli", "adapters"))

    assert found == expected, f"{source!r} must name {expected}, not {found}"
