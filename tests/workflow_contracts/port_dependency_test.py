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

A reference can appear in a direct path (``crate::cli::x``), as a root branch
of a grouped import (``use crate::{cli::x, ports::y};``), or relatively
(``use super::ports::CheckOutcome;``). A group may nest. The scanner therefore
walks brace depth from each ``crate::{`` and reads the leading identifier of
each top-level branch, so a wrapped import cannot slip past and
``use crate::{ports::{cli}};`` is a dependency on ``ports`` rather than on the
``cli`` command layer.

Relative paths are resolved against the file's own module path, because
``super::ports`` means ``crate::ports`` from ``src/library_path/mod.rs`` but
``crate::adapters::ports`` from ``src/adapters/check/preflight.rs`` — so the
same spelling is a leak in one file and not in the other. String literals and
comments are removed first, because prose is not a dependency.

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
#: A relative path prefix: `self`, `super`, or a chain of `super`s.
_RELATIVE = re.compile(r"\b((?:self|super)(?:\s*::\s*super)*)\s*::\s*")
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


def _module_path(path: Path, tree: Path | None = None) -> list[str]:
    """Return the module segments a file occupies, below ``tree``'s ``src``."""
    root = ROOT if tree is None else tree
    parts = path.relative_to(root).with_suffix("").parts
    if not parts or parts[0] != "src":
        return list(parts)
    segments = list(parts[1:])
    # A file or directory named `mod` *is* its parent module.
    if segments and segments[-1] == "mod":
        segments.pop()
    return segments


def _heads(text: str, start: int) -> list[str]:
    """Return the leading names a path or group beginning at ``start`` names."""
    rest = text[start:].lstrip()
    if rest.startswith("{"):
        body = _group_members(rest, 0)
        return [
            root.group(1)
            for branch in _branches(body)
            if (root := _BRANCH_ROOT.match(branch))
        ]
    head = _BRANCH_ROOT.match(rest)
    return [head.group(1)] if head else []


def _relative_targets(source: str, module: list[str]) -> set[str]:
    """Return the layers each relative path in ``source`` resolves into.

    A relative path is resolved against the file's own module, so the same
    spelling names a different layer in a different file. Only the first
    segment of the resolved path matters: that is the module the import
    actually reaches, and a deeper segment named ``cli`` is not the command
    layer.
    """
    text = _code(source)
    reached: set[str] = set()
    for match in _RELATIVE.finditer(text):
        # `self` stays put; every `super` climbs one segment.
        depth = sum(
            1 for segment in match.group(1).split("::") if segment.strip() == "super"
        )
        base = module[: len(module) - depth] if depth else module
        heads = _heads(text, match.end())
        if base:
            # The path lands inside this crate, so the branch heads are its
            # own submodules — not layers.
            continue
        reached.update(heads)
    return reached


def _references(
    source: str, layers: tuple[str, ...], module: list[str] | None = None
) -> list[str]:
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
    if module is not None:
        named.update(_relative_targets(source, module))
    return sorted(layer for layer in layers if layer in named)


def _offending(directory: Path, layers: tuple[str, ...]) -> list[str]:
    """Return ``path`` for each file that names one of ``layers``."""
    return [
        str(path.relative_to(ROOT))
        for path in sorted(directory.rglob("*.rs"))
        if _references(path.read_text(), layers, _module_path(path))
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


@pytest.mark.parametrize(
    ("module", "source", "layers", "expected"),
    [
        # A relative import that reaches a layer from the pure rules.
        ("src/library_path/mod.rs", "use super::ports::X;", ("ports",), ["ports"]),
        (
            "src/library_path/mod.rs",
            "use super::{ports::X, cli::y};",
            ("ports", "cli"),
            ["cli", "ports"],
        ),
        # Inside the port layer, `super::ports` is the port layer itself.
        ("src/ports/mod.rs", "use super::ports::X;", ("cli",), []),
        ("src/ports/mod.rs", "use self::direct_path::X;", ("cli",), []),
        # `super::super::ports` from an adapter is `adapters::ports`, not the
        # port layer; one more `super` does reach the command layer.
        ("src/adapters/check/preflight.rs", "use super::super::ports::X;", ("ports",), []),
        (
            "src/adapters/check/preflight.rs",
            "use super::super::super::cli::X;",
            ("cli",),
            ["cli"],
        ),
    ],
)
def test_relative_paths_resolve_against_the_file_they_appear_in(
    module: str, source: str, layers: tuple[str, ...], expected: list[str]
) -> None:
    """A relative path names the layer it resolves to, not its own spelling."""
    found = _references(source, layers, _module_path(ROOT / module))

    assert found == expected, (
        f"{source!r} in {module} must name {expected}, not {found}"
    )


def test_a_relative_import_of_the_port_layer_is_caught_in_a_real_tree(
    tmp_path: Path,
) -> None:
    """The rule the resolution exists for: a leak written as `super::ports`.

    ``_offending`` is driven over a throwaway tree, so a future refactor that
    keeps the unit cases passing but stops wiring each file's module path into
    the scan still fails here.
    """
    rules = tmp_path / "src" / "library_path"
    rules.mkdir(parents=True)
    (rules / "mod.rs").write_text("use super::ports::CheckOutcome;\n")

    offending = [
        path.name
        for path in sorted(rules.rglob("*.rs"))
        if _references(path.read_text(), ("ports",), _module_path(path, tmp_path))
    ]

    assert offending == ["mod.rs"], (
        "a relative import of the port layer must be caught: " + str(offending)
    )
