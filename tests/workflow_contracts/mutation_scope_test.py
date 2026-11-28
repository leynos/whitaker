"""Exercise the mutation-scope discovery helpers.

``mutation_scope`` asks Cargo for workspace members and follows ``#[path]``
module declarations that place a source outside its package's ``src`` tree.
Both are discovery paths the scope guard depends on, so they are tested here
with a real temporary tree and the repository's own metadata rather than only
with a constructed ``PackageScope``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

from mutation_discovery import path_attributed_sources
from mutation_scope import KANI_ITEM_EXCLUSION, is_excluded, workspace_packages
from ubicloud_workflow_support import REPOSITORY_ROOT


def test_path_attributed_modules_are_discovered(tmp_path: Path) -> None:
    """Modules outside ``src`` and their children join the inventory."""
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "lib.rs").write_text(
        '#[path = "../shared.rs"]\nmod shared;\n', encoding="utf-8"
    )
    (tmp_path / "shared.rs").write_text(
        "mod child;\npub fn shared() {}\n", encoding="utf-8"
    )
    (tmp_path / "shared").mkdir()
    (tmp_path / "shared" / "child.rs").write_text(
        "pub fn child() {}\n", encoding="utf-8"
    )
    discovered = path_attributed_sources(("src/lib.rs",), tmp_path)
    assert discovered == ("shared.rs", "shared/child.rs", "src/lib.rs"), (
        "the scope inventory must follow #[path] modules and their child modules"
    )


def test_custom_target_root_resolves_child_modules(tmp_path: Path) -> None:
    """A declared target root resolves child modules beside itself.

    Cargo lets a target live at ``tools/generate.rs`` and declare ``mod
    child;``, which resolves to ``tools/child.rs``. Without target-root
    identity the guard would look under ``tools/generate/`` and miss an
    uncovered child outside ``paths``.
    """
    (tmp_path / "tools").mkdir()
    (tmp_path / "tools" / "generate.rs").write_text(
        "mod child;\n", encoding="utf-8"
    )
    (tmp_path / "tools" / "child.rs").write_text(
        "pub fn child() {}\n", encoding="utf-8"
    )
    roots = ("tools/generate.rs",)
    discovered = path_attributed_sources(roots, tmp_path, roots)
    assert discovered == ("tools/child.rs", "tools/generate.rs"), (
        "a declared target root must resolve its child modules beside itself"
    )
    assert path_attributed_sources(roots, tmp_path) == roots, (
        "without target identity the child must not be guessed from a module path"
    )


def test_inline_module_children_resolve_under_the_inline_module(
    tmp_path: Path,
) -> None:
    """A child of an inline module resolves under the inline module name."""
    (tmp_path / "tools").mkdir()
    (tmp_path / "tools" / "generate.rs").write_text(
        "mod nested {\n    mod child;\n}\n", encoding="utf-8"
    )
    (tmp_path / "tools" / "nested").mkdir()
    (tmp_path / "tools" / "nested" / "child.rs").write_text(
        "pub fn child() {}\n", encoding="utf-8"
    )
    roots = ("tools/generate.rs",)
    discovered = path_attributed_sources(roots, tmp_path, roots)
    assert "tools/nested/child.rs" in discovered, (
        "a child of an inline module must resolve under the inline module name"
    )


def test_unlisted_lib_rs_is_not_treated_as_a_root(tmp_path: Path) -> None:
    """Only declared roots use crate-root resolution for their children."""
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "lib.rs").write_text("mod child;\n", encoding="utf-8")
    (tmp_path / "src" / "lib").mkdir()
    (tmp_path / "src" / "lib" / "child.rs").write_text(
        "pub fn child() {}\n", encoding="utf-8"
    )
    assert path_attributed_sources(("src/lib.rs",), tmp_path) == (
        "src/lib.rs",
        "src/lib/child.rs",
    ), "an undeclared lib.rs is a module file, not a target root"


def test_glob_semantics_match_cargo_mutants() -> None:
    """A single star stays within one path segment; a double star crosses."""
    assert not is_excluded(
        "crates/example/src/tests/ui.rs", ("crates/example/src/tests*",)
    ), "cargo-mutants does not let a single star cross a path separator"
    assert is_excluded(
        "crates/example/src/tests/ui.rs", ("crates/example/src/tests/**",)
    ), "a double star must span path segments"
    assert is_excluded(
        "crates/rustc_ast/src/lib.rs", ("crates/rustc_*/**",)
    ), "a star within one segment must still match the directory name"


def test_workspace_discovery_uses_cargo_metadata() -> None:
    """Discovery includes the root package and a vendored workspace member."""
    directories = {scope.directory for scope in workspace_packages(REPOSITORY_ROOT)}
    assert "." in directories, "the root package must be discovered"
    assert "vendor/dylint_testing" in directories, (
        "a workspace member outside the main source prefixes must be discovered"
    )


def _line_containing(lines: list[str], needle: str, start: int = 0) -> int:
    """Return the 1-based line number of the first line containing ``needle``."""
    for index in range(start, len(lines)):
        if needle in lines[index]:
            return index + 1
    raise AssertionError(f"no line contains {needle!r}")


def _braced_block_end(lines: list[str], start: int) -> int:
    """Return the 1-based line number closing the block opened at ``start``."""
    depth = 0
    for index in range(start, len(lines)):
        depth += lines[index].count("{") - lines[index].count("}")
        if depth == 0 and index > start:
            return index + 1
    raise AssertionError("unclosed brace block")


def test_kani_line_bounds_cover_the_proof_seams_only() -> None:
    """The line-bound clauses exclude Kani regions, not later production code.

    ``minhash.rs`` defines ``sketch_values`` twice, and production functions
    follow the Kani variant. ``lsh.rs`` keeps a ``cfg(kani)`` block inside the
    production ``LshIndex::insert``. The name filter cannot distinguish those
    regions, so the exclusion is line-bound. This test fails if either bound
    drifts over production code or stops covering its proof seam.
    """
    pattern = re.compile(KANI_ITEM_EXCLUSION)

    minhash = (
        REPOSITORY_ROOT / "crates/whitaker_clones_core/src/index/minhash.rs"
    ).read_text(encoding="utf-8").splitlines()
    kani_fn = _line_containing(minhash, "fn sketch_values", 126)
    kani_end = _braced_block_end(minhash, kani_fn - 1)
    production_sketch = _line_containing(minhash, "fn sketch_values")
    assert pattern.search(f"minhash.rs:{kani_fn}:5: replace sketch_values"), (
        "the Kani bound must cover the start of the cfg(kani) sketch_values"
    )
    assert pattern.search(f"minhash.rs:{kani_end}:5: replace sketch_values"), (
        "the Kani bound must cover the end of the cfg(kani) sketch_values"
    )
    assert not pattern.search(f"minhash.rs:{production_sketch}:5: replace sketch_values"), (
        "the Kani bound must leave the production sketch_values mutable"
    )
    for function in ("fn unique_hashes", "fn minimum_mixed_hash"):
        line = _line_containing(minhash, function)
        assert not pattern.search(f"minhash.rs:{line}:5: replace production"), (
            f"the Kani bound must leave {function!r} mutable"
        )

    lsh = (
        REPOSITORY_ROOT / "crates/whitaker_clones_core/src/index/lsh.rs"
    ).read_text(encoding="utf-8").splitlines()
    insert_line = _line_containing(lsh, "pub fn insert")
    kani_cfg = _line_containing(lsh, "#[cfg(kani)]", insert_line)
    kani_block_end = _braced_block_end(lsh, kani_cfg - 1)
    kani_operator = _line_containing(lsh, "if band_index <", kani_cfg)
    assert pattern.search(f"lsh.rs:{kani_operator}:31: replace < with =="), (
        "the LSH bound must cover the cfg(kani) comparison inside insert"
    )
    assert pattern.search(f"lsh.rs:{kani_block_end}:5: replace insert with ()"), (
        "the LSH bound must cover the whole cfg(kani) block"
    )
    assert not pattern.search(f"lsh.rs:{insert_line}:9: replace insert with ()"), (
        "the LSH bound must leave the production insert body mutable"
    )
