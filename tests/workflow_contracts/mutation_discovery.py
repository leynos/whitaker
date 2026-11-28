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

#: A raw string opener and its hash count.
RAW_STRING: Final[re.Pattern[str]] = re.compile(r'r(#*)?"')

#: A single-character literal, including an escape.
CHAR_LITERAL: Final[re.Pattern[str]] = re.compile(r"'(?:\\.|[^'\\])'", re.DOTALL)


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

    Comments and literals are masked first, so a ``}`` inside a comment,
    string, or raw string cannot change the tracked depth. Braces in code
    delimit inline modules, and each file-backed declaration resolves under
    the inline modules that enclose it.
    """
    base = _module_directory(source, root, roots)
    depth = 0
    inline: list[tuple[int, str]] = []
    for line in _mask_non_code(text).splitlines():
        declaration = MOD_DECLARATION.match(line)
        enclosing = tuple(name for level, name in inline if level <= depth)
        if declaration and declaration.group(2) == ";":
            yield from _resolve(base.joinpath(*enclosing), declaration.group(1))
        elif declaration:
            inline.append((depth + 1, declaration.group(1)))
        depth += line.count("{") - line.count("}")
        while inline and inline[-1][0] > depth:
            inline.pop()


def _mask_non_code(text: str) -> str:
    """Return ``text`` with comments and literals replaced by spaces.

    Braces and ``mod`` declarations inside a comment, string, raw string, or
    character literal must not affect module discovery. Newlines are kept so
    line-oriented matching still works.
    """
    masked = list(text)
    index = 0
    while index < len(text):
        index = _mask_at(text, masked, index)
    return "".join(masked)


def _mask_at(text: str, masked: list[str], index: int) -> int:
    """Mask the comment or literal starting at ``index`` and return its end."""
    if text.startswith("//", index):
        return _mask_span(masked, index, _line_end(text, index))
    if text.startswith("/*", index):
        return _mask_span(masked, index, _block_end(text, index))
    raw = RAW_STRING.match(text, index)
    if raw is not None:
        return _mask_span(masked, index, _raw_end(text, raw))
    if text[index] == '"':
        return _mask_span(masked, index, _string_end(text, index))
    char = CHAR_LITERAL.match(text, index)
    if char is not None:
        return _mask_span(masked, index, char.end())
    return index + 1


def _mask_span(masked: list[str], start: int, end: int) -> int:
    """Replace non-newline characters in ``masked[start:end]`` with spaces."""
    for position in range(start, end):
        if masked[position] != "\n":
            masked[position] = " "
    return end


def _line_end(text: str, start: int) -> int:
    """Return the end of the line comment starting at ``start``."""
    end = text.find("\n", start)
    return len(text) if end < 0 else end


def _block_end(text: str, start: int) -> int:
    """Return the end of a possibly nested block comment."""
    depth = 1
    index = start + 2
    while index < len(text) and depth:
        if text.startswith("/*", index):
            depth += 1
            index += 2
        elif text.startswith("*/", index):
            depth -= 1
            index += 2
        else:
            index += 1
    return index


def _raw_end(text: str, opener: re.Match[str]) -> int:
    """Return the end of the raw string opened by ``opener``."""
    closing = '"' + opener.group(1)
    end = text.find(closing, opener.end())
    return len(text) if end < 0 else end + len(closing)


def _string_end(text: str, start: int) -> int:
    """Return the end of the string literal starting at ``start``."""
    index = start + 1
    while index < len(text):
        if text[index] == "\\":
            index += 2
        elif text[index] == '"':
            return index + 1
        else:
            index += 1
    return len(text)


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