"""Pure naming and parsing rules for the prune-safety checker.

The checker's filesystem and process adapters live in
``check_prune_safety.py``. These functions decide what a library file name
means and which crates survive the oldest installer's prune, so they can be
tested without archives or subprocesses. Keeping them separate holds each
module below the repository's file-size limit.
"""

from __future__ import annotations

import re
from collections.abc import Sequence

SUITE_CRATE = "whitaker_suite"
DEFAULT_TARGET = "x86_64-unknown-linux-gnu"


class PruneSafetyError(Exception):
    """Represent a failure to establish or check the prune contract."""


def _constant_pattern(name: str) -> re.Pattern[str]:
    """Return a pattern capturing the body of ``pub const <name>: &[&str]``."""
    return re.compile(
        rf"pub(?:\(crate\))?\s+const\s+{name}\s*:\s*&\[\s*&str\s*\]\s*=\s*&\[(.*?)\];",
        re.DOTALL,
    )


_LINT_CRATES_PATTERN = _constant_pattern("LINT_CRATES")
_EXPERIMENTAL_PATTERN = _constant_pattern("EXPERIMENTAL_LINT_CRATES")


def parse_lint_crates(source: str) -> list[str]:
    """Return every crate name the installer's prune knows by name.

    The prune removes each crate in ``LINT_CRATES`` and, when the installer
    has one, ``EXPERIMENTAL_LINT_CRATES`` that was not requested.

    >>> parse_lint_crates('pub const LINT_CRATES: &[&str] = &["a", "b"];')
    ['a', 'b']
    >>> parse_lint_crates(
    ...     'pub const LINT_CRATES: &[&str] = &["a"];'
    ...     'pub const EXPERIMENTAL_LINT_CRATES: &[&str] = &["x"];'
    ... )
    ['a', 'x']
    """
    match = _LINT_CRATES_PATTERN.search(source)
    if match is None:
        raise PruneSafetyError("no LINT_CRATES constant found in the installer source")
    crates = re.findall(r'"([^"]+)"', match.group(1))
    if not crates:
        raise PruneSafetyError("the installer's LINT_CRATES is empty")
    experimental = _EXPERIMENTAL_PATTERN.search(source)
    if experimental is not None:
        crates.extend(re.findall(r'"([^"]+)"', experimental.group(1)))
    return crates


_ARCHIVE_NAME = re.compile(
    r"^whitaker-lints-(?P<sha>[0-9a-f]+)-(?P<toolchain>.+?)-"
    r"(?P<target>(?:x86_64|aarch64|i686|armv7)-[A-Za-z0-9_.-]+)\.tar\.zst$"
)


def parse_archive_name(name: str) -> dict[str, str]:
    """Split ``whitaker-lints-<sha>-<toolchain>-<target>.tar.zst`` into parts.

    >>> parts = parse_archive_name(
    ...     "whitaker-lints-abc1234-nightly-2026-05-28-x86_64-unknown-linux-gnu.tar.zst"
    ... )
    >>> parts["sha"], parts["toolchain"], parts["target"]
    ('abc1234', 'nightly-2026-05-28', 'x86_64-unknown-linux-gnu')
    """
    match = _ARCHIVE_NAME.match(name)
    if match is None:
        raise PruneSafetyError(
            f"{name} does not follow the whitaker-lints archive naming"
        )
    return match.groupdict()


def library_affixes(target: str) -> tuple[str, str]:
    """Return the library file ``(prefix, extension)`` the installer expects.

    Mirrors the installer's target rules: Windows has no prefix and ``.dll``,
    macOS ``lib`` and ``.dylib``, everything else ``lib`` and ``.so``.

    >>> library_affixes("x86_64-pc-windows-msvc")
    ('', '.dll')
    >>> library_affixes("aarch64-apple-darwin")
    ('lib', '.dylib')
    >>> library_affixes("x86_64-unknown-linux-gnu")
    ('lib', '.so')
    """
    if "windows" in target:
        return "", ".dll"
    if "darwin" in target:
        return "lib", ".dylib"
    return "lib", ".so"


def crate_of_entry(entry: str, target: str = DEFAULT_TARGET) -> str:
    """Return the crate behind a library file name for ``target``.

    The installer stages and prunes ``<prefix><crate><extension>`` with the
    target's own prefix and extension. A name that does not have exactly that
    shape (a missing ``lib`` on Linux, a ``.so`` on Windows, extra dots, an
    ``@toolchain`` suffix, another extension) is returned whole: the installer
    would not prune it, and it is not the suite either.

    >>> crate_of_entry("libbumpy_road_function.so")
    'bumpy_road_function'
    >>> crate_of_entry("whitaker_suite.dll", "x86_64-pc-windows-msvc")
    'whitaker_suite'
    >>> crate_of_entry("libmodule_max_lines.dll", "x86_64-pc-windows-msvc")
    'libmodule_max_lines'
    >>> crate_of_entry("module_max_lines.so")
    'module_max_lines.so'
    >>> crate_of_entry("whitaker_suite.so")
    'whitaker_suite.so'
    >>> crate_of_entry("libmodule_max_lines.extra.so")
    'libmodule_max_lines.extra.so'
    >>> crate_of_entry("libfoo@nightly-2026-05-28.so")
    'libfoo@nightly-2026-05-28.so'
    >>> crate_of_entry("libfoo.dylib")
    'libfoo.dylib'
    """
    name = entry.rsplit("/", 1)[-1]
    prefix, extension = library_affixes(target)
    if not (name.startswith(prefix) and name.endswith(extension)):
        return name
    base = name.removeprefix(prefix).removesuffix(extension)
    return base if re.fullmatch(r"[A-Za-z0-9_-]+", base) else name


def surviving_crates(
    entries: Sequence[str], known: Sequence[str], target: str = DEFAULT_TARGET
) -> list[str]:
    """Return the crates left after a suite install prunes the ``known`` ones.

    >>> surviving_crates(["liba.so", "libb.so", "libwhitaker_suite.so"], ["a"])
    ['b', 'whitaker_suite']
    """
    crates = {crate_of_entry(entry, target) for entry in entries}
    return sorted(crates - set(known))
