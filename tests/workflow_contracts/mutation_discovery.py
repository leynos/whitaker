"""Discover every module file reachable from a package's target roots.

Cargo-mutants enumerates module files that are not themselves targets, and
``#[path]`` attributes can place a module outside its package's ``src`` tree.
The scope guard therefore has to follow declarations the way the compiler
does: from each declared target root, through ``#[path]`` attributes, file
modules, and inline modules.

This module is test support only; no production code depends on it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from collections.abc import Iterable, Iterator
from pathlib import Path
from typing import Final

#: A module declaration that places its file outside the usual tree.
PATH_ATTRIBUTE: Final[re.Pattern[str]] = re.compile(
    r'#\[path\s*=\s*"([^"]+)"\]'
)

#: A file-backed or inline module declaration at the start of a line.
MOD_DECLARATION: Final[re.Pattern[str]] = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*([;{])"
)


def path_attributed_sources(
    initial: Iterable[str], root: Path, roots: Iterable[str] = ()
) -> tuple[str, ...]:
    """Return ``initial`` plus modules reached from those files.

    Follow each ``#[path]`` attribute, file-backed ``mod child;``
    declaration, and inline ``mod child { ... }`` transitively. A declared
    target source is a crate root, so its child modules resolve beside it
    rather than under a directory named after the file.

    Parameters
    ----------
    initial : Iterable[str]
        Repository-relative source paths to start from.
    root : Path
        Repository root.
    roots : Iterable[str], optional
        Repository-relative sources that are declared Cargo targets. Their
        child modules use crate-root resolution.

    Returns
    -------
    tuple[str, ...]
        Sorted repository-relative paths, including every reachable module
        that exists under ``root``.
    """
    seen = set(initial)
    queue = list(seen)
    root_sources = set(roots)
    while queue:
        for target in _module_targets(root / queue.pop(), root, root_sources):
            _record_source(target, root, seen, queue)
    return tuple(sorted(seen))


def _module_targets(source: Path, root: Path, roots: set[str]) -> Iterator[Path]:
    """Yield every existing module file the source declares."""
    try:
        text = source.read_text(encoding="utf-8")
    except OSError:
        return
    yield from _path_targets(source, text)
    yield from _file_targets(source, text, root, roots)


def _path_targets(source: Path, text: str) -> Iterator[Path]:
    """Yield existing ``#[path = "..."]`` module targets."""
    for match in PATH_ATTRIBUTE.finditer(text):
        target = (source.parent / match.group(1)).resolve()
        if target.is_file():
            yield target


def _file_targets(
    source: Path, text: str, root: Path, roots: set[str]
) -> Iterator[Path]:
    """Yield existing file-backed modules, honouring inline module nesting.

    Rustfmt indents an inline module's body one level deeper than its
    declaration, so indentation identifies the enclosing module without
    counting braces. Brace counting would be fooled by a ``}`` inside a
    comment or string literal.
    """
    base = _module_directory(source, root, roots)
    inline: list[tuple[int, str]] = []
    for line in text.splitlines():
        indent = len(line) - len(line.lstrip())
        while inline and indent <= inline[-1][0]:
            inline.pop()
        declaration = MOD_DECLARATION.match(line)
        if declaration is None:
            continue
        if declaration.group(2) == ";":
            enclosing = tuple(name for _, name in inline)
            yield from _resolve(base.joinpath(*enclosing), declaration.group(1))
        else:
            inline.append((indent, declaration.group(1)))


def _resolve(base: Path, name: str) -> Iterator[Path]:
    """Yield the existing file for one file-backed module declaration."""
    candidates = (base / f"{name}.rs", base / name / "mod.rs")
    target = next((candidate for candidate in candidates if candidate.is_file()), None)
    if target is not None:
        yield target


def _module_directory(source: Path, root: Path, roots: set[str]) -> Path:
    """Return the directory a file-backed child module resolves against."""
    try:
        relative = source.resolve().relative_to(root).as_posix()
    except ValueError:
        relative = ""
    if relative in roots or source.name == "mod.rs":
        return source.parent
    return source.parent / source.stem


def _record_source(target: Path, root: Path, seen: set[str], queue: list[str]) -> None:
    """Record a discovered source under ``root`` and queue it for scanning."""
    try:
        discovered = target.resolve().relative_to(root).as_posix()
    except ValueError:
        return
    if discovered not in seen:
        seen.add(discovered)
        queue.append(discovered)