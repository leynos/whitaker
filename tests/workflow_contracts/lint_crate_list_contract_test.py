"""Hold the workflow and Makefile lint-crate lists to the installer's constants.

The rolling release builds one lint library per crate named in its
``LINT_CRATES`` workflow env, then hands the whole ``target/<triple>/release``
directory to ``whitaker-package-lints``. That binary discovers what to package
from `installer::resolution::{LINT_CRATES, SUITE_CRATE}`, not from the
workflow, so the two lists are the same fact written twice. When they drift the
build still succeeds and the packaging step fails afterwards, once per matrix
leg, with ``error: library file not found: <path>`` -- the crate is required by
the packager but was never compiled.

That is exactly what happened when `string_continuation_style` joined
``installer/src/resolution.rs``: the crate was added to the installer and the
Makefile but not to `rolling-release.yml`, and every ``build-lints`` leg of the
rolling release failed on `main`. An equivalent check lived in
``tests/workflows/test_rolling_release_workflow.py``, but that directory is an
opt-in ``act`` suite that no workflow runs, so nothing gated the drift.

Two lists are checked here:

* `rolling-release.yml` builds from its own `LINT_CRATES` env and then packages;
  a missing entry fails packaging on every matrix leg. Order is part of that
  contract, since both definitions drive deterministic build logs.
* The `Makefile`'s `LINT_CRATES ?=` is read by `make package-lints`, which does
  invoke ``whitaker-package-lints``, and therefore fails the same way. It is in
  a deliberately different order, so it is compared as a set. `make
  publish-check` reads the same variable but only copies each
  ``lib$$lint.so`` into a Dylint library directory. A crate omitted there is
  never attempted, so it is silently absent from that directory; a `cp` failure
  means a *listed* crate did not produce a library.

These tests live in `tests/workflow_contracts` because ``make
test-workflow-contracts`` runs inside the required ``linux-full`` job.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

from ubicloud_workflow_support import REPOSITORY_ROOT, load_workflow

RESOLUTION_PATH: typ.Final[Path] = REPOSITORY_ROOT / "installer/src/resolution.rs"
MAKEFILE_PATH: typ.Final[Path] = REPOSITORY_ROOT / "Makefile"
WORKFLOW_NAME: typ.Final[str] = "rolling-release.yml"

_LINT_CRATES_PATTERN: typ.Final[re.Pattern[str]] = re.compile(
    r"pub(?:\(crate\))?\s+const\s+LINT_CRATES\s*:\s*&\[\s*&str\s*\]\s*=\s*&\[(.*?)\];",
    re.DOTALL,
)
_SUITE_CRATE_PATTERN: typ.Final[re.Pattern[str]] = re.compile(
    r'pub(?:\(crate\))?\s+const\s+SUITE_CRATE\s*:\s*&str\s*=\s*"([^"]+)";'
)
_MAKEFILE_LINT_CRATES_PATTERN: typ.Final[re.Pattern[str]] = re.compile(
    r"^LINT_CRATES\s*\?=\s*(.+)$", re.MULTILINE
)


def _installer_lint_crates() -> list[str]:
    """Return the packager's canonical list: `LINT_CRATES` then `SUITE_CRATE`."""
    source = RESOLUTION_PATH.read_text(encoding="utf-8")

    lint_match = _LINT_CRATES_PATTERN.search(source)
    assert lint_match, f"unable to parse LINT_CRATES from {RESOLUTION_PATH}"
    lint_crates = re.findall(r'"([^"]+)"', lint_match.group(1))
    assert lint_crates, f"{RESOLUTION_PATH} LINT_CRATES is unexpectedly empty"

    suite_match = _SUITE_CRATE_PATTERN.search(source)
    assert suite_match, f"unable to parse SUITE_CRATE from {RESOLUTION_PATH}"
    return [*lint_crates, suite_match.group(1)]


def _workflow_lint_crates() -> list[str]:
    """Return the crate names the release workflow compiles, in order."""
    workflow_env = load_workflow(WORKFLOW_NAME).get("env") or {}
    raw_value = workflow_env.get("LINT_CRATES")
    assert raw_value is not None, f"{WORKFLOW_NAME} is missing LINT_CRATES"

    # A `>-` block scalar arrives as one space-separated string; a flow or
    # block sequence arrives already split. Both spellings mean the same list.
    match raw_value:
        case str() as crates:
            return crates.split()
        case list() as crates:
            return [str(crate).strip() for crate in crates if str(crate).strip()]
        case _:
            raise AssertionError(
                f"{WORKFLOW_NAME} LINT_CRATES has unexpected type {type(raw_value)!r}"
            )


def _makefile_lint_crates() -> list[str]:
    """Return the crate names `make package-lints` builds, in order."""
    source = MAKEFILE_PATH.read_text(encoding="utf-8")
    match = _MAKEFILE_LINT_CRATES_PATTERN.search(source)
    assert match, f"unable to parse LINT_CRATES from {MAKEFILE_PATH}"
    return match.group(1).split()


def test_rolling_release_builds_every_crate_the_packager_requires() -> None:
    """Ensure the workflow compiles exactly what the packager will collect.

    A crate missing here builds fine and then fails packaging on every matrix
    leg; a crate listed here but absent from the installer is compiled and
    silently never published.
    """
    workflow_crates = _workflow_lint_crates()
    packager_crates = _installer_lint_crates()

    assert workflow_crates == packager_crates, (
        f"{WORKFLOW_NAME} LINT_CRATES drifted from "
        f"installer::resolution constants:\n"
        f"  workflow={workflow_crates}\n"
        f"  installer={packager_crates}\n"
        f"missing from workflow: {sorted(set(packager_crates) - set(workflow_crates))}\n"
        f"unknown to installer: {sorted(set(workflow_crates) - set(packager_crates))}"
    )


def test_makefile_builds_every_crate_the_packager_requires() -> None:
    """Ensure `make package-lints` builds exactly what the packager collects.

    The Makefile reorders the list relative to the installer's constants, so
    this compares sets: the two definitions are independent hand-maintained
    orderings of one membership fact, and only membership affects packaging.
    """
    makefile_crates = _makefile_lint_crates()
    packager_crates = _installer_lint_crates()

    assert set(makefile_crates) == set(packager_crates), (
        f"{MAKEFILE_PATH} LINT_CRATES drifted from "
        f"installer::resolution constants:\n"
        f"  missing from Makefile: {sorted(set(packager_crates) - set(makefile_crates))}\n"
        f"  unknown to installer: {sorted(set(makefile_crates) - set(packager_crates))}"
    )
