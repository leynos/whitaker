"""Inventory the workspace sources a mutation run can enumerate.

The scheduled mutation caller scopes change detection through ``paths`` and
mutates every package through ``--workspace``. Those two settings must agree
on the workspace: a member outside ``paths`` schedules no run, and a member
outside both ``paths`` and ``exclude-globs`` is unreachable once a run does
start. Cargo also promotes a member's path dependencies to members
implicitly, so the root manifest's explicit ``members`` list is not the whole
workspace.

This module asks Cargo for the effective member list and each package's
targets, then reports sources that neither a path prefix nor an exclusion
covers. It is test support only; no production code depends on it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import dataclasses
import fnmatch
import json
import re
import subprocess
from collections.abc import Iterable, Iterator
from pathlib import Path
from typing import Final

#: The command that returns every workspace package without resolving deps.
CARGO_METADATA: Final[tuple[str, ...]] = (
    "cargo",
    "metadata",
    "--no-deps",
    "--format-version",
    "1",
    "--locked",
)

#: Target kinds whose sources cargo-mutants can mutate. Integration tests,
#: benches, and examples are excluded: the tool does not mutate them, and
#: the root package keeps its end-to-end tests under top-level ``tests/``.
#: A module declaration that places its file outside the usual tree.
PATH_ATTRIBUTE: Final[re.Pattern[str]] = re.compile(
    r'#\[path\s*=\s*"([^"]+)"\]'
)

#: An ordinary file-backed module declaration.
MOD_DECLARATION: Final[re.Pattern[str]] = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;",
    re.MULTILINE,
)

#: Mutant-name filter for Kani-only items in files that also hold production
#: code. Path exclusions cannot remove them without losing the compiled code,
#: so cargo-mutants filters them by the qualified name or source line it
#: prints. The ``minhash`` clause covers the unrolled ``cfg(kani)``
#: ``sketch_values`` function (lines 127-259), and the ``lsh`` clause covers
#: the ``cfg(kani)`` block inside ``LshIndex::insert`` (lines 95-108). Contract
#: tests pin both bounds so line drift fails rather than silently dropping
#: production mutants.
KANI_ITEM_EXCLUSION: Final[str] = (
    r"(_for_kani|ForKani|verification::"
    r"|minhash\.rs:(12[7-9]|1[3-9][0-9]|2[0-4][0-9]|25[0-9]):"
    r"|lsh\.rs:(9[5-9]|10[0-8]):)"
)

MUTABLE_TARGET_KINDS: Final[frozenset[str]] = frozenset(
    {
        "bin",
        "cdylib",
        "dylib",
        "lib",
        "proc-macro",
        "rlib",
        "staticlib",
    }
)


@dataclasses.dataclass(frozen=True, slots=True)
class PackageScope:
    """One workspace package and the sources cargo-mutants can inspect.

    Attributes
    ----------
    name : str
        The package name from its manifest.
    directory : str
        Repository-relative directory holding the package manifest.
    sources : tuple[str, ...]
        Repository-relative source files: every declared target plus the
        package's ``src`` tree, because cargo-mutants enumerates module files
        that are not themselves targets.
    """

    name: str
    directory: str
    sources: tuple[str, ...]


def workspace_packages(root: Path) -> tuple[PackageScope, ...]:
    """Return every effective workspace member and its mutable sources.

    Parameters
    ----------
    root : Path
        Repository root containing the workspace manifest.

    Returns
    -------
    tuple[PackageScope, ...]
        Packages ordered by their repository-relative directory.

    Raises
    ------
    RuntimeError
        If ``cargo metadata`` fails or returns an unexpected shape.
    """
    metadata = _cargo_metadata(root)
    packages = {package["id"]: package for package in metadata["packages"]}
    scopes: list[PackageScope] = []
    for package_id in metadata["workspace_members"]:
        package = packages[package_id]
        manifest = Path(package["manifest_path"]).parent
        directory = manifest.relative_to(root).as_posix()
        scopes.append(
            PackageScope(
                name=str(package["name"]),
                directory=directory,
                sources=_package_sources(package, manifest, root),
            )
        )
    assert scopes, "cargo metadata returned no workspace members"
    return tuple(sorted(scopes, key=lambda scope: scope.directory))


def uncovered_sources(
    packages: tuple[PackageScope, ...],
    prefixes: tuple[str, ...],
    globs: tuple[str, ...],
) -> list[str]:
    """Return sources neither a path prefix nor an exclusion covers.

    A source under ``paths`` is visible to scheduled change detection. A
    source matched by ``exclude-globs`` is deliberately out of mutation
    scope. Anything else can schedule nothing or enumerate nothing, which is
    the silent outcome this guard exists to prevent.

    Parameters
    ----------
    packages : tuple[PackageScope, ...]
        Workspace packages and their sources.
    prefixes : tuple[str, ...]
        Comma-split ``paths`` entries, without surrounding whitespace.
    globs : tuple[str, ...]
        Comma-split ``exclude-globs`` entries, without whitespace.

    Returns
    -------
    list[str]
        Repository-relative sources that are neither detected nor excluded.
    """
    return [
        source
        for package in packages
        for source in package.sources
        if not any(is_under(source, prefix) for prefix in prefixes)
        and not is_excluded(source, globs)
    ]


def is_under(source: str, prefix: str) -> bool:
    """Return True when ``source`` is the prefix directory or beneath it."""
    base = prefix.rstrip("/")
    return source == base or source.startswith(f"{base}/")


def is_excluded(source: str, globs: tuple[str, ...]) -> bool:
    """Return True when an exclusion glob matches the source path."""
    return any(fnmatch.fnmatchcase(source, glob) for glob in globs)


def _cargo_metadata(root: Path) -> dict[str, object]:
    """Run ``cargo metadata`` and return its parsed JSON."""
    result = subprocess.run(
        CARGO_METADATA,
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            "cargo metadata failed: "
            f"exit {result.returncode}; stderr: {result.stderr.strip()}"
        )
    metadata = json.loads(result.stdout)
    assert isinstance(metadata, dict), "cargo metadata returned non-object JSON"
    return metadata


def _package_sources(
    package: dict[str, object], manifest: Path, root: Path
) -> tuple[str, ...]:
    """Return a package's declared targets plus every file under ``src``."""
    sources: set[str] = set()
    roots: set[str] = set()
    targets = package.get("targets")
    assert isinstance(targets, list), f"{package['name']} declares no targets"
    for target in targets:
        assert isinstance(target, dict), "a cargo target is not an object"
        kinds = target.get("kind")
        assert isinstance(kinds, list), "a cargo target declares no kind"
        if not MUTABLE_TARGET_KINDS.intersection(kinds):
            continue
        source = Path(str(target["src_path"]))
        if source.is_file():
            relative = source.relative_to(root).as_posix()
            roots.add(relative)
            sources.add(relative)
    for source in (manifest / "src").rglob("*.rs"):
        sources.add(source.relative_to(root).as_posix())
    assert sources, f"{package['name']} has no source files"
    return path_attributed_sources(sources, root, roots)


def path_attributed_sources(
    initial: Iterable[str], root: Path, roots: Iterable[str] = ()
) -> tuple[str, ...]:
    """Return ``initial`` plus modules reached from those files.

    A module behind ``#[path = "../shared.rs"]`` can live outside its
    package's ``src`` tree, where a plain directory walk would miss it.
    Follow each attributed path and each ordinary ``mod child;`` declaration
    transitively so discovery matches the files a compiler reads. A declared
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
    yield from _child_targets(source, text, root, roots)


def _path_targets(source: Path, text: str) -> Iterator[Path]:
    """Yield existing ``#[path = "..."]`` module targets."""
    for match in PATH_ATTRIBUTE.finditer(text):
        target = (source.parent / match.group(1)).resolve()
        if target.is_file():
            yield target


def _child_targets(
    source: Path, text: str, root: Path, roots: set[str]
) -> Iterator[Path]:
    """Yield existing targets of ordinary ``mod child;`` declarations."""
    base = _module_directory(source, root, roots)
    for match in MOD_DECLARATION.finditer(text):
        name = match.group(1)
        candidates = (base / f"{name}.rs", base / name / "mod.rs")
        target = next((c for c in candidates if c.is_file()), None)
        if target is not None:
            yield target


def _module_directory(source: Path, root: Path, roots: set[str]) -> Path:
    """Return the directory a file-backed child module resolves against."""
    try:
        relative = source.resolve().relative_to(root).as_posix()
    except ValueError:
        relative = ""
    if relative in roots or source.name in {"lib.rs", "main.rs", "mod.rs"}:
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


#: The workspace source prefixes that schedule a scoped mutation run.
EXPECTED_PATHS = (
    'src/',
    'cli/',
    'common/',
    'crates/',
    'installer/',
    'suite/',
)

#: Sources deliberately outside mutation scope: compiler-internal scaffolding,
#: dylint UI fixtures, shared test infrastructure, the vendored upstream fork,
#: modules compiled only under ``dylint-driver``, Kani harnesses, and a
#: Windows-only installer test. Cargo-mutants enumerates module files without
#: evaluating their cfg, so an unexcluded feature-gated module would report
#: survivors for code the default-feature baseline never compiled.
EXPECTED_EXCLUDES = (
    'crates/rustc_*/**',
    'crates/clippy_utils/**',
    'crates/*/ui/**',
    'crates/*/examples/**',
    'crates/whitaker_lint_core/src/testing/**',
    'common/src/test_support/**',
    'vendor/dylint_testing/**',
    'crates/bumpy_road_function/src/driver/**',
    'crates/conditional_max_n_branches/src/driver/**',
    'crates/conditional_max_n_branches/src/tests/**',
    'crates/function_attrs_follow_docs/src/driver*',
    'crates/function_attrs_follow_docs/src/tests/**',
    'crates/macro_aware_cognitive_complexity/src/driver/**',
    'crates/module_max_lines/src/driver.rs',
    'crates/module_max_lines/src/lib_ui_tests.rs',
    'crates/module_must_have_inner_docs/src/driver/**',
    'crates/module_must_have_inner_docs/src/tests/**',
    'crates/no_expect_outside_tests/src/behaviour.rs',
    'crates/no_expect_outside_tests/src/context/**',
    'crates/no_expect_outside_tests/src/dependency_rlib_tests.rs',
    'crates/no_expect_outside_tests/src/diagnostics.rs',
    'crates/no_expect_outside_tests/src/driver/**',
    'crates/no_expect_outside_tests/src/lib_ui_tests.rs',
    'crates/no_expect_outside_tests/src/tests.rs',
    'crates/no_expect_outside_tests/src/tests/**',
    'crates/no_std_fs_operations/src/behaviour.rs',
    'crates/no_std_fs_operations/src/config*',
    'crates/no_std_fs_operations/src/diagnostics*',
    'crates/no_std_fs_operations/src/driver.rs',
    'crates/no_std_fs_operations/src/exclusion*',
    'crates/no_std_fs_operations/src/tests*',
    'crates/no_std_fs_operations/src/usage/**',
    'crates/no_unwrap_or_else_panic/src/context/**',
    'crates/no_unwrap_or_else_panic/src/diagnostics.rs',
    'crates/no_unwrap_or_else_panic/src/driver.rs',
    'crates/no_unwrap_or_else_panic/src/lib_ui_tests.rs',
    'crates/no_unwrap_or_else_panic/src/panic_detector.rs',
    'crates/no_unwrap_or_else_panic/src/policy.rs',
    'crates/no_unwrap_or_else_panic/src/tests*',
    'crates/rstest_helper_should_be_fixture/src/collector*',
    'crates/rstest_helper_should_be_fixture/src/driver*',
    'crates/rstest_helper_should_be_fixture/src/visitor.rs',
    'crates/string_continuation_style/src/config.rs',
    'crates/string_continuation_style/src/diagnostics.rs',
    'crates/string_continuation_style/src/driver/**',
    'crates/string_continuation_style/src/exclusion.rs',
    'crates/test_must_not_have_example/src/behaviour.rs',
    'crates/test_must_not_have_example/src/driver.rs',
    'crates/test_must_not_have_example/src/heuristics.rs',
    'crates/test_must_not_have_example/src/lib_ui_tests.rs',
    'crates/whitaker_lint_core/src/hir/**',
    'suite/src/driver.rs',
    'common/src/decomposition_advice/community/../community_kani/**',
    'crates/whitaker_clones_core/src/**/kani.rs',
    'crates/whitaker_string_literals/src/**/kani.rs',
    'installer/src/deps/path_windows_tests.rs',
    'src/cli/check/flow/samplers.rs',
    'src/cli/check/flow/script.rs',
    'src/cli/check/flow/verification.rs',
    'src/cli/check/flow/witnesses.rs',
)
