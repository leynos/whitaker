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

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

#: A Rust `use` or path that names another layer.
_LAYER_REFERENCE = re.compile(r"\bcrate::(cli|adapters)\b")


def _rust_sources(directory: Path) -> list[Path]:
    """Return every Rust source file under ``directory``."""
    return sorted(directory.rglob("*.rs"))


def _offending(directory: Path, layers: tuple[str, ...]) -> list[str]:
    """Return ``path:line`` for each reference to one of ``layers``."""
    found: list[str] = []
    for path in _rust_sources(directory):
        for number, line in enumerate(path.read_text().splitlines(), start=1):
            if line.lstrip().startswith("//"):
                # A doc comment may *describe* another layer without importing
                # it; only code creates the dependency.
                continue
            for match in _LAYER_REFERENCE.finditer(line):
                if match.group(1) in layers:
                    found.append(f"{path.relative_to(ROOT)}:{number}: {line.strip()}")
    return found


def test_ports_depend_on_neither_the_command_layer_nor_the_adapters() -> None:
    """The port layer names no command or adapter module."""
    offending = _offending(ROOT / "src" / "ports", ("cli", "adapters"))

    assert not offending, "the port layer must not depend on another layer:\n" + "\n".join(
        offending
    )


def test_adapters_do_not_depend_on_the_command_layer() -> None:
    """The adapters name no command module."""
    offending = _offending(ROOT / "src" / "adapters", ("cli",))

    assert not offending, "an adapter must not depend on the command layer:\n" + "\n".join(
        offending
    )