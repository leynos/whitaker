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
_RELATIVE = re.compile(r"\b((?:self|super)(?:\s*::\s*super)*)\s*::\s*(?=\S)")
#: An inline module's name, which adds a segment between a file and the
#: references inside it: `mod inner { ... }` gives its body the path
#: `<file module>::inner`, so `super` from there climbs to the file module.
_INLINE_MODULE = re.compile(r"\bmod\s+([A-Za-z_][A-Za-z0-9_]*)")
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


def _paths_after(text: str, start: int) -> tuple[list[tuple[int, str]], int]:
    """Return the relative paths beginning at ``start`` and where they end.

    Rust composes a branch's own prefix with its parent's, so ``use
    super::{super::cli::X}`` climbs two segments, and a branch may open another
    group. The depth is the total number of ``super`` segments and the head is
    the first name after them. The second result is the offset just past the
    construct, so a caller does not read the same group twice.
    """
    depth = 0
    index = start
    while True:
        while index < len(text) and text[index].isspace():
            index += 1
        prefix = _RELATIVE.match(text, index)
        if prefix is None:
            break
        depth += sum(
            1 for segment in prefix.group(1).split("::") if segment.strip() == "super"
        )
        index = prefix.end()
    if index < len(text) and text[index] == "{":
        body = _group_members(text, index)
        found: list[tuple[int, str]] = []
        for branch in _branches(body):
            nested, _ = _paths_after(branch, 0)
            found.extend((depth + climb, head) for climb, head in nested)
        return found, index + len(body) + 2
    head = _BRANCH_ROOT.match(text, index)
    if head is None:
        return [], index
    return [(depth, head.group(1))], head.end()


def _inline_scopes(text: str) -> list[tuple[int, str]]:
    """Return each inline module's body start and the segment it adds."""
    scopes = []
    for match in _INLINE_MODULE.finditer(text):
        opening = text.find("{", match.end())
        if opening != -1:
            scopes.append((opening, match.group(1)))
    return scopes


def _enclosing_module(text: str, offset: int, module: list[str]) -> list[str]:
    """Return the module path in force at ``offset``, inline modules included."""
    enclosing = list(module)
    for opening, segment in _inline_scopes(text):
        if opening < offset < opening + len(_group_members(text, opening)) + 1:
            enclosing.append(segment)
    return enclosing


def _relative_targets(source: str, module: list[str]) -> set[str]:
    """Return the layers each relative path in ``source`` resolves into.

    A relative path is resolved against the module it appears in, so the same
    spelling names a different layer in a different file — and in a different
    inline module of one file. Only the head of the resolved path matters:
    that is the module the import actually reaches, and a deeper segment named
    ``cli`` is not the command layer.
    """
    text = _code(source)
    reached: set[str] = set()
    consumed = 0
    for match in _RELATIVE.finditer(text):
        # A group's branches are read by the outer prefix; reading them again
        # here would resolve them without that prefix.
        if match.start() < consumed:
            continue
        paths, consumed = _paths_after(text, match.start())
        # Every branch of one group starts from the same enclosing module.
        scope = _enclosing_module(text, match.start(), module)
        for depth, head in paths:
            base = scope[: len(scope) - depth] if depth else scope
            if base:
                # The path lands inside this crate, so the head names one of
                # its own modules — not a layer.
                continue
            reached.add(head)
    return reached


def _references(
    source: str, layers: tuple[str, ...], module: list[str] | None = None
) -> list[str]:
    """Return the layers this source names directly or as a branch root."""
    text = _code(source)
    named = {match.group(1) for match in _direct_pattern(layers).finditer(text)}
    for group in _GROUP.finditer(text):
        body = _group_members(text, group.end() - 1)
        # `crate::{...}` names the layer at the start of each branch. A branch
        # that itself climbs with `super` reaches outside the crate, which
        # `_relative_targets` decides, so it is skipped here.
        for branch in _branches(body):
            paths, _ = _paths_after(branch, 0)
            named.update(head for depth, head in paths if depth == 0)
    if module is not None:
        named.update(_relative_targets(source, module))
    return sorted(layer for layer in layers if layer in named)


def _offending(
    directory: Path, layers: tuple[str, ...], tree: Path | None = None
) -> list[str]:
    """Return ``path`` for each file that names one of ``layers``.

    ``tree`` is the checkout the module paths are resolved against, so the same
    scan can be driven over a throwaway tree without losing the wiring this
    function exists to prove.
    """
    root = ROOT if tree is None else tree
    return [
        str(path.relative_to(root))
        for path in sorted(directory.rglob("*.rs"))
        if _references(path.read_text(), layers, _module_path(path, root))
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
        # A branch composes its own prefix with the group's: two `super`s
        # reach the crate root from `ports::classify` either way.
        ("src/ports/classify.rs", "use super::{super::cli::X};", ("cli",), ["cli"]),
        ("src/ports/classify.rs", "use super::super::cli::X;", ("cli",), ["cli"]),
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
    benign = rules / "rules.rs"
    benign.write_text("use crate::library_path::classify;\n")

    offending = _offending(rules, ("ports",), tmp_path)

    assert offending == ["src/library_path/mod.rs"], (
        "the scan must catch the relative leak and spare the benign file: "
        + str(offending)
    )


def test_a_reference_inside_an_inline_module_resolves_from_that_module(
    tmp_path: Path,
) -> None:
    """An inline module adds a segment, so a leak written inside one is caught.

    ``super`` inside ``mod tests`` climbs from the inline module, not from the
    file, so the same spelling resolves the same way one level up. A scanner
    that ignored inline modules would resolve the path one segment short and
    miss the reference.
    """
    rules = tmp_path / "src" / "ports"
    rules.mkdir(parents=True)
    nested = rules / "classify.rs"
    nested.write_text(
        "mod tests {\n"
        "    use super::super::super::cli::X;\n"
        "}\n"
    )

    offending = _offending(rules, ("cli",), tmp_path)

    assert offending == ["src/ports/classify.rs"], (
        "the scan must resolve a reference from its inline module: " + str(offending)
    )
