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

from mutation_scope import (
    KANI_ITEM_EXCLUSION,
    path_attributed_sources,
    workspace_packages,
)
from ubicloud_workflow_support import REPOSITORY_ROOT


def test_path_attributed_modules_are_discovered(tmp_path: Path) -> None:
    """A ``#[path]`` module outside ``src`` joins the package inventory."""
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "lib.rs").write_text(
        '#[path = "../shared.rs"]\nmod shared;\n', encoding="utf-8"
    )
    (tmp_path / "shared.rs").write_text("pub fn shared() {}\n", encoding="utf-8")
    discovered = path_attributed_sources(("src/lib.rs",), tmp_path)
    assert discovered == ("shared.rs", "src/lib.rs"), (
        "the scope inventory must follow a #[path] module outside the src tree"
    )


def test_workspace_discovery_uses_cargo_metadata() -> None:
    """Discovery includes the root package and a vendored workspace member."""
    directories = {scope.directory for scope in workspace_packages(REPOSITORY_ROOT)}
    assert "." in directories, "the root package must be discovered"
    assert "vendor/dylint_testing" in directories, (
        "a workspace member outside the main source prefixes must be discovered"
    )


def test_kani_name_filter_leaves_the_production_variant() -> None:
    """The line-bound Kani clause excludes the proof seam, not production.

    ``minhash.rs`` defines ``sketch_values`` twice: an idiomatic production
    variant under ``cfg(not(kani))`` and an unrolled Kani-only variant. The
    name filter cannot distinguish them, so the exclusion is line-bound. This
    test fails if either definition moves across the bound.
    """
    source = (
        REPOSITORY_ROOT / "crates/whitaker_clones_core/src/index/minhash.rs"
    ).read_text(encoding="utf-8")
    lines = source.splitlines()

    def definition_line(cfg: str) -> int:
        """Return the line number of the ``sketch_values`` variant for ``cfg``."""
        for index, line in enumerate(lines):
            if line.strip() != cfg:
                continue
            if any("fn sketch_values" in near for near in lines[index : index + 3]):
                return index + 1
        raise AssertionError(f"no sketch_values definition follows {cfg}")

    kani_line = definition_line('#[cfg(kani)]')
    production_line = definition_line('#[cfg(not(kani))]')
    pattern = re.compile(KANI_ITEM_EXCLUSION)
    assert pattern.search(f"minhash.rs:{kani_line}:5: replace sketch_values"), (
        "the Kani name filter must cover the cfg(kani) proof seam"
    )
    assert not pattern.search(f"minhash.rs:{production_line}:5: replace sketch_values"), (
        "the Kani name filter must leave the compiled production variant mutable"
    )