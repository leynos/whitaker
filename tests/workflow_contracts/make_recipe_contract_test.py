"""Keep the passes `make test` delegates actually part of their recipes.

`make test` runs three of its passes through sub-targets: `test-root-lib`,
`test-nextest-filter`, and, under `ACT_WORKFLOW_TESTS`, `workflow-test`. Make
ends a recipe at the first line that does not begin with a tab, so a command
line that loses its tab is parsed as a variable assignment instead. The recipe
then ends early, the pass runs nothing, and `make test` still exits 0.

That failure is invisible to every other gate: the line stays in the file, its
text still reads as the command it was meant to be, and `mbake validate`
accepts the file as valid syntax. A stripped tab has shipped before now,
leaving the nextest filter guard unexercised while the suite reported green.

These tests read the wiring from `make -n`, which prints the commands make would
actually run, so they test make's interpretation instead of re-implementing its
recipe parser. Only the delegated sub-targets are dry-run: `make -n test`
evaluates the recipe's `$(shell)` expansions and is too slow for a contract.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import subprocess  # noqa: S404  # The wiring is read from make -n.
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]

#: Every pass `make test` delegates, with a fragment of the command that proves
#: the dry run reached the recipe body rather than stopping at its guard line.
DELEGATED_PASSES = {
    "test-root-lib": "-p whitaker --lib",
    "test-nextest-filter": "--test nextest_ui_filter",
    "workflow-test": "pytest tests/workflows",
}


def _dry_run(target: str) -> str:
    """Return the commands ``make -n TARGET`` would run, one per line."""
    # For example, dry-running `test-nextest-filter` yields the `command -v
    # cargo-nextest` guard followed by the `cargo nextest run ... --test
    # nextest_ui_filter` line. A target whose command line lost its tab yields
    # only the guard, because make no longer treats the command as part of the
    # recipe.
    result = subprocess.run(  # noqa: S603  # Fixed argument list, no shell.
        ["make", "-n", target],  # noqa: S607  # make comes from PATH, as in the gate.
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout


@pytest.mark.parametrize(("target", "marker"), DELEGATED_PASSES.items())
def test_the_delegated_pass_is_part_of_its_recipe(target: str, marker: str) -> None:
    """Fail when a delegated pass's command is no longer in its recipe."""
    assert marker in _dry_run(target), (
        f"{target} never runs {marker!r}. Make stops a recipe at the first line "
        "without a leading tab, which turns the command into a variable "
        "assignment and leaves this pass silently doing nothing."
    )